// POS view — counter sale flow:
//   search products (/products) → click to add to cart → qty steppers →
//   running total → payment method (Efectivo / Débito / Crédito) → "Cobrar"
//   posts /pos/sale with a fresh Idempotency-Key (minted in Rust). On success:
//   toast + clear cart. INSUFFICIENT_STOCK surfaces as an inline Spanish error.
//
// Money discipline: each cart line keeps the product's ORIGINAL price string
// (`unit_price`) and re-emits it verbatim — local math (Number) is display-only
// for the running total, never sent back to the server.
import {
  listProducts,
  posSale,
  parseSaleError,
  type Product,
  type PosItem,
  type PaymentMethod,
} from "../api";
import { clp, toNumber, num } from "../format";
import { tableSkeleton, asMessage, escapeHtml } from "./inventory";

interface CartLine {
  product: string; // record id
  name: string;
  unit_price: string; // original server Decimal string
  qty: number;
  stock: number; // last-known stock, for the +/- guard
}

const SEARCH_LIMIT = 40;

const METHODS: { id: PaymentMethod; label: string }[] = [
  { id: "pos_cash", label: "Efectivo" },
  { id: "pos_debit", label: "Débito" },
  { id: "pos_credit", label: "Crédito" },
];

export function renderPos(host: HTMLElement, serverUrl: string): void {
  const cart: CartLine[] = [];
  let method: PaymentMethod = "pos_cash";

  host.innerHTML = `
    <section class="view view-pos">
      <div class="pos-grid">
        <!-- left: product picker -->
        <div class="pos-pick">
          <div class="view-search">
            <input id="pos-search" type="search" placeholder="Buscar producto para agregar…" autocomplete="off" />
          </div>
          <div id="pos-results" class="pos-results">${tableSkeleton(6)}</div>
        </div>

        <!-- right: cart + checkout -->
        <aside class="pos-cart">
          <h3 class="section-title">Carrito</h3>
          <div id="pos-lines" class="pos-lines"></div>

          <div class="pos-total">
            <span>Total</span>
            <strong id="pos-total-val">${clp(0)}</strong>
          </div>

          <div class="pos-methods" id="pos-methods">
            ${METHODS.map(
              (m, i) => `<button type="button" class="pos-method ${i === 0 ? "active" : ""}" data-method="${m.id}">${m.label}</button>`,
            ).join("")}
          </div>

          <div id="pos-error" class="pos-error" hidden></div>

          <button id="pos-charge" class="btn-primary pos-charge" disabled>
            <span class="btn-label">Cobrar</span>
            <span class="btn-pulse"></span>
          </button>
        </aside>
      </div>
      <div id="pos-toast" class="toast" hidden></div>
    </section>
  `;

  const searchEl = host.querySelector<HTMLInputElement>("#pos-search")!;
  const resultsEl = host.querySelector<HTMLElement>("#pos-results")!;
  const linesEl = host.querySelector<HTMLElement>("#pos-lines")!;
  const totalEl = host.querySelector<HTMLElement>("#pos-total-val")!;
  const chargeBtn = host.querySelector<HTMLButtonElement>("#pos-charge")!;
  const errorEl = host.querySelector<HTMLElement>("#pos-error")!;
  const toastEl = host.querySelector<HTMLElement>("#pos-toast")!;
  const methodsEl = host.querySelector<HTMLElement>("#pos-methods")!;

  // ---- product search (debounced, server-side) ----
  let timer: number | undefined;
  const runSearch = (q: string) => {
    resultsEl.innerHTML = tableSkeleton(6);
    void loadResults(resultsEl, serverUrl, q, addToCart);
  };
  searchEl.addEventListener("input", () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(() => runSearch(searchEl.value.trim()), 220);
  });
  runSearch("");

  // ---- payment method selector ----
  methodsEl.querySelectorAll<HTMLButtonElement>(".pos-method").forEach((b) => {
    b.addEventListener("click", () => {
      methodsEl.querySelectorAll(".pos-method").forEach((x) => x.classList.remove("active"));
      b.classList.add("active");
      method = b.dataset.method as PaymentMethod;
    });
  });

  // ---- cart ops ----
  function addToCart(p: Product): void {
    if (p.stock <= 0) return; // guarded in the UI too, defensive here
    const existing = cart.find((l) => l.product === p.id);
    if (existing) {
      if (existing.qty < existing.stock) existing.qty += 1;
    } else {
      cart.push({ product: p.id, name: p.name, unit_price: p.price, qty: 1, stock: p.stock });
    }
    clearError();
    renderCart();
  }

  function changeQty(id: string, delta: number): void {
    const line = cart.find((l) => l.product === id);
    if (!line) return;
    line.qty += delta;
    if (line.qty <= 0) {
      cart.splice(cart.indexOf(line), 1);
    } else if (line.qty > line.stock) {
      line.qty = line.stock;
    }
    renderCart();
  }

  function renderCart(): void {
    if (cart.length === 0) {
      linesEl.innerHTML = `<p class="empty">El carrito está vacío. Busca un producto para agregarlo.</p>`;
    } else {
      linesEl.innerHTML = cart
        .map(
          (l) => `
        <div class="pos-line" data-id="${escapeHtml(l.product)}">
          <div class="pos-line-info">
            <div class="cell-main">${escapeHtml(l.name)}</div>
            <div class="cell-sub muted">${clp(l.unit_price)} c/u</div>
          </div>
          <div class="qty">
            <button type="button" class="qty-btn" data-act="dec" aria-label="Quitar uno">−</button>
            <span class="qty-val">${l.qty}</span>
            <button type="button" class="qty-btn" data-act="inc" aria-label="Agregar uno" ${l.qty >= l.stock ? "disabled" : ""}>+</button>
          </div>
          <div class="pos-line-sub num">${clp(toNumber(l.unit_price) * l.qty)}</div>
        </div>`,
        )
        .join("");
      linesEl.querySelectorAll<HTMLButtonElement>(".qty-btn").forEach((b) => {
        b.addEventListener("click", () => {
          const id = b.closest<HTMLElement>(".pos-line")!.dataset.id!;
          changeQty(id, b.dataset.act === "inc" ? 1 : -1);
        });
      });
    }
    const total = cart.reduce((sum, l) => sum + toNumber(l.unit_price) * l.qty, 0);
    totalEl.textContent = clp(total);
    chargeBtn.disabled = cart.length === 0;
  }

  function clearError(): void {
    errorEl.hidden = true;
    errorEl.textContent = "";
  }

  function showError(msg: string): void {
    errorEl.textContent = msg;
    errorEl.hidden = false;
  }

  function toast(msg: string): void {
    toastEl.textContent = msg;
    toastEl.hidden = false;
    toastEl.classList.add("show");
    window.setTimeout(() => {
      toastEl.classList.remove("show");
      window.setTimeout(() => (toastEl.hidden = true), 250);
    }, 2600);
  }

  // ---- checkout ----
  chargeBtn.addEventListener("click", async () => {
    if (cart.length === 0) return;
    clearError();
    chargeBtn.disabled = true;
    chargeBtn.classList.add("loading");

    const items: PosItem[] = cart.map((l) => ({
      product: l.product,
      product_name: l.name,
      quantity: l.qty,
      unit_price: l.unit_price,
    }));
    const total = cart.reduce((sum, l) => sum + toNumber(l.unit_price) * l.qty, 0);
    // Single-tender: hand the full total to the chosen rail so server-side
    // balance checks pass. Mixed cash+card is out of scope for this view.
    const cash = method === "pos_cash" ? String(total) : undefined;
    const card = method === "pos_cash" ? undefined : String(total);

    try {
      await posSale(serverUrl, items, method, cash, card);
      const count = cart.reduce((n, l) => n + l.qty, 0);
      cart.length = 0;
      renderCart();
      toast(`Venta registrada · ${num(count)} ítem(es) · ${clp(total)}`);
      // Refresh the picker so stock counts reflect the sale.
      runSearch(searchEl.value.trim());
    } catch (err) {
      const { code, message } = parseSaleError(err);
      showError(
        code === "INSUFFICIENT_STOCK"
          ? `Stock insuficiente para completar la venta. ${message}`
          : message,
      );
    } finally {
      chargeBtn.classList.remove("loading");
      chargeBtn.disabled = cart.length === 0;
    }
  });

  renderCart();
}

async function loadResults(
  host: HTMLElement,
  serverUrl: string,
  search: string,
  onAdd: (p: Product) => void,
): Promise<void> {
  try {
    const rows: Product[] = await listProducts(serverUrl, search || undefined, SEARCH_LIMIT);
    if (rows.length === 0) {
      host.innerHTML = `<p class="empty">Sin resultados${search ? ` para «${escapeHtml(search)}»` : ""}.</p>`;
      return;
    }
    host.innerHTML = rows.map(resultCard).join("");
    host.querySelectorAll<HTMLButtonElement>(".pos-result").forEach((b, i) => {
      b.addEventListener("click", () => {
        if (rows[i].stock > 0) onAdd(rows[i]);
      });
    });
  } catch (err) {
    host.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
  }
}

function resultCard(p: Product): string {
  const out = p.stock <= 0;
  return `
    <button type="button" class="pos-result ${out ? "is-out" : ""}" ${out ? "disabled" : ""}>
      <div class="pos-result-info">
        <div class="cell-main">${escapeHtml(p.name)}</div>
        <div class="cell-sub muted">Stock: ${num(p.stock)}${out ? " · agotado" : ""}</div>
      </div>
      <div class="pos-result-price num">${clp(p.price)}</div>
    </button>
  `;
}

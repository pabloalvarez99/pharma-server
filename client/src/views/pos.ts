// POS view — counter sale flow:
//   search products (/products) → click / Enter to add to cart → qty steppers →
//   running total → optional customer (loyalty) → payment method → cash received
//   + vuelto → "Cobrar" posts /pos/sale with a fresh Idempotency-Key (minted in
//   Rust). On success: boleta modal (printable) + toast with loyalty awarded.
//   INSUFFICIENT_STOCK surfaces as an inline Spanish error.
//
// Money discipline: each cart line keeps the product's ORIGINAL price string
// (`unit_price`) and re-emits it verbatim — local Number math is display-only
// (running total, vuelto), never sent back as a product price. The cash TENDERED
// is a separate amount the cashier types; the server computes the authoritative
// `change` on the receipt.
import {
  listProducts,
  posSale,
  parseSaleError,
  customerSearch,
  getReceipt,
  CUSTOMERS_MODULE_MISSING,
  type Product,
  type PosItem,
  type PaymentMethod,
  type Customer,
  type Receipt,
  type LowStockAlert,
} from "../api";
import { clp, toNumber, num, parseCash, effectiveTender, vuelto, quickCashAmounts } from "../format";
import { tableSkeleton, asMessage, escapeHtml } from "./inventory";
import "./rutbrand.css";
import {
  addToCart as addCartLine,
  changeQty as changeCartQty,
  cartTotal as cartTotalOf,
  orderDiscount as orderDiscountOf,
  payableTotal as payableTotalOf,
  lineDiscount as lineDiscountOf,
  splitPayment,
  type CartLine,
} from "./cashier-loop";
import { bindModalKeys } from "./modal-keys";

interface PickedCustomer {
  id: string;
  name: string;
  points: number;
}

const SEARCH_LIMIT = 40;
const CUST_LIMIT = 8;

const METHODS: { id: PaymentMethod; label: string }[] = [
  { id: "pos_cash", label: "Efectivo" },
  { id: "pos_debit", label: "Débito" },
  { id: "pos_credit", label: "Crédito" },
  { id: "pos_mixed", label: "Mixto" },
];

const METHOD_LABEL: Record<string, string> = {
  pos_cash: "Efectivo",
  pos_debit: "Débito",
  pos_credit: "Crédito",
  pos_mixed: "Mixto",
};

export function renderPos(host: HTMLElement, serverUrl: string): void {
  const cart: CartLine[] = [];
  let method: PaymentMethod = "pos_cash";
  let globalDiscount = 0; // flat order-level discount in pesos (cashier-typed)
  let selectedCustomer: PickedCustomer | null = null;
  let customerModuleOk = true;
  let currentResults: Product[] = [];

  host.innerHTML = `
    <section class="view view-pos">
      <div class="pos-grid">
        <!-- left: product picker -->
        <div class="pos-pick">
          <div class="view-search">
            <input id="pos-search" type="search" placeholder="Buscar producto (Enter agrega el primero)…" autocomplete="off" />
          </div>
          <div id="pos-results" class="pos-results">${tableSkeleton(6)}</div>
        </div>

        <!-- right: cart + checkout -->
        <aside class="pos-cart">
          <h3 class="section-title">Carrito</h3>
          <div id="pos-lines" class="pos-lines"></div>

          <!-- global discount (per-line discount lives on each cart line) -->
          <div class="pos-discount" id="pos-discount">
            <label class="pos-disc-label" for="pos-disc-in">Descuento global</label>
            <input id="pos-disc-in" type="text" inputmode="numeric" placeholder="0" autocomplete="off" />
          </div>

          <div class="pos-totals" id="pos-totals">
            <div class="pos-subtotal" id="pos-subtotal-row" hidden>
              <span>Subtotal</span>
              <strong id="pos-subtotal-val" class="rb-num">${clp(0)}</strong>
            </div>
            <div class="pos-disc-row" id="pos-disc-row" hidden>
              <span>Descuento</span>
              <strong id="pos-disc-val" class="rb-num">− ${clp(0)}</strong>
            </div>
            <div class="pos-total">
              <span>Total</span>
              <strong id="pos-total-val" class="rb-num">${clp(0)}</strong>
            </div>
          </div>

          <!-- customer (loyalty) -->
          <div class="pos-customer" id="pos-customer">
            <div class="pos-cust-row" id="pos-cust-row">
              <input id="pos-cust-search" type="search" placeholder="Cliente (opcional) — nombre o RUT…" autocomplete="off" />
              <div id="pos-cust-results" class="pos-cust-results" hidden></div>
            </div>
            <div id="pos-cust-chip" class="customer-chip" hidden></div>
            <div id="pos-cust-note" class="pos-cust-note muted" hidden></div>
          </div>

          <div class="pos-methods" id="pos-methods">
            ${METHODS.map(
              (m, i) => `<button type="button" class="pos-method ${i === 0 ? "active" : ""}" data-method="${m.id}">${m.label}</button>`,
            ).join("")}
          </div>

          <!-- cash tendered + vuelto (only for Efectivo) -->
          <div class="pos-cash" id="pos-cash">
            <label class="pos-cash-label" for="pos-cash-in">Efectivo recibido</label>
            <input id="pos-cash-in" type="text" inputmode="numeric" placeholder="0" autocomplete="off" />
            <div class="pos-quick" id="pos-quick"></div>
            <div class="pos-vuelto" id="pos-vuelto" hidden></div>
          </div>

          <!-- split tender: efectivo + tarjeta (only for Mixto) -->
          <div class="pos-split" id="pos-split" hidden>
            <div class="pos-split-row">
              <label class="pos-cash-label" for="pos-split-cash">Efectivo</label>
              <input id="pos-split-cash" type="text" inputmode="numeric" placeholder="0" autocomplete="off" />
            </div>
            <div class="pos-split-row">
              <label class="pos-cash-label" for="pos-split-card">Tarjeta</label>
              <input id="pos-split-card" type="text" inputmode="numeric" placeholder="0" autocomplete="off" />
            </div>
            <div class="pos-split-info" id="pos-split-info" hidden></div>
          </div>

          <div id="pos-error" class="pos-error" hidden></div>

          <button id="pos-charge" class="btn-primary pos-charge" disabled>
            <span class="btn-label">Cobrar</span>
            <span class="btn-pulse"></span>
          </button>
        </aside>
      </div>
      <div id="pos-toast" class="toast" hidden></div>
      <div id="pos-modal-host"></div>
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
  const modalHost = host.querySelector<HTMLElement>("#pos-modal-host")!;
  // customer els
  const custRow = host.querySelector<HTMLElement>("#pos-cust-row")!;
  const custSearchEl = host.querySelector<HTMLInputElement>("#pos-cust-search")!;
  const custResultsEl = host.querySelector<HTMLElement>("#pos-cust-results")!;
  const custChipEl = host.querySelector<HTMLElement>("#pos-cust-chip")!;
  const custNoteEl = host.querySelector<HTMLElement>("#pos-cust-note")!;
  // cash els
  const cashWrap = host.querySelector<HTMLElement>("#pos-cash")!;
  const cashIn = host.querySelector<HTMLInputElement>("#pos-cash-in")!;
  const quickEl = host.querySelector<HTMLElement>("#pos-quick")!;
  const vueltoEl = host.querySelector<HTMLElement>("#pos-vuelto")!;
  // discount els
  const discIn = host.querySelector<HTMLInputElement>("#pos-disc-in")!;
  const subtotalRow = host.querySelector<HTMLElement>("#pos-subtotal-row")!;
  const subtotalVal = host.querySelector<HTMLElement>("#pos-subtotal-val")!;
  const discRow = host.querySelector<HTMLElement>("#pos-disc-row")!;
  const discVal = host.querySelector<HTMLElement>("#pos-disc-val")!;
  // split (mixed) els
  const splitWrap = host.querySelector<HTMLElement>("#pos-split")!;
  const splitCashIn = host.querySelector<HTMLInputElement>("#pos-split-cash")!;
  const splitCardIn = host.querySelector<HTMLInputElement>("#pos-split-card")!;
  const splitInfo = host.querySelector<HTMLElement>("#pos-split-info")!;

  // Money model: gross subtotal → order discount (line + global) → payable.
  // `payable` is the authoritative total for vuelto, quick chips and the split.
  const subtotal = (): number => cartTotalOf(cart);
  const discountNow = (): number => orderDiscountOf(cart, globalDiscount);
  const payable = (): number => payableTotalOf(cart, globalDiscount);

  // ---- product search (debounced, server-side) ----
  let timer: number | undefined;
  const runSearch = (q: string) => {
    resultsEl.innerHTML = tableSkeleton(6);
    void loadResults(q);
  };
  searchEl.addEventListener("input", () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(() => runSearch(searchEl.value.trim()), 220);
  });
  // Scan-fast: Enter adds the first in-stock result (USB scanners emit Enter).
  searchEl.addEventListener("keydown", (e) => {
    if (e.key !== "Enter") return;
    e.preventDefault();
    const first = currentResults.find((p) => p.stock > 0);
    if (first) {
      addToCart(first);
      searchEl.value = "";
      runSearch("");
    }
  });
  runSearch("");

  async function loadResults(search: string): Promise<void> {
    try {
      const rows: Product[] = await listProducts(serverUrl, search || undefined, SEARCH_LIMIT);
      currentResults = rows;
      if (rows.length === 0) {
        resultsEl.innerHTML = `<p class="empty">Sin resultados${search ? ` para «${escapeHtml(search)}»` : ""}.</p>`;
        return;
      }
      resultsEl.innerHTML = rows.map(resultCard).join("");
      resultsEl.querySelectorAll<HTMLButtonElement>(".pos-result").forEach((b, i) => {
        b.addEventListener("click", () => {
          if (rows[i].stock > 0) addToCart(rows[i]);
        });
      });
    } catch (err) {
      currentResults = [];
      resultsEl.innerHTML = `<div class="view-error">${escapeHtml(asMessage(err))}</div>`;
    }
  }

  // ---- payment method selector ----
  methodsEl.querySelectorAll<HTMLButtonElement>(".pos-method").forEach((b) => {
    b.addEventListener("click", () => {
      methodsEl.querySelectorAll(".pos-method").forEach((x) => x.classList.remove("active"));
      b.classList.add("active");
      method = b.dataset.method as PaymentMethod;
      syncPayment();
    });
  });

  // ---- tender panels: Efectivo (single) vs Mixto (split) vs card (none) ----
  function syncPayment(): void {
    const isCash = method === "pos_cash";
    const isMixed = method === "pos_mixed";
    cashWrap.style.display = isCash ? "" : "none";
    splitWrap.hidden = !isMixed;
    if (isCash) renderQuickChips();
    if (isMixed) renderSplit();
    renderVuelto();
    syncChargeBtn();
  }

  // ---- global discount ----
  discIn.addEventListener("input", () => {
    globalDiscount = parseCash(discIn.value);
    renderTotals();
    renderQuickChips();
    renderVuelto();
    renderSplit();
    syncChargeBtn();
  });

  // Subtotal / descuento / total breakdown. The subtotal + discount rows only
  // show when a discount is actually applied (clean ticket for the common case).
  function renderTotals(): void {
    const sub = subtotal();
    const disc = discountNow();
    const showDisc = disc > 0;
    subtotalRow.hidden = !showDisc;
    discRow.hidden = !showDisc;
    if (showDisc) {
      subtotalVal.textContent = clp(sub);
      discVal.innerHTML = `− ${clp(disc)}`;
    }
    totalEl.textContent = clp(payable());
  }

  function renderQuickChips(): void {
    const amounts = quickCashAmounts(payable());
    if (amounts.length === 0) {
      quickEl.innerHTML = "";
      return;
    }
    quickEl.innerHTML = amounts
      .map((a) => `<button type="button" class="pos-quick-chip" data-amt="${a}">${clp(a)}</button>`)
      .join("");
    quickEl.querySelectorAll<HTMLButtonElement>(".pos-quick-chip").forEach((b) => {
      b.addEventListener("click", () => {
        cashIn.value = String(b.dataset.amt);
        renderVuelto();
      });
    });
  }

  function renderVuelto(): void {
    if (method !== "pos_cash") {
      vueltoEl.hidden = true;
      return;
    }
    const v = vuelto(parseCash(cashIn.value), payable());
    if (v.kind === "none") {
      vueltoEl.hidden = true;
      return;
    }
    vueltoEl.hidden = false;
    if (v.kind === "ok") {
      vueltoEl.className = "pos-vuelto ok";
      vueltoEl.innerHTML = `Vuelto <strong class="rb-num">${clp(v.amount)}</strong>`;
    } else {
      vueltoEl.className = "pos-vuelto short";
      vueltoEl.innerHTML = `Faltan <strong class="rb-num">${clp(v.amount)}</strong>`;
    }
  }

  // Live split-tender feedback: how much is still missing, or the vuelto when
  // cash+card exceeds the total (the overpayment falls on the cash side).
  function currentSplit() {
    return splitPayment(
      { cash: parseCash(splitCashIn.value), card: parseCash(splitCardIn.value) },
      payable(),
    );
  }

  function renderSplit(): void {
    if (method !== "pos_mixed") {
      splitInfo.hidden = true;
      return;
    }
    const s = currentSplit();
    if (s.tendered <= 0) {
      splitInfo.hidden = true;
      return;
    }
    splitInfo.hidden = false;
    if (!s.ok) {
      splitInfo.className = "pos-split-info short";
      splitInfo.innerHTML = `Faltan <strong class="rb-num">${clp(s.short)}</strong>`;
    } else if (s.change > 0) {
      splitInfo.className = "pos-split-info ok";
      splitInfo.innerHTML = `Vuelto <strong class="rb-num">${clp(s.change)}</strong>`;
    } else {
      splitInfo.className = "pos-split-info ok";
      splitInfo.innerHTML = `Pago exacto`;
    }
  }

  // Charge is blocked on an empty cart, or a Mixto split that doesn't cover the
  // total yet — so the cashier can't post an underfunded mixed sale.
  function chargeEnabled(): boolean {
    if (cart.length === 0) return false;
    if (method === "pos_mixed") return currentSplit().ok;
    return true;
  }
  function syncChargeBtn(): void {
    chargeBtn.disabled = !chargeEnabled();
  }

  cashIn.addEventListener("input", renderVuelto);
  splitCashIn.addEventListener("input", () => {
    renderSplit();
    syncChargeBtn();
  });
  splitCardIn.addEventListener("input", () => {
    renderSplit();
    syncChargeBtn();
  });

  // ---- customer picker (loyalty) ----
  let custTimer: number | undefined;
  custSearchEl.addEventListener("input", () => {
    window.clearTimeout(custTimer);
    const q = custSearchEl.value.trim();
    if (q.length < 2) {
      custResultsEl.hidden = true;
      return;
    }
    custTimer = window.setTimeout(() => void loadCustomers(q), 240);
  });
  custSearchEl.addEventListener("blur", () => {
    // delay so a result click registers before we hide the dropdown
    window.setTimeout(() => (custResultsEl.hidden = true), 160);
  });

  async function loadCustomers(q: string): Promise<void> {
    try {
      const rows: Customer[] = await customerSearch(serverUrl, q);
      if (!customerModuleOk) return;
      if (rows.length === 0) {
        custResultsEl.hidden = false;
        custResultsEl.innerHTML = `<p class="empty">Sin clientes para «${escapeHtml(q)}».</p>`;
        return;
      }
      custResultsEl.hidden = false;
      custResultsEl.innerHTML = rows
        .slice(0, CUST_LIMIT)
        .map(
          (c) => `
        <button type="button" class="cli-result pos-cust-result" data-id="${escapeHtml(c.id)}">
          <div class="pos-cust-info">
            <div class="cell-main">${escapeHtml(c.name)}</div>
            <div class="cell-sub muted rb-num">${c.rut ? escapeHtml(c.rut) : "sin RUT"}</div>
          </div>
          <span class="cli-points rb-num">${num(c.loyalty_points)} pts</span>
        </button>`,
        )
        .join("");
      custResultsEl.querySelectorAll<HTMLButtonElement>(".pos-cust-result").forEach((b, i) => {
        b.addEventListener("click", () => {
          const c = rows[i];
          selectedCustomer = { id: c.id, name: c.name, points: c.loyalty_points };
          custResultsEl.hidden = true;
          custSearchEl.value = "";
          renderCustomer();
        });
      });
    } catch (err) {
      if (err === CUSTOMERS_MODULE_MISSING) {
        customerModuleOk = false;
        custRow.hidden = true;
        custResultsEl.hidden = true;
        custNoteEl.hidden = false;
        custNoteEl.textContent = "Módulo de clientes no disponible en este servidor.";
        return;
      }
      custResultsEl.hidden = false;
      custResultsEl.innerHTML = `<p class="empty">${escapeHtml(asMessage(err))}</p>`;
    }
  }

  function renderCustomer(): void {
    if (selectedCustomer) {
      custRow.hidden = true;
      custChipEl.hidden = false;
      custChipEl.innerHTML = `
        <span class="customer-chip-name">${escapeHtml(selectedCustomer.name)}</span>
        <span class="customer-chip-pts rb-num">${num(selectedCustomer.points)} pts</span>
        <button type="button" class="customer-chip-x" aria-label="Quitar cliente">×</button>`;
      custChipEl.querySelector<HTMLButtonElement>(".customer-chip-x")!.addEventListener("click", () => {
        selectedCustomer = null;
        renderCustomer();
      });
    } else if (customerModuleOk) {
      custRow.hidden = false;
      custChipEl.hidden = true;
    }
  }

  // ---- cart ops ----
  function addToCart(p: Product): void {
    addCartLine(cart, p); // pure: out-of-stock reject + stock-capped increment
    clearError();
    renderCart();
    searchEl.focus(); // keep the scanner/keyboard flow going
  }

  function changeQty(id: string, delta: number): void {
    changeCartQty(cart, id, delta); // pure: clamp to stock, drop at 0
    renderCart();
  }

  function removeLine(id: string): void {
    const i = cart.findIndex((l) => l.product === id);
    if (i < 0) return;
    cart.splice(i, 1);
    renderCart();
    searchEl.focus(); // line gone — return to the scan box
  }

  // renderCart() rewrites the lines' innerHTML, dropping DOM focus. Re-focus the
  // same line by id after a qty tweak so holding ↑/↓ keeps editing it; if the
  // line vanished (qty hit 0), fall back to the search box.
  function refocusLine(id: string): void {
    const el = linesEl.querySelector<HTMLElement>(`.pos-line[data-id="${CSS.escape(id)}"]`);
    if (el) el.focus();
    else searchEl.focus();
  }

  function renderCart(): void {
    if (cart.length === 0) {
      linesEl.innerHTML = `<p class="empty">El carrito está vacío. Busca un producto para agregarlo.</p>`;
    } else {
      linesEl.innerHTML = cart
        .map(
          (l) => `
        <div class="pos-line" data-id="${escapeHtml(l.product)}" tabindex="0" role="group"
             aria-label="${escapeHtml(l.name)}, ${l.qty} unidad(es). Flechas para ajustar, Supr para quitar.">
          <div class="pos-line-info">
            <div class="cell-main">${escapeHtml(l.name)}</div>
            <div class="cell-sub muted rb-num">${clp(l.unit_price)} c/u</div>
          </div>
          <div class="qty">
            <button type="button" class="qty-btn" data-act="dec" aria-label="Quitar uno">−</button>
            <span class="qty-val rb-num">${l.qty}</span>
            <button type="button" class="qty-btn" data-act="inc" aria-label="Agregar uno" ${l.qty >= l.stock ? "disabled" : ""}>+</button>
          </div>
          <input class="pos-line-disc" type="text" inputmode="numeric" placeholder="Desc."
                 value="${lineDiscountOf(l) > 0 ? lineDiscountOf(l) : ""}"
                 aria-label="Descuento para ${escapeHtml(l.name)}" />
          <div class="pos-line-sub num rb-num">${clp(toNumber(l.unit_price) * l.qty - lineDiscountOf(l))}</div>
        </div>`,
        )
        .join("");
      linesEl.querySelectorAll<HTMLButtonElement>(".qty-btn").forEach((b) => {
        b.addEventListener("click", () => {
          const id = b.closest<HTMLElement>(".pos-line")!.dataset.id!;
          changeQty(id, b.dataset.act === "inc" ? 1 : -1);
        });
      });
      // Keyboard-editable cart: a focused line takes ↑/+/→ to add, ↓/−/← to
      // remove one, Supr/Backspace to drop it — so the cashier never leaves the
      // keyboard between scanning and checkout. Focus is preserved across the
      // re-render by id so holding a key keeps adjusting the same line.
      linesEl.querySelectorAll<HTMLElement>(".pos-line").forEach((row) => {
        row.addEventListener("keydown", (e) => {
          const id = row.dataset.id!;
          const k = e.key;
          if (k === "ArrowUp" || k === "ArrowRight" || k === "+") {
            e.preventDefault();
            changeQty(id, 1);
            refocusLine(id);
          } else if (k === "ArrowDown" || k === "ArrowLeft" || k === "-") {
            e.preventDefault();
            changeQty(id, -1);
            refocusLine(id);
          } else if (k === "Delete" || k === "Backspace") {
            e.preventDefault();
            removeLine(id);
          }
        });
      });
      // Per-line discount: editing it updates only the totals/tender previews,
      // never re-renders the cart (which would steal focus from the input mid-
      // type). Keystrokes are stopped from bubbling to the row so digits/arrows
      // here don't trigger the qty/delete handlers above.
      linesEl.querySelectorAll<HTMLInputElement>(".pos-line-disc").forEach((inp) => {
        inp.addEventListener("keydown", (e) => e.stopPropagation());
        inp.addEventListener("input", () => {
          const id = inp.closest<HTMLElement>(".pos-line")!.dataset.id!;
          const line = cart.find((l) => l.product === id);
          if (!line) return;
          line.discount = parseCash(inp.value);
          const subEl = inp.parentElement!.querySelector<HTMLElement>(".pos-line-sub");
          if (subEl) subEl.textContent = clp(toNumber(line.unit_price) * line.qty - lineDiscountOf(line));
          renderTotals();
          renderQuickChips();
          renderVuelto();
          renderSplit();
          syncChargeBtn();
        });
      });
    }
    renderTotals();
    renderQuickChips();
    renderVuelto();
    renderSplit();
    syncChargeBtn();
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
    }, 3200);
  }

  // ---- checkout ----
  // Keyboard-first: the click handler and the cash-input Enter both route here,
  // so a cashier never has to reach for the mouse to close a cash sale. The
  // `chargeBtn.disabled` guard doubles as a re-entrancy lock (empty cart OR a
  // sale already in flight), so a double Enter can't post twice.
  async function charge(): Promise<void> {
    if (chargeBtn.disabled || cart.length === 0) return;
    clearError();
    chargeBtn.disabled = true;
    chargeBtn.classList.add("loading");

    const items: PosItem[] = cart.map((l) => ({
      product: l.product,
      product_name: l.name,
      quantity: l.qty,
      unit_price: l.unit_price,
    }));
    const total = payable(); // NET of discounts — the amount actually due
    const discount = discountNow();
    // Hand each rail the amount that makes the server-side balance check pass:
    //  · pos_cash  → the tendered cash (or the exact total) so the server
    //    computes the vuelto; · card → the exact total; · pos_mixed → the
    //    cashier's split (cash + card), validated to cover the total here.
    let cash: string | undefined;
    let card: string | undefined;
    if (method === "pos_cash") {
      cash = String(effectiveTender(parseCash(cashIn.value), total));
    } else if (method === "pos_mixed") {
      const s = currentSplit();
      if (!s.ok) {
        showError(`El pago dividido no cubre el total. Faltan ${clp(s.short)}.`);
        chargeBtn.classList.remove("loading");
        syncChargeBtn();
        return;
      }
      cash = String(parseCash(splitCashIn.value));
      card = String(parseCash(splitCardIn.value));
    } else {
      card = String(total);
    }
    const discountArg = discount > 0 ? String(discount) : undefined;

    try {
      const result = await posSale(
        serverUrl,
        items,
        method,
        cash,
        card,
        selectedCustomer?.id,
        discountArg,
      );
      const count = cart.reduce((n, l) => n + l.qty, 0);
      cart.length = 0;
      globalDiscount = 0;
      discIn.value = "";
      cashIn.value = "";
      splitCashIn.value = "";
      splitCardIn.value = "";
      renderCart();
      renderVuelto();

      const pts = result.loyaltyPointsAwarded;
      toast(`Venta registrada · ${num(count)} ítem(es) · ${clp(total)}${pts > 0 ? ` · +${num(pts)} pts` : ""}`);
      flashLowStock(result.lowStockAlerts);

      // Reset the customer for the next sale, then fetch + show the boleta.
      const orderId = result.orderId;
      selectedCustomer = null;
      renderCustomer();
      if (orderId) {
        try {
          const receipt = await getReceipt(serverUrl, orderId);
          showReceipt(receipt);
        } catch {
          // Sale already committed — a missing ticket never blocks the flow.
        }
      }
      // Refresh the picker so stock counts reflect the sale.
      runSearch(searchEl.value.trim());
      searchEl.focus();
    } catch (err) {
      const { code, message } = parseSaleError(err);
      showError(
        code === "INSUFFICIENT_STOCK"
          ? `Stock insuficiente para completar la venta. ${message}`
          : message,
      );
    } finally {
      chargeBtn.classList.remove("loading");
      syncChargeBtn();
    }
  }

  chargeBtn.addEventListener("click", () => void charge());
  // Enter in any tender field closes the sale — the fast path (no mouse).
  for (const el of [cashIn, splitCashIn, splitCardIn]) {
    el.addEventListener("keydown", (e) => {
      if (e.key !== "Enter") return;
      e.preventDefault();
      void charge();
    });
  }

  function flashLowStock(alerts: LowStockAlert[]): void {
    if (!alerts || alerts.length === 0) return;
    const names = alerts.map((a) => a.product_name).slice(0, 3).join(", ");
    const more = alerts.length > 3 ? ` +${alerts.length - 3}` : "";
    showError(`Stock bajo: ${escapeHtml(names)}${more}. Revisa reposición.`);
    errorEl.className = "pos-error pos-warn";
    window.setTimeout(() => {
      clearError();
      errorEl.className = "pos-error";
    }, 5200);
  }

  // ---- boleta / receipt modal ----
  function showReceipt(r: Receipt): void {
    const when = (() => {
      const d = new Date(r.datetime);
      return Number.isNaN(d.getTime()) ? r.datetime : d.toLocaleString("es-CL");
    })();
    const rows = r.items
      .map(
        (it) => `
      <tr>
        <td>${escapeHtml(it.name)}</td>
        <td class="num rb-num">${num(it.qty)}</td>
        <td class="num rb-num">${clp(it.unit_price)}</td>
        <td class="num rb-num">${clp(it.line_total)}</td>
      </tr>`,
      )
      .join("");
    const discountRow =
      toNumber(r.discount) > 0
        ? `<div class="rcpt-line"><span>Descuento</span><strong class="rb-num">− ${clp(r.discount)}</strong></div>`
        : "";
    // Cash sale: Recibido + Vuelto. Mixed sale: the two tenders + Vuelto (the
    // server computes `change` for pos_mixed now too — F-paul-pay-001).
    const cashBlock =
      r.payment_method === "pos_cash" && r.cash_amount
        ? `<div class="rcpt-line"><span>Recibido</span><strong class="rb-num">${clp(r.cash_amount)}</strong></div>
           <div class="rcpt-line"><span>Vuelto</span><strong class="rb-num">${clp(r.change ?? "0")}</strong></div>`
        : r.payment_method === "pos_mixed"
          ? `<div class="rcpt-line"><span>Efectivo</span><strong class="rb-num">${clp(r.cash_amount ?? "0")}</strong></div>
             <div class="rcpt-line"><span>Tarjeta</span><strong class="rb-num">${clp(r.card_amount ?? "0")}</strong></div>
             <div class="rcpt-line"><span>Vuelto</span><strong class="rb-num">${clp(r.change ?? "0")}</strong></div>`
          : "";
    const loyaltyBlock =
      r.loyalty_points_awarded > 0
        ? `<div class="rcpt-line accent"><span>Puntos ganados</span><strong class="rb-num">+${num(r.loyalty_points_awarded)}</strong></div>`
        : "";

    modalHost.innerHTML = `
      <div class="modal-backdrop" id="rcpt-backdrop">
        <div class="modal receipt-modal" id="receipt-print">
          <div class="rcpt-head">
            <div class="rcpt-tenant">${escapeHtml(r.tenant_name)}</div>
            <div class="rcpt-meta muted">Boleta <span class="rb-num">${escapeHtml(r.folio_or_number)}</span> · ${escapeHtml(when)}</div>
            ${r.cashier ? `<div class="rcpt-meta muted">Cajero: ${escapeHtml(r.cashier)}</div>` : ""}
          </div>
          <table class="data-table rcpt-table">
            <thead><tr><th>Producto</th><th class="num">Cant</th><th class="num">P/U</th><th class="num">Total</th></tr></thead>
            <tbody>${rows}</tbody>
          </table>
          <div class="rcpt-totals">
            <div class="rcpt-line"><span>Subtotal</span><strong class="rb-num">${clp(r.subtotal)}</strong></div>
            ${discountRow}
            <div class="rcpt-line total"><span>Total</span><strong class="rb-num">${clp(r.total)}</strong></div>
            <div class="rcpt-line"><span>Pago</span><strong>${escapeHtml(METHOD_LABEL[r.payment_method] ?? r.payment_method)}</strong></div>
            ${cashBlock}
            ${loyaltyBlock}
          </div>
          <div class="rcpt-foot muted">${escapeHtml(r.footer_note)}</div>
          <div class="modal-actions rcpt-actions">
            <button type="button" class="btn-ghost" id="rcpt-close">Cerrar</button>
            <button type="button" class="btn-primary modal-confirm" id="rcpt-print">Imprimir</button>
          </div>
        </div>
      </div>
    `;

    // Escape dismisses the boleta and returns to the scan box, so a mouseless
    // cashier isn't trapped on the ticket. (Enter is left for the focused
    // Imprimir/Cerrar buttons — binding it here would swallow keyboard print.)
    const detachKeys = bindModalKeys(() => close());
    const close = () => {
      detachKeys();
      modalHost.innerHTML = "";
      searchEl.focus();
    };
    host.querySelector<HTMLButtonElement>("#rcpt-close")!.addEventListener("click", close);
    host.querySelector<HTMLButtonElement>("#rcpt-print")!.addEventListener("click", () => window.print());
    host.querySelector<HTMLElement>("#rcpt-backdrop")!.addEventListener("click", (e) => {
      if (e.target === e.currentTarget) close();
    });
  }

  // initial paint + scan-ready focus
  renderCart();
  renderCustomer();
  syncPayment();
  searchEl.focus();
}

function resultCard(p: Product): string {
  const out = p.stock <= 0;
  return `
    <button type="button" class="pos-result ${out ? "is-out" : ""}" ${out ? "disabled" : ""}>
      <div class="pos-result-info">
        <div class="cell-main">${escapeHtml(p.name)}</div>
        <div class="cell-sub muted">Stock: <span class="rb-num">${num(p.stock)}</span>${out ? " · agotado" : ""}</div>
      </div>
      <div class="pos-result-price num rb-num">${clp(p.price)}</div>
    </button>
  `;
}

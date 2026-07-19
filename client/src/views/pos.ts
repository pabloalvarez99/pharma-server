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
  listProductVariants,
  productByBarcode,
  productFromDetail,
  posSale,
  parseSaleError,
  customerSearch,
  getReceipt,
  emitBoleta,
  sendDte,
  dteXml,
  printReceiptPreferThermal,
  CUSTOMERS_MODULE_MISSING,
  type Product,
  type PosItem,
  type PaymentMethod,
  type Customer,
  type Receipt,
  type Dte,
  type LowStockAlert,
} from "../api";
import { clp, toNumber, num, parseCash, effectiveTender, vuelto, quickCashAmounts } from "../format";
import { tableSkeleton, asMessage, escapeHtml } from "./view-blocks";
import { receiptText } from "./receipt-text";
import { loadFeatures, activeVocab, loadBusinessName } from "../vertical";
import "./rutbrand.css";
import {
  addToCart as addCartLine,
  changeQty as changeCartQty,
  cartTotal as cartTotalOf,
  orderDiscount as orderDiscountOf,
  payableTotal as payableTotalOf,
  lineDiscount as lineDiscountOf,
  splitPayment,
  holdSale,
  recallSale,
  parseDiscountEntry,
  type CartLine,
  type HeldSale,
} from "./cashier-loop";
import { bindModalKeys } from "./modal-keys";
import {
  parentWithVariantsError,
  preferBarcodeLookup,
  isParentHasVariantsMessage,
  plainOutOfStockError,
  posVariantsSearchHint,
} from "./variants-ui";

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
  // Stock discipline depends on the rubro: a service rubro (physicalStock:false —
  // peluquería, oficios) sells with no inventory, so "Stock 0 · agotado" would be
  // a lie and the stock guards would dead-end every sale. Default to tracking
  // (the physical-rubro behaviour) until the persisted rubro loads, then relax it
  // for services and repaint the picker so the cards read honestly.
  let trackStock = true;
  // Pack item word (Producto / Servicio / Plato) — refreshed after loadFeatures.
  let itemWord = activeVocab().item || "Producto";
  // Business display name for ticket footer when the receipt has none.
  let businessName: string | null = null;
  // Parked sales (ventas en espera): each is a frozen snapshot of the draft. The
  // cashier holds the current cart to ring up someone else, then recalls it.
  let held: HeldSale[] = [];
  // The cart line to flash on the next render — set when a scan/click adds or
  // bumps a line, cleared once the flash is applied. A fast cashier scanning the
  // SAME SKU repeatedly only sees the qty tick up, so the flash is the signal the
  // scan actually landed (one-shot CSS animation on the freshly-rendered node).
  let flashLineId: string | null = null;

  host.innerHTML = `
    <section class="view view-pos">
      <div class="pos-grid">
        <!-- left: product picker -->
        <div class="pos-pick">
          <div class="view-search">
            <input id="pos-search" type="search" placeholder="${escapeHtml(posSearchPlaceholder(itemWord, trackStock))}" autocomplete="off" />
          </div>
          <div id="pos-results" class="pos-results">${tableSkeleton(6)}</div>
        </div>

        <!-- right: cart + checkout -->
        <aside class="pos-cart">
          <div class="pos-cart-head">
            <h3 class="section-title">Carrito</h3>
            <button type="button" class="pos-hold-btn" id="pos-hold" disabled
                    title="Poner la venta en espera (F2)">En espera</button>
          </div>
          <!-- parked sales (ventas en espera): click un chip para recuperar -->
          <div class="pos-held-bar" id="pos-held-bar" hidden></div>
          <div id="pos-lines" class="pos-lines"></div>

          <!-- global discount (per-line discount lives on each cart line) -->
          <div class="pos-discount" id="pos-discount">
            <label class="pos-disc-label" for="pos-disc-in">Descuento global</label>
            <input id="pos-disc-in" type="text" inputmode="numeric" placeholder="$ o %" autocomplete="off" />
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
  const holdBtn = host.querySelector<HTMLButtonElement>("#pos-hold")!;
  const heldBar = host.querySelector<HTMLElement>("#pos-held-bar")!;
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
    clearScanMiss(); // typing clears a previous "no encontrado" flag
    window.clearTimeout(timer);
    timer = window.setTimeout(() => runSearch(searchEl.value.trim()), 220);
  });
  // Scan-fast: Enter prefers by-barcode (multi-SKU child / plain EAN), then the
  // first sellable search hit. USB scanners dump the code + Enter. Parent
  // products with variants are blocked client-side with Spanish copy (server
  // also rejects on charge). Misses never dead-end the field.
  searchEl.addEventListener("keydown", (e) => {
    if (e.key !== "Enter") return;
    e.preventDefault();
    void handleScanEnter(searchEl.value.trim());
  });
  runSearch("");

  /** Session cache: product id → has active variants (avoid GET on every click). */
  const variantsCache = new Map<string, boolean>();

  async function productHasVariants(p: Product): Promise<boolean> {
    // Fast path: B list/detail exposes variants_stock / variant_count on parents.
    if (
      (p.variants_stock != null && typeof p.variants_stock === "number") ||
      (p.variant_count != null && typeof p.variant_count === "number")
    ) {
      variantsCache.set(p.id, true);
      return true;
    }
    if (variantsCache.has(p.id)) return variantsCache.get(p.id)!;
    try {
      const kids = await listProductVariants(serverUrl, p.id);
      const has = kids.length > 0;
      variantsCache.set(p.id, has);
      return has;
    } catch {
      // Old server without variants route — treat as plain SKU (no block).
      variantsCache.set(p.id, false);
      return false;
    }
  }

  async function tryAddSellable(p: Product): Promise<boolean> {
    // Parent multi-SKU first (even if shell stock is 0 / looks "agotado").
    if (await productHasVariants(p)) {
      showError(parentWithVariantsError(p.name));
      beep(false);
      return false;
    }
    if (trackStock && p.stock <= 0) {
      showError(plainOutOfStockError(p.name));
      beep(false);
      return false;
    }
    addToCart(p);
    return true;
  }

  async function handleScanEnter(code: string): Promise<void> {
    if (!code) {
      // Prefer in-stock, but still try first row so parent@0 surfaces variants error.
      const first =
        (trackStock ? currentResults.find((p) => p.stock > 0) : undefined) ?? currentResults[0];
      if (first) void tryAddSellable(first);
      return;
    }
    // 1) Barcode path (variant or plain SKU with product_barcode).
    if (preferBarcodeLookup(code) || currentResults.length === 0) {
      try {
        const hit = await productByBarcode(serverUrl, code);
        // by-barcode returns the sellable row (child or plano) — never the bare parent.
        // Still guard: if API ever returns a parent shell, block with Spanish copy.
        const ok = await tryAddSellable(productFromDetail(hit));
        if (ok) {
          searchEl.value = "";
          runSearch("");
        }
        return;
      } catch {
        // Not a registered barcode — fall through to name search hit.
      }
    }
    // 2) First search result (name match), with parent-variants + stock guards.
    // Do not skip stock=0 parents — tryAddSellable explains "tiene variantes".
    const first = currentResults[0];
    if (first) {
      const ok = await tryAddSellable(first);
      if (ok) {
        searchEl.value = "";
        runSearch("");
      }
      return;
    }
    // 3) Last chance: try by-barcode even when preferBarcodeLookup was false
    // (short internal codes) before declaring a miss.
    try {
      const hit = await productByBarcode(serverUrl, code);
      const ok = await tryAddSellable(productFromDetail(hit));
      if (ok) {
        searchEl.value = "";
        runSearch("");
      }
      return;
    } catch {
      scanMiss(code);
    }
  }

  // Pack features (cached after shell login); never throws. Service rubro →
  // drop stock tracking and repaint so cards stop showing phantom "agotado".
  // Also re-hydrate vocab + business name if this view mounted before shell
  // hydrate finished (C4 race).
  void Promise.all([loadFeatures(serverUrl), loadBusinessName(serverUrl)]).then(([f, name]) => {
    itemWord = activeVocab().item || "Producto";
    searchEl.placeholder = posSearchPlaceholder(itemWord, trackStock);
    businessName = name;
    const nextTrack = f.physicalStock;
    if (nextTrack !== trackStock) {
      trackStock = nextTrack;
    }
    // Always repaint results so service/physical cards + vocab are honest.
    runSearch(searchEl.value.trim());
    if (cart.length === 0) renderCart();
  });

  async function loadResults(search: string): Promise<void> {
    try {
      const rows: Product[] = await listProducts(serverUrl, search || undefined, SEARCH_LIMIT);
      currentResults = rows;
      if (rows.length === 0) {
        resultsEl.innerHTML = `<p class="empty">${escapeHtml(
          search
            ? `Sin ${itemWord.toLowerCase()}s para «${search}». Revisa el código o el nombre.`
            : `No hay ${itemWord.toLowerCase()}s en el catálogo todavía.`,
        )}</p>`;
        return;
      }
      resultsEl.innerHTML = rows.map((p) => resultCard(p, trackStock, itemWord)).join("");
      resultsEl.querySelectorAll<HTMLButtonElement>(".pos-result").forEach((b, i) => {
        // Always clickable: stock=0 may be a multi-SKU parent (variants error)
        // or a plain out-of-stock SKU (Spanish sin stock). Never dead-end the card.
        b.addEventListener("click", () => {
          void tryAddSellable(rows[i]);
        });
      });
    } catch (err) {
      currentResults = [];
      resultsEl.innerHTML = `<div class="view-error">${escapeHtml(friendlyPosError(err))}</div>`;
    }
  }

  // ---- payment method selector ----
  methodsEl.querySelectorAll<HTMLButtonElement>(".pos-method").forEach((b) => {
    b.addEventListener("click", () => {
      methodsEl.querySelectorAll(".pos-method").forEach((x) => x.classList.remove("active"));
      b.classList.add("active");
      method = b.dataset.method as PaymentMethod;
      syncPayment();
      // Focus the relevant tender field so the cashier can type immediately.
      if (method === "pos_cash") {
        cashIn.focus();
        cashIn.select();
      } else if (method === "pos_mixed") {
        splitCashIn.focus();
        splitCashIn.select();
      }
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
    // Accepts a flat monto ("1500") or a percentage of the subtotal ("10%").
    globalDiscount = parseDiscountEntry(discIn.value, subtotal());
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

  cashIn.addEventListener("input", () => {
    renderVuelto();
    syncChargeBtn();
  });
  // Mixed: as the cashier types cash, auto-suggest the remaining on card when
  // card is still empty (common path: "paga 5 mil en efectivo y el resto tarjeta").
  splitCashIn.addEventListener("input", () => {
    const total = payable();
    const cash = parseCash(splitCashIn.value);
    if (splitCardIn.value.trim() === "" && cash > 0 && cash < total) {
      // Don't fight a cashier who already typed card — only fill when blank.
      // Value is display-only digits; parseCash strips grouping.
      splitCardIn.placeholder = String(total - cash);
    }
    renderSplit();
    syncChargeBtn();
  });
  splitCardIn.addEventListener("input", () => {
    renderSplit();
    syncChargeBtn();
  });
  // Tab from empty card with a cash shortfall → fill the remainder (keyboard path).
  splitCardIn.addEventListener("focus", () => {
    const total = payable();
    const cash = parseCash(splitCashIn.value);
    if (splitCardIn.value.trim() === "" && cash > 0 && cash < total) {
      splitCardIn.value = String(total - cash);
      renderSplit();
      syncChargeBtn();
    }
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
    addCartLine(cart, p, { trackStock }); // pure: stock guards gated by rubro
    flashLineId = p.id; // confirm the scan/click on the affected line
    beep(true); // audible "landed" cue for a head-down cashier
    clearError();
    renderCart();
    searchEl.focus(); // keep the scanner/keyboard flow going
  }

  function changeQty(id: string, delta: number): void {
    changeCartQty(cart, id, delta, { trackStock }); // clamp to stock unless service
    if (delta > 0) flashLineId = id; // confirm the increment (kbd/+ button)
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
      linesEl.innerHTML = `<p class="empty">El carrito está vacío. Busca un ${escapeHtml(itemWord.toLowerCase())} para agregarlo.</p>`;
    } else {
      linesEl.innerHTML = cart
        .map(
          (l) => `
        <div class="pos-line${l.product === flashLineId ? " pos-line-flash" : ""}" data-id="${escapeHtml(l.product)}" tabindex="0" role="group"
             aria-label="${escapeHtml(l.name)}, ${l.qty} unidad(es). Flechas para ajustar, Supr para quitar.">
          <div class="pos-line-info">
            <div class="cell-main">${escapeHtml(l.name)}</div>
            <div class="cell-sub muted rb-num">${clp(l.unit_price)} c/u</div>
          </div>
          <div class="qty">
            <button type="button" class="qty-btn" data-act="dec" aria-label="Quitar uno">−</button>
            <span class="qty-val rb-num">${l.qty}</span>
            <button type="button" class="qty-btn" data-act="inc" aria-label="Agregar uno" ${trackStock && l.qty >= l.stock ? "disabled" : ""}>+</button>
          </div>
          <input class="pos-line-disc" type="text" inputmode="numeric" placeholder="$ o %"
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
          // Monto ("500") or % of this line's gross ("10%").
          line.discount = parseDiscountEntry(inp.value, toNumber(line.unit_price) * line.qty);
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
    flashLineId = null; // one-shot: don't re-flash on later discount/qty renders
    holdBtn.disabled = cart.length === 0; // can't park an empty cart
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

  // ---- scan feedback (audible + visual) ----
  // A short WebAudio blip confirms a scan landed without the cashier looking up;
  // a lower tone marks a miss. Best-effort: muted/blocked audio (no AudioContext,
  // autoplay policy) never throws, so the sale flow is untouched.
  let audioCtx: AudioContext | null = null;
  function beep(ok: boolean): void {
    try {
      const Ctor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!Ctor) return;
      audioCtx ??= new Ctor();
      const osc = audioCtx.createOscillator();
      const gain = audioCtx.createGain();
      osc.frequency.value = ok ? 880 : 220;
      gain.gain.value = 0.04; // quiet — a tick, not a siren
      osc.connect(gain);
      gain.connect(audioCtx.destination);
      const t = audioCtx.currentTime;
      osc.start(t);
      osc.stop(t + 0.06);
    } catch {
      /* audio is optional polish — never block the sale */
    }
  }

  // A scanned code with no match: tone + shake + keep the cashier in the box with
  // the code selected, so the next scan (or a retype) just works.
  function scanMiss(code: string): void {
    beep(false);
    showError(`No encontramos «${code}» en el catálogo. Revisa el código o el nombre.`);
    searchEl.classList.remove("pos-scan-miss");
    void searchEl.offsetWidth; // reflow so the CSS animation restarts
    searchEl.classList.add("pos-scan-miss");
    searchEl.select();
  }
  function clearScanMiss(): void {
    searchEl.classList.remove("pos-scan-miss");
    clearError();
  }

  // ---- hold / recall (ventas en espera) ----
  function clearDraft(): void {
    cart.length = 0;
    globalDiscount = 0;
    discIn.value = "";
    cashIn.value = "";
    splitCashIn.value = "";
    splitCardIn.value = "";
    selectedCustomer = null;
  }

  function holdCurrent(): void {
    if (cart.length === 0) return;
    const res = holdSale(held, { lines: cart, globalDiscount, customer: selectedCustomer });
    held = res.held;
    clearDraft();
    clearError();
    renderCart();
    renderCustomer();
    renderVuelto();
    renderHeld();
    toast(`Venta en espera · ${res.sale.label}`);
    searchEl.focus();
  }

  function recall(id: string): void {
    const r = recallSale(held, id);
    if (!r) return;
    let nextHeld = r.held;
    // Don't lose the current cart: park it before swapping in the recalled one.
    if (cart.length > 0) {
      const parked = holdSale(nextHeld, { lines: cart, globalDiscount, customer: selectedCustomer });
      nextHeld = parked.held;
    }
    held = nextHeld;
    clearDraft();
    for (const l of r.sale.lines) cart.push(l);
    globalDiscount = r.sale.globalDiscount;
    discIn.value = globalDiscount > 0 ? String(globalDiscount) : "";
    selectedCustomer = r.sale.customer;
    clearError();
    renderCart();
    renderCustomer();
    renderVuelto();
    renderHeld();
    toast(`Venta recuperada · ${r.sale.label}`);
    searchEl.focus();
  }

  function renderHeld(): void {
    holdBtn.disabled = cart.length === 0;
    if (held.length === 0) {
      heldBar.hidden = true;
      heldBar.innerHTML = "";
      return;
    }
    heldBar.hidden = false;
    heldBar.innerHTML =
      `<span class="pos-held-title muted">En espera</span>` +
      held
        .map((h) => {
          const count = h.lines.reduce((n, l) => n + l.qty, 0);
          const total = payableTotalOf(h.lines, h.globalDiscount);
          return `<button type="button" class="pos-held-chip" data-id="${escapeHtml(h.id)}"
                  title="Recuperar ${escapeHtml(h.label)}">
            <span class="pos-held-chip-name">${escapeHtml(h.label)}</span>
            <span class="pos-held-chip-meta rb-num">${num(count)} ít · ${clp(total)}</span>
          </button>`;
        })
        .join("");
    heldBar.querySelectorAll<HTMLButtonElement>(".pos-held-chip").forEach((b) => {
      b.addEventListener("click", () => recall(b.dataset.id!));
    });
  }

  holdBtn.addEventListener("click", holdCurrent);
  // F2 parks the current sale from anywhere in the view (keyboard-first).
  host.addEventListener("keydown", (e) => {
    if (e.key === "F2") {
      e.preventDefault();
      holdCurrent();
    }
  });

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
      const tendered = parseCash(cashIn.value);
      // Explicit short cash: warn and keep focus so the cashier can top up.
      // Blank/0 still means "pago exacto" (effectiveTender floors up) — the
      // common path when the customer hands exact change.
      if (tendered > 0 && tendered < total) {
        showError(`Faltan ${clp(total - tendered)} en efectivo. Completa el monto o elige otro medio.`);
        chargeBtn.classList.remove("loading");
        syncChargeBtn();
        cashIn.focus();
        cashIn.select();
        return;
      }
      cash = String(effectiveTender(tendered, total));
    } else if (method === "pos_mixed") {
      const s = currentSplit();
      if (!s.ok) {
        showError(`El pago mixto no cubre el total. Faltan ${clp(s.short)}.`);
        chargeBtn.classList.remove("loading");
        syncChargeBtn();
        splitCashIn.focus();
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
      const unit = count === 1 ? itemWord.toLowerCase() : `${itemWord.toLowerCase()}s`;
      toast(`Venta registrada · ${num(count)} ${unit} · ${clp(total)}${pts > 0 ? ` · +${num(pts)} pts` : ""}`);
      if (trackStock) flashLowStock(result.lowStockAlerts);

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
      if (code === "INSUFFICIENT_STOCK") {
        showError(
          trackStock
            ? `Stock insuficiente. Revisa el carrito o repone el inventario. ${message}`
            : friendlyPosError(message),
        );
      } else if (isParentHasVariantsMessage(message)) {
        // Server-side parent guard (domain sales) — surface as-is (Spanish).
        showError(message);
      } else {
        showError(friendlyPosError(message || err));
      }
      // Return focus to search so the next scan isn't lost after a failed charge.
      searchEl.focus();
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
    if (!trackStock || !alerts || alerts.length === 0) return;
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

    // Footer: prefer server footer_note; else business.name; never blank brand.
    const foot = (r.footer_note && r.footer_note.trim()) || businessName || r.tenant_name || "";
    const headName = (businessName && businessName.trim()) || r.tenant_name;

    modalHost.innerHTML = `
      <div class="modal-backdrop" id="rcpt-backdrop">
        <div class="modal receipt-modal" id="receipt-print">
          <div class="rcpt-head">
            <div class="rcpt-tenant">${escapeHtml(headName)}</div>
            <div class="rcpt-meta muted">Boleta <span class="rb-num">${escapeHtml(r.folio_or_number)}</span> · ${escapeHtml(when)}</div>
            ${r.cashier ? `<div class="rcpt-meta muted">Cajero: ${escapeHtml(r.cashier)}</div>` : ""}
          </div>
          <table class="data-table rcpt-table">
            <thead><tr><th>${escapeHtml(itemWord)}</th><th class="num">Cant</th><th class="num">P/U</th><th class="num">Total</th></tr></thead>
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
          <div class="rcpt-foot muted">${escapeHtml(foot)}</div>
          <!-- Boleta electrónica SII — optional, collapsed: the everyday cash
               sale stays a one-key flow (print/copy/close). Emission needs the
               cert passphrase, so it never blocks the hot path. -->
          <details class="rcpt-sii" id="rcpt-sii">
            <summary>Boleta electrónica SII</summary>
            <div class="rcpt-sii-body">
              <input type="password" id="rcpt-sii-pass" class="rb-input" placeholder="Clave del certificado" autocomplete="off" />
              <input type="text" id="rcpt-sii-rut" class="rb-input" placeholder="RUT receptor (opcional)" autocomplete="off" />
              <button type="button" class="btn-secondary" id="rcpt-sii-emit">Emitir y firmar</button>
              <div class="rcpt-sii-status" id="rcpt-sii-status" hidden></div>
              <div class="rcpt-sii-actions" id="rcpt-sii-after" hidden>
                <button type="button" class="btn-ghost" id="rcpt-sii-xml">Descargar XML</button>
                <button type="button" class="btn-ghost" id="rcpt-sii-send">Enviar al SII</button>
              </div>
            </div>
          </details>
          <div class="modal-actions rcpt-actions">
            <button type="button" class="btn-ghost" id="rcpt-close">Cerrar</button>
            <button type="button" class="btn-ghost" id="rcpt-copy">Copiar</button>
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
    // Enrich ticket with business display name for thermal + clipboard paths
    // (window.print still uses the on-screen modal HTML).
    const ticket: Receipt = {
      ...r,
      tenant_name: headName,
      footer_note: foot || r.footer_note,
    };

    host.querySelector<HTMLButtonElement>("#rcpt-close")!.addEventListener("click", close);
    // Thermal when configured (localStorage rb.thermalPrinter); else browser print.
    // Failures fall back to window.print so a dead spooler never blocks the cajero.
    host.querySelector<HTMLButtonElement>("#rcpt-print")!.addEventListener("click", () => {
      void printReceiptPreferThermal(ticket);
    });
    host.querySelector<HTMLButtonElement>("#rcpt-copy")!.addEventListener("click", () => {
      void copyReceipt(ticket, host.querySelector<HTMLButtonElement>("#rcpt-copy")!);
    });
    host.querySelector<HTMLElement>("#rcpt-backdrop")!.addEventListener("click", (e) => {
      if (e.target === e.currentTarget) close();
    });
    bindSiiEmit(r);
  }

  // Copy a pasteable boleta to the clipboard (WhatsApp/email share). Falls back
  // to a hidden textarea + execCommand where the async Clipboard API is blocked
  // (insecure origin / older webview) so it works in the Tauri shell too.
  async function copyReceipt(r: Receipt, btn: HTMLButtonElement): Promise<void> {
    const text = receiptText(r);
    let ok = false;
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
        ok = true;
      }
    } catch {
      ok = false;
    }
    if (!ok) ok = legacyCopy(text);
    const prev = btn.textContent;
    btn.textContent = ok ? "Copiado ✓" : "No se pudo copiar";
    window.setTimeout(() => {
      btn.textContent = prev;
    }, 1600);
  }

  function legacyCopy(text: string): boolean {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    let ok = false;
    try {
      ok = document.execCommand("copy");
    } catch {
      ok = false;
    }
    document.body.removeChild(ta);
    return ok;
  }

  // Inline boleta-39 emission from the ticket. Free signs the XML locally (valid
  // offline boleta); the SII *upload* is tier-gated, so "Enviar al SII" on Free
  // returns FEATURE_REQUIRES_UPGRADE — surfaced as a calm note, never a crash.
  function bindSiiEmit(r: Receipt): void {
    const passEl = host.querySelector<HTMLInputElement>("#rcpt-sii-pass")!;
    const rutEl = host.querySelector<HTMLInputElement>("#rcpt-sii-rut")!;
    const emitBtn = host.querySelector<HTMLButtonElement>("#rcpt-sii-emit")!;
    const statusEl = host.querySelector<HTMLElement>("#rcpt-sii-status")!;
    const afterEl = host.querySelector<HTMLElement>("#rcpt-sii-after")!;
    const xmlBtn = host.querySelector<HTMLButtonElement>("#rcpt-sii-xml")!;
    const sendBtn = host.querySelector<HTMLButtonElement>("#rcpt-sii-send")!;
    let dte: Dte | null = null;

    const setStatus = (msg: string, kind: "ok" | "err" | "info"): void => {
      statusEl.textContent = msg;
      statusEl.className = `rcpt-sii-status ${kind}`;
      statusEl.hidden = false;
    };

    emitBtn.addEventListener("click", async () => {
      if (!passEl.value) {
        setStatus("Ingresa la clave del certificado digital.", "err");
        return;
      }
      emitBtn.disabled = true;
      try {
        dte = await emitBoleta(serverUrl, r.order_id, passEl.value, rutEl.value.trim() || undefined);
        passEl.value = "";
        setStatus(`Boleta folio ${num(dte.folio)} firmada · XML local listo.`, "ok");
        afterEl.hidden = false;
      } catch (err) {
        const { message } = parseSaleError(err);
        setStatus(message, "err");
        emitBtn.disabled = false;
      }
    });

    xmlBtn.addEventListener("click", async () => {
      if (!dte) return;
      try {
        const xml = await dteXml(serverUrl, dte.id);
        downloadXml(xml, `boleta-${dte.folio}.xml`);
      } catch (err) {
        setStatus(asMessage(err), "err");
      }
    });

    sendBtn.addEventListener("click", async () => {
      if (!dte) return;
      sendBtn.disabled = true;
      try {
        const sent = await sendDte(serverUrl, dte.id);
        dte = sent;
        setStatus(`Boleta enviada al SII (${sent.estado}).`, "ok");
      } catch (err) {
        const { code, message } = parseSaleError(err);
        setStatus(
          code === "FEATURE_REQUIRES_UPGRADE"
            ? `Envío automático al SII es plan Pro. El XML local ya es válido. ${message}`
            : message,
          "info",
        );
        sendBtn.disabled = false;
      }
    });
  }

  function downloadXml(xml: string, filename: string): void {
    const blob = new Blob([xml], { type: "application/xml" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }

  // initial paint + scan-ready focus
  renderCart();
  renderCustomer();
  renderHeld();
  syncPayment();
  searchEl.focus();
}

// A picker card. On a service rubro (`trackStock` false) stock is meaningless, so
// the card never shows a count or an "agotado" dead-end — it reads the pack item
// word ("Servicio") and stays sellable. On a physical rubro it keeps the stock
// line + out-of-stock lock. Exported (pure, no DOM) so the per-rubro render is
// unit-tested directly.
export function resultCard(p: Product, trackStock: boolean, itemLabel = "Servicio"): string {
  if (!trackStock) {
    return `
    <button type="button" class="pos-result is-service">
      <div class="pos-result-info">
        <div class="cell-main">${escapeHtml(p.name)}</div>
        <div class="cell-sub muted">${escapeHtml(itemLabel)}</div>
      </div>
      <div class="pos-result-price num rb-num">${clp(p.price)}</div>
    </button>
  `;
  }
  const isParent =
    (p.variants_stock != null && typeof p.variants_stock === "number") ||
    (p.variant_count != null && typeof p.variant_count === "number");
  if (isParent) {
    const sum = p.variants_stock != null ? Number(p.variants_stock) : null;
    const count = p.variant_count != null ? Number(p.variant_count) : null;
    const mid =
      count != null
        ? count === 1
          ? "1 variante"
          : `${count} variantes`
        : sum != null
          ? `stock en variantes: ${num(sum)}`
          : "multi-SKU";
    return `
    <button type="button" class="pos-result is-parent-variants" aria-description="Producto con variantes; escanear código de barras del hijo">
      <div class="pos-result-info">
        <div class="cell-main">${escapeHtml(p.name)}</div>
        <div class="cell-sub muted">Multi-SKU · ${escapeHtml(mid)} · escanear barcode hijo</div>
      </div>
      <div class="pos-result-price num rb-num">${clp(p.price)}</div>
    </button>
  `;
  }
  const out = p.stock <= 0;
  // Never `disabled` on stock=0: multi-SKU parents without variants_stock flag
  // (old server) still need click → GET /variants. Plain OOS gets Spanish copy.
  return `
    <button type="button" class="pos-result ${out ? "is-out" : ""}" ${
      out ? 'aria-description="Agotado o posible padre multi-SKU"' : ""
    }>
      <div class="pos-result-info">
        <div class="cell-main">${escapeHtml(p.name)}</div>
        <div class="cell-sub muted">Stock: <span class="rb-num">${num(p.stock)}</span>${
          out ? ' · <span class="pos-agotado">Agotado</span> · ¿variantes?' : ""
        }</div>
      </div>
      <div class="pos-result-price num rb-num">${clp(p.price)}</div>
    </button>
  `;
}

/** Search box placeholder — pack vocab so a peluquería never reads "producto". */
export function posSearchPlaceholder(itemWord: string, physicalStock = true): string {
  if (physicalStock) return posVariantsSearchHint(itemWord);
  const w = (itemWord || "producto").toLowerCase();
  return `Buscar ${w} (Enter agrega el primero)…`;
}

/** Operator-facing POS error: strip raw/dev English when possible. */
export function friendlyPosError(err: unknown): string {
  const msg = typeof err === "string" ? err : asMessage(err);
  const low = msg.toLowerCase();
  if (low.includes("timeout") || low.includes("timed out")) {
    return "El servidor no respondió a tiempo. Inténtalo de nuevo.";
  }
  if (low.includes("network") || low.includes("conexión") || low.includes("conectar")) {
    return "Sin conexión al servidor. Verifica que esté en marcha e inténtalo de nuevo.";
  }
  if (low.includes("403") || low.includes("denegado") || low.includes("permiso")) {
    return "No tienes permiso para cobrar. Contacta al administrador.";
  }
  // Keep Spanish server messages; never surface raw stack traces.
  if (msg.includes("\n") || msg.includes("at ") || msg.startsWith("Error:")) {
    return "No se pudo completar la venta. Inténtalo de nuevo.";
  }
  return msg || "No se pudo completar la venta. Inténtalo de nuevo.";
}

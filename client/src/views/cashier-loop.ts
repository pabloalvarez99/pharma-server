// Pure cashier-loop state logic behind the POS / caja / devoluciones views.
// No DOM: this is the money + stock math a real cashier drives through a full
// shift (open caja → sell → pay+vuelto → boleta → devolución → arqueo+cierre),
// extracted so it's regression-testable as a sequence of user-journey steps
// (vitest, node env — no jsdom). The views import these and keep only rendering
// and event wiring, so the arithmetic is single-sourced and can't drift between
// what the screen shows and what the tests check.
//
// Money discipline (mirrors the views): server amounts are Decimal STRINGS; we
// parse only for local display/arithmetic and never round-trip a float back as
// a price. CLP has no minor unit, so cash is integer pesos throughout.
import { toNumber } from "../format";

// --- POS cart ----------------------------------------------------------------

/** A line in the POS cart: the product's original price string is kept verbatim
 *  for re-emit; `stock` is the last-known count used to cap the quantity. */
export interface CartLine {
  product: string; // record id
  name: string;
  unit_price: string; // original server Decimal string
  qty: number;
  stock: number; // last-known stock, for the +/- guard
  /** Optional per-line discount in integer pesos. Display-only on the client:
   *  it is summed (with the global discount) into the SINGLE `discount` amount
   *  the server applies — `unit_price` is always re-emitted verbatim, never a
   *  net price (money discipline). Absent/≤0 means no line discount. */
  discount?: number;
}

/** Anything sellable the picker can hand the cart (a `Product` satisfies it). */
export interface Sellable {
  id: string;
  name: string;
  price: string;
  stock: number;
}

/** Cart-mutation options.
 *
 *  `trackStock` (default `true`) gates the stock guards. A **service rubro**
 *  (`physicalStock: false` — peluquería, oficios) sells with no inventory: its
 *  products carry a meaningless `stock` (often 0), so the out-of-stock reject and
 *  the qty cap would dead-end the sale. Passing `{ trackStock: false }` drops both
 *  guards so a service line adds and increments freely; the money math is
 *  untouched. Physical rubros omit it and keep the exact shelf-bound behaviour. */
export interface CartOpts {
  trackStock?: boolean;
}

/** Add one unit of `p` to `cart` (in place). With stock tracking (default),
 *  out-of-stock items are rejected and an existing line increments only while it
 *  stays at or below its known stock, so the cart can never promise more units
 *  than the shelf holds. With `{ trackStock: false }` (service rubro) neither
 *  guard applies — there is no shelf to bound. */
export function addToCart(cart: CartLine[], p: Sellable, opts: CartOpts = {}): void {
  const track = opts.trackStock !== false;
  if (track && p.stock <= 0) return;
  const existing = cart.find((l) => l.product === p.id);
  if (existing) {
    if (!track || existing.qty < existing.stock) existing.qty += 1;
  } else {
    cart.push({ product: p.id, name: p.name, unit_price: p.price, qty: 1, stock: p.stock });
  }
}

/** Nudge a line's quantity by `delta` (in place). Hitting 0 (or below) drops the
 *  line; exceeding stock clamps to stock UNLESS stock tracking is off (service
 *  rubro), where qty grows unbounded. Returns the resulting cart so callers can
 *  chain, but the mutation is in place to match the view's array identity. */
export function changeQty(
  cart: CartLine[],
  id: string,
  delta: number,
  opts: CartOpts = {},
): CartLine[] {
  const track = opts.trackStock !== false;
  const line = cart.find((l) => l.product === id);
  if (!line) return cart;
  line.qty += delta;
  if (line.qty <= 0) {
    cart.splice(cart.indexOf(line), 1);
  } else if (track && line.qty > line.stock) {
    line.qty = line.stock;
  }
  return cart;
}

/** Running GROSS subtotal of the cart in integer pesos (before any discount).
 *  Garbage prices floor to 0 (via `toNumber`) so the total never reads NaN. */
export function cartTotal(cart: CartLine[]): number {
  return cart.reduce((sum, l) => sum + toNumber(l.unit_price) * l.qty, 0);
}

// --- POS discount (línea + global) -------------------------------------------
// The server applies ONE global `discount` amount (PosSaleRequest.discount),
// clamped into [0, subtotal] (crate::domain::invariants::clamp_discount) so the
// total never goes negative. The POS lets the cashier enter a discount per line
// AND a global one; both are summed into that single amount. We mirror the
// server's clamps EXACTLY so the live preview equals what the server will store.

/** Clamp a discount into `[0, subtotal]` — mirrors `clamp_discount`: over the
 *  subtotal caps at subtotal, negative floors to 0. Truncates to whole pesos. */
export function clampDiscount(discountIn: number, subtotal: number): number {
  const d = Math.trunc(Number.isFinite(discountIn) ? discountIn : 0);
  if (d > subtotal) return Math.max(0, subtotal);
  if (d < 0) return 0;
  return d;
}

/** One line's discount, clamped into `[0, that line's gross]` — a line discount
 *  can never exceed what the line is worth (so it can't bleed into other lines
 *  or make a line negative). */
export function lineDiscount(line: CartLine): number {
  const gross = toNumber(line.unit_price) * line.qty;
  return clampDiscount(line.discount ?? 0, gross);
}

/** Σ of every line's clamped discount. */
export function lineDiscountsTotal(cart: CartLine[]): number {
  return cart.reduce((sum, l) => sum + lineDiscount(l), 0);
}

/** Total order discount the server will apply = (Σ clamped line discounts +
 *  clamped global), capped at the gross subtotal. This is the single `discount`
 *  amount sent on the wire. */
export function orderDiscount(cart: CartLine[], global = 0): number {
  const subtotal = cartTotal(cart);
  const raw = lineDiscountsTotal(cart) + clampDiscount(global, subtotal);
  return raw > subtotal ? subtotal : raw;
}

/** What the customer actually pays = gross subtotal − order discount (≥ 0).
 *  Mirrors `order_total(subtotal, clamp_discount(discount, subtotal))`. */
export function payableTotal(cart: CartLine[], global = 0): number {
  return cartTotal(cart) - orderDiscount(cart, global);
}

// --- POS quick-discount entry (monto OR %) -----------------------------------
// The per-line and global discount inputs accept a flat peso amount ("1500") OR
// a percentage of the relevant base ("10%", "10 %", "10,5%"). A percentage is
// resolved to whole pesos AT ENTRY against the current base (line gross for a
// line, subtotal for the global) and stored as that peso amount — so it survives
// later qty edits exactly like a typed monto, and the wire still carries a single
// peso `discount` (money discipline: never a rate). The result is clamped into
// `[0, base]` via `clampDiscount`, so a "200%" or a huge monto can't go negative
// or exceed what it discounts.

/** Parse a discount entry that is either flat pesos ("1.500") or a percentage of
 *  `base` ("10%"). Returns integer pesos clamped into `[0, base]`; blank/garbage
 *  → 0. A percentage rounds to the nearest peso (CLP has no minor unit). */
export function parseDiscountEntry(raw: string, base: number): number {
  const s = (raw ?? "").trim();
  if (s === "") return 0;
  const pct = /^(\d+(?:[.,]\d+)?)\s*%$/.exec(s);
  if (pct) {
    const p = Number(pct[1].replace(",", "."));
    if (!Number.isFinite(p) || p <= 0) return 0;
    return clampDiscount(Math.round((base * p) / 100), base);
  }
  // Flat pesos: keep digits only (drop grouping dots/spaces), mirror parseCash.
  const n = Number(s.replace(/[^\d]/g, ""));
  return clampDiscount(Number.isFinite(n) ? n : 0, base);
}

// --- POS hold / recall (ventas en espera) ------------------------------------
// A real counter parks a sale ("voy por la otra cosa") and rings up the next
// customer without losing the cart, then recalls it. We snapshot the whole POS
// draft (cart lines DEEP-COPIED, the global discount, the picked customer) so a
// held sale is frozen — mutating the live cart after holding can't bleed into
// what was parked. Multiple holds coexist; recall pops one back and removes it
// from the parked list (you can't double-recall the same ticket).

/** The loyalty customer attached to a sale, snapshotted into a hold. */
export interface HeldCustomer {
  id: string;
  name: string;
  points: number;
}

/** A parked sale: a frozen snapshot of the POS draft plus an identifier and a
 *  cashier-facing label ("Espera N" by default). */
export interface HeldSale {
  id: string;
  label: string;
  lines: CartLine[];
  globalDiscount: number;
  customer: HeldCustomer | null;
  heldAt: number; // epoch ms — for "hace 3 min" / ordering
}

/** What the view hands `holdSale`: the live draft plus optional overrides
 *  (`label`, and `id`/`heldAt` for deterministic tests). */
export interface HoldDraft {
  lines: CartLine[];
  globalDiscount?: number;
  customer?: HeldCustomer | null;
  label?: string;
  id?: string;
  heldAt?: number;
}

/** Shallow-copy each cart line so a hold is independent of the live cart. Lines
 *  are flat (no nested objects), so a per-line spread is a full deep copy. */
export function cloneCart(cart: CartLine[]): CartLine[] {
  return cart.map((l) => ({ ...l }));
}

/** Suggest the next "Espera N" label given the parked sales — `N` = (max numeric
 *  suffix already used) + 1, so recalling #1 and parking again doesn't reuse #1.
 *  Mirrors `nextDrawerName`'s max-suffix rule. */
export function nextHoldLabel(held: HeldSale[]): string {
  let maxN = 0;
  for (const h of held) {
    const m = /^Espera (\d+)$/.exec(h.label.trim());
    if (m) {
      const n = Number(m[1]);
      if (n > maxN) maxN = n;
    }
  }
  return `Espera ${maxN + 1}`;
}

function makeHoldId(): string {
  return `hold-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

/** Park `draft` as a frozen held sale. Returns the new parked list (the input is
 *  never mutated) and the created sale. An empty cart is the caller's guard — the
 *  pure op parks whatever it's given. */
export function holdSale(held: HeldSale[], draft: HoldDraft): { held: HeldSale[]; sale: HeldSale } {
  const sale: HeldSale = {
    id: draft.id ?? makeHoldId(),
    label: draft.label?.trim() || nextHoldLabel(held),
    lines: cloneCart(draft.lines),
    globalDiscount: Math.max(0, Math.trunc(draft.globalDiscount ?? 0)),
    customer: draft.customer ?? null,
    heldAt: draft.heldAt ?? Date.now(),
  };
  return { held: [...held, sale], sale };
}

/** Pop the parked sale `id` back: returns the remaining parked list (without it)
 *  and the recalled sale, or `null` if no such id (already recalled / unknown).
 *  The recalled `lines` are re-cloned so editing the restored cart can't mutate
 *  any other reference that still points at the old snapshot. */
export function recallSale(
  held: HeldSale[],
  id: string,
): { held: HeldSale[]; sale: HeldSale } | null {
  const found = held.find((h) => h.id === id);
  if (!found) return null;
  const sale: HeldSale = { ...found, lines: cloneCart(found.lines) };
  return { held: held.filter((h) => h.id !== id), sale };
}

// --- POS split / mixed payment (multi-tender) --------------------------------
// The server's pos_mixed accepts cash_amount + card_amount and only requires
// cash + card ≥ total (crate::domain::sales::service). The overpayment falls on
// the cash side (you can't over-charge a card), so vuelto = (cash+card) − total.

/** A split payment: how much is tendered in cash and on card (integer pesos). */
export interface MixedTender {
  cash: number;
  card: number;
}

/** Validate a split payment against the payable total.
 *  - `ok`     → cash+card covers the total; `change` is the vuelto (≥ 0).
 *  - `!ok`    → still short by `short` (> 0); `change` is 0.
 *  `tendered` = cash+card. Negatives floor to 0 so a stray sign can't trick the
 *  balance check. Mirrors the server's `cash + card < total` rejection. */
export function splitPayment(
  tender: MixedTender,
  total: number,
): { ok: boolean; tendered: number; change: number; short: number } {
  const cash = Math.max(0, Math.trunc(Number.isFinite(tender.cash) ? tender.cash : 0));
  const card = Math.max(0, Math.trunc(Number.isFinite(tender.card) ? tender.card : 0));
  const tendered = cash + card;
  if (total <= 0) return { ok: false, tendered, change: 0, short: 0 };
  return tendered >= total
    ? { ok: true, tendered, change: tendered - total, short: 0 }
    : { ok: false, tendered, change: 0, short: total - tendered };
}

/** Remaining pesos to put on card given cash already entered (for mixto UX).
 *  0 when cash already covers the total; never negative. */
export function remainingOnCard(cash: number, total: number): number {
  const c = Math.max(0, Math.trunc(Number.isFinite(cash) ? cash : 0));
  const t = Math.max(0, Math.trunc(Number.isFinite(total) ? total : 0));
  return Math.max(0, t - c);
}

/** Whether a cash-only tender is short (cashier typed a positive amount below
 *  total). Blank/0 is NOT short — that path means "pago exacto" at the till. */
export function cashTenderShort(tendered: number, total: number): boolean {
  const t = Math.max(0, Math.trunc(Number.isFinite(tendered) ? tendered : 0));
  const due = Math.max(0, Math.trunc(Number.isFinite(total) ? total : 0));
  return t > 0 && t < due;
}

// --- caja drawer naming ------------------------------------------------------
// The Business tier runs N drawers per tenant (one per cashier). The open-caja
// modal defaulted to a hard-coded "caja-1" every time, so opening a second
// drawer proposed a name that collided with the first — confusing at arqueo
// ("¿cuál caja-1 cerré?"). Suggest the next free `caja-N` instead.

/** Suggest the next drawer name given the names already open. Picks
 *  `caja-(max numeric suffix + 1)`; with no `caja-N` names open it falls back to
 *  `caja-(count + 1)` so a custom-named drawer still bumps the number. Always
 *  `caja-1` when nothing is open. */
export function nextDrawerName(existing: string[]): string {
  if (existing.length === 0) return "caja-1";
  let maxN = 0;
  for (const name of existing) {
    const m = /^caja-(\d+)$/.exec(name.trim());
    if (m) {
      const n = Number(m[1]);
      if (n > maxN) maxN = n;
    }
  }
  const next = maxN > 0 ? maxN + 1 : existing.length + 1;
  return `caja-${next}`;
}

// --- caja arqueo -------------------------------------------------------------

/** The four cash flows that make up an expected drawer at close. Each may be a
 *  server Decimal string or a number. */
export interface ArqueoInput {
  opening_cash: string | number;
  cash_sales: string | number;
  movements_in: string | number;
  movements_out: string | number;
}

/** Expected cash in the drawer = opening + cash sales + ingresos − egresos.
 *  Mirrors `caja.ts` exactly so the close-modal preview and the tests agree. */
export function expectedCash(a: ArqueoInput): number {
  return (
    toNumber(a.opening_cash) +
    toNumber(a.cash_sales) +
    toNumber(a.movements_in) -
    toNumber(a.movements_out)
  );
}

export type DiscrepancyTone = "ok" | "warn" | "danger";

/** Counted-vs-expected verdict for the arqueo. `amount` is always non-negative
 *  (the sign lives in `tone`/`label`): cuadra (0), sobrante (counted > expected),
 *  faltante (counted < expected). */
export function discrepancy(
  counted: number,
  expected: number,
): { tone: DiscrepancyTone; label: string; amount: number } {
  const diff = counted - expected;
  if (diff === 0) return { tone: "ok", label: "Cuadra", amount: 0 };
  if (diff > 0) return { tone: "warn", label: "Sobrante", amount: diff };
  return { tone: "danger", label: "Faltante", amount: -diff };
}

// --- devoluciones ------------------------------------------------------------

/** A refund line under construction: how many of `sold` units to return. */
export interface RefundDraftLine {
  name: string;
  sold: number;
  qty: number;
  unit_price: string;
  product?: string; // present only when the source line identifies a product
}

/** "Total" only when every sold line is returned in full AND at least one unit
 *  is being returned; otherwise "Parcial". Returning nothing is Parcial, never
 *  a no-op Total. */
export function deriveTipo(lines: RefundDraftLine[]): "total" | "parcial" {
  const anyReturned = lines.some((l) => l.qty > 0);
  const allFull = lines.length > 0 && lines.every((l) => l.qty === l.sold);
  return anyReturned && allFull ? "total" : "parcial";
}

/** The `devolucion.tipo` axis as the server schema defines it
 *  (migrations/0007_sales.surql): the return MOTIVO, NOT the total/parcial scope.
 *  A POS counter return is `venta` (a return against a sale). The old client sent
 *  `total`/`parcial` here, which the schema ASSERT rejected → every return 500'd
 *  (BUG-paul-001). `deriveTipo` (scope) stays presentational; this is what goes
 *  on the wire. */
export type ReturnMotivo = "venta" | "cancelacion" | "garantia" | "error";

/** Default motivo for a normal POS return against a sale. */
export const DEFAULT_RETURN_MOTIVO: ReturnMotivo = "venta";

/** Spanish label for a persisted `devolucion.tipo` motivo. Unknown values pass
 *  through capitalized so a future schema value never renders blank. */
export function returnMotivoLabel(tipo: string): string {
  switch (tipo.toLowerCase()) {
    case "venta":
      return "Venta";
    case "cancelacion":
      return "Cancelación";
    case "garantia":
      return "Garantía";
    case "error":
      return "Error";
    default:
      return tipo ? tipo.charAt(0).toUpperCase() + tipo.slice(1) : "—";
  }
}

/** What a confirmed refund posts: validated lines + derived tipo + total. */
export interface RefundResult {
  items: { product_name: string; quantity: number; unit_price: string; restock: boolean }[];
  tipo: "total" | "parcial";
  total: number;
}

/** Validate the refund draft as the cashier would confirm it. Quantities must be
 *  integers in `0..=sold`; at least one positive quantity is required. `restock`
 *  is honored only when the line identifies a product (boleta lines don't, so it
 *  stays false). Spanish error messages match the modal. */
export function validateRefund(
  lines: RefundDraftLine[],
  opts: { restock?: boolean } = {},
): { ok: true; value: RefundResult } | { ok: false; error: string } {
  const items: RefundResult["items"] = [];
  let total = 0;
  for (const l of lines) {
    if (!Number.isInteger(l.qty) || l.qty < 0) {
      return { ok: false, error: "Las cantidades deben ser enteros ≥ 0." };
    }
    if (l.qty > l.sold) {
      return { ok: false, error: `No puedes devolver más de lo vendido en «${l.name}».` };
    }
    if (l.qty > 0) {
      items.push({
        product_name: l.name,
        quantity: l.qty,
        unit_price: l.unit_price,
        restock: Boolean(opts.restock) && Boolean(l.product),
      });
      total += toNumber(l.unit_price) * l.qty;
    }
  }
  if (items.length === 0) {
    return { ok: false, error: "Ingresa al menos una cantidad a devolver." };
  }
  return { ok: true, value: { items, tipo: deriveTipo(lines), total } };
}

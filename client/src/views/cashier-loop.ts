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
}

/** Anything sellable the picker can hand the cart (a `Product` satisfies it). */
export interface Sellable {
  id: string;
  name: string;
  price: string;
  stock: number;
}

/** Add one unit of `p` to `cart` (in place). Out-of-stock items are rejected;
 *  an existing line increments only while it stays at or below its known stock,
 *  so the cart can never promise more units than the shelf holds. */
export function addToCart(cart: CartLine[], p: Sellable): void {
  if (p.stock <= 0) return;
  const existing = cart.find((l) => l.product === p.id);
  if (existing) {
    if (existing.qty < existing.stock) existing.qty += 1;
  } else {
    cart.push({ product: p.id, name: p.name, unit_price: p.price, qty: 1, stock: p.stock });
  }
}

/** Nudge a line's quantity by `delta` (in place). Hitting 0 (or below) drops the
 *  line; exceeding stock clamps to stock. Returns the resulting cart so callers
 *  can chain, but the mutation is in place to match the view's array identity. */
export function changeQty(cart: CartLine[], id: string, delta: number): CartLine[] {
  const line = cart.find((l) => l.product === id);
  if (!line) return cart;
  line.qty += delta;
  if (line.qty <= 0) {
    cart.splice(cart.indexOf(line), 1);
  } else if (line.qty > line.stock) {
    line.qty = line.stock;
  }
  return cart;
}

/** Running total of the cart in integer pesos. Garbage prices floor to 0 (via
 *  `toNumber`) so the total never reads NaN. */
export function cartTotal(cart: CartLine[]): number {
  return cart.reduce((sum, l) => sum + toNumber(l.unit_price) * l.qty, 0);
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

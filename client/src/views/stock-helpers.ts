// Pure helpers for the stock / money-out views (inventory · compras · gastos).
// NO DOM, NO Tauri imports — only the pure `format` module (node-safe) — so they
// unit-test under vitest's node env (like format.ts) and stay shared without
// coupling the views. The views delegate their inline validation / aggregation /
// status logic here so the user-journey tests exercise the REAL code path.
import { toNumber } from "../format";

/** A `<input type="date">` value (`YYYY-MM-DD`) → RFC3339 the server parses as
 *  `DateTime<Utc>`, anchored at **noon UTC**. Noon (not midnight) so the stored
 *  instant never crosses a day boundary when re-rendered in Chile's local TZ
 *  (UTC-3/-4): `2026-05-01T00:00:00Z` shows as 30-04-2026 in es-CL, but
 *  `2026-05-01T12:00:00Z` stays 01-05-2026. Empty input ⇒ undefined (the server
 *  defaults to "now"). Mirrors the expiry/incurred-at date contract. */
export function toRfc3339Noon(dateInput: string): string | undefined {
  const v = dateInput.trim();
  if (!v) return undefined;
  return `${v}T12:00:00Z`;
}

/** Stock level bucket for the inventory pill. `lowThreshold` is the inclusive
 *  upper bound for "low" (≤ threshold but > 0). Defaults to 5 (the legacy UI
 *  constant). A non-positive stock is always "out". */
export type StockLevel = "out" | "low" | "ok";
export function stockLevel(stock: number, lowThreshold = 5): StockLevel {
  if (!(stock > 0)) return "out"; // also catches NaN / negatives
  return stock <= lowThreshold ? "low" : "ok";
}

/** Advisory expiry verdict for a lote, computed on whole-day boundaries so it
 *  matches the server's `days_to_expiry` (floor of calendar days from `now` to
 *  the expiry date, both at UTC day granularity). `< 0` ⇒ already expired. */
export interface ExpiryStatus {
  days: number;
  expired: boolean;
  tone: "danger" | "warn" | "ok" | "muted";
  label: string;
}
export function expiryStatus(iso: string, now: Date = new Date()): ExpiryStatus {
  const exp = new Date(iso);
  if (Number.isNaN(exp.getTime())) {
    return { days: NaN, expired: false, tone: "muted", label: "—" };
  }
  // Truncate both ends to the UTC calendar day so a same-day expiry reads 0, not
  // a fractional ±1 depending on the time component.
  const DAY = 86_400_000;
  const expDay = Math.floor(exp.getTime() / DAY);
  const nowDay = Math.floor(now.getTime() / DAY);
  const days = expDay - nowDay;
  if (days < 0) return { days, expired: true, tone: "danger", label: "Caducado" };
  if (days <= 30) return { days, expired: false, tone: "warn", label: "Por vencer" };
  return { days, expired: false, tone: "ok", label: "Vigente" };
}

/** Whether the pharmacy-only product fields (laboratorio, principio activo,
 *  tipo de receta) should be shown for a given `business_vertical` admin setting.
 *  Multi-rubro: a minimarket / general store doesn't use them, so they're hidden
 *  there. Anything unknown/unset (incl. a `getSetting` 403/null) defaults to
 *  TRUE — pharmacy is the historical default and the safe back-compat choice. */
export function pharmaFieldsVisible(vertical: string | null | undefined): boolean {
  if (vertical == null) return true;
  const v = vertical.trim().toLowerCase();
  if (v === "") return true;
  // Explicit non-pharmacy verticals hide the clinical fields.
  const nonPharma = new Set([
    "general",
    "minimarket",
    "market",
    "almacen",
    "almacén",
    "retail",
    "otro",
    "other",
  ]);
  return !nonPharma.has(v);
}

// ===========================================================================
//  Compras — purchase-order status, KPIs, line + receipt validation, WAC
// ===========================================================================

/** Canonical server PO statuses (crates/domain/src/purchasing). Lifecycle:
 *  `draft → sent/approved → partially_received → received`, or
 *  `draft → cancelled`. The server emits `partially_received` — NEVER `partial`
 *  (the old client constant `partial` matched nothing → broken filter + KPI). */
export type PoStatus =
  | "draft"
  | "sent"
  | "approved"
  | "partially_received"
  | "received"
  | "cancelled";

// Open = still live (counts toward the "Pendientes" KPI). Excludes received +
// cancelled. partially_received IS open (it can still receive the remainder).
const PO_OPEN: ReadonlySet<string> = new Set([
  "draft",
  "sent",
  "approved",
  "partially_received",
]);

// Statuses a goods-receipt is allowed from — mirrors the receive guard in
// service.rs (`sent | approved | partially_received`). A draft must be issued.
const PO_RECEIVABLE: ReadonlySet<string> = new Set([
  "sent",
  "approved",
  "partially_received",
]);

/** Is this PO still open/pending (not received, not cancelled)? */
export function poIsOpen(status: string): boolean {
  return PO_OPEN.has(status.trim().toLowerCase());
}

/** May a goods-receipt be recorded against a PO in this status? */
export function poIsReceivable(status: string): boolean {
  return PO_RECEIVABLE.has(status.trim().toLowerCase());
}

/** Spanish label + pill tone for a PO status. Unknown → raw value, ok tone
 *  (defensive — server data always hits a known case, so no English leaks). */
export interface PoStatusMeta {
  label: string;
  tone: "ok" | "warn" | "danger";
}
export function poStatusMeta(status: string): PoStatusMeta {
  switch (status.trim().toLowerCase()) {
    case "draft":
      return { label: "Borrador", tone: "warn" };
    case "sent":
      return { label: "Enviada", tone: "warn" };
    case "approved":
      return { label: "Aprobada", tone: "warn" };
    case "partially_received":
      return { label: "Parcial", tone: "warn" };
    case "received":
      return { label: "Recibida", tone: "ok" };
    case "cancelled":
      return { label: "Cancelada", tone: "danger" };
    default:
      return { label: status, tone: "ok" };
  }
}

/** Compras KPI roll-up: order count, open/pending count, summed value. Money is
 *  a Decimal STRING on the wire → summed via toNumber (display only). */
export interface PoKpis {
  total: number;
  pending: number;
  totalValue: number;
}
export function poKpis(rows: { status: string; total: string | number }[]): PoKpis {
  let pending = 0;
  let totalValue = 0;
  for (const r of rows) {
    if (poIsOpen(r.status)) pending++;
    totalValue += toNumber(r.total);
  }
  return { total: rows.length, pending, totalValue };
}

/** Remaining (un-received) quantity across PO items. */
export function poPending(items: { quantity: number; qty_received: number }[]): number {
  return items.reduce((n, it) => n + Math.max(0, it.quantity - it.qty_received), 0);
}

export type ParseResult<T> = { ok: true; value: T } | { ok: false; error: string };

export interface PoLineDraft {
  name: string;
  qty: string | number;
  cost: string;
}
export interface PoLineParsed {
  product_name: string;
  quantity: number;
  unit_cost: string;
}

/** Validate + normalise the raw "Nueva OC" line rows into the items payload the
 *  server expects. Blank rows (no name AND no cost) are skipped. Mirrors the
 *  server guards: qty integer ≥ 1, unit_cost finite ≥ 0, name required. */
export function parsePoLines(rows: PoLineDraft[]): ParseResult<PoLineParsed[]> {
  const items: PoLineParsed[] = [];
  for (const row of rows) {
    const name = String(row.name).trim();
    const costStr = String(row.cost).trim();
    if (name === "" && costStr === "") continue; // skip blank rows
    if (name === "") return { ok: false, error: "Cada línea necesita un nombre de producto." };
    const qty = Number(row.qty);
    if (!Number.isInteger(qty) || qty < 1) {
      return { ok: false, error: "La cantidad debe ser un entero ≥ 1." };
    }
    const costNum = Number(costStr);
    if (costStr === "" || !Number.isFinite(costNum) || costNum < 0) {
      return { ok: false, error: "El costo unitario debe ser un número válido ≥ 0." };
    }
    items.push({ product_name: name, quantity: qty, unit_cost: costStr });
  }
  if (items.length === 0) {
    return { ok: false, error: "Agrega al menos una línea con producto y costo." };
  }
  return { ok: true, value: items };
}

/** Validate one goods-receipt quantity against a line's pending balance.
 *  Integer in `[0, max]`; 0 = "don't receive this line now". null = ok. */
export function validateReceiveQty(qty: number, max: number): string | null {
  if (!Number.isInteger(qty) || qty < 0) return "Las cantidades deben ser enteros ≥ 0.";
  if (qty > max) return "No puedes recibir más de lo pendiente.";
  return null;
}

/** Weighted-average cost recompute — FAITHFUL to the server contract
 *  (crates/domain/src/purchasing/service.rs `receive_purchase_order_lines`):
 *    new_cost = (old_stock·old_cost + Σ(qty·unit_cost)) / (old_stock + Σqty)
 *  Base rule: with NO prior cost (null) OR old_stock ≤ 0, the new cost is the
 *  line average Σ(qty·cost)/Σqty — a first receipt seeds the cost instead of
 *  diluting against a phantom zero. Multiple receipt lines on the same product
 *  are aggregated BEFORE the average (server buckets per product). Full
 *  precision (server uses Decimal); callers round only for display. This is the
 *  executable spec the receive journey asserts — a regression guard if WAC is
 *  ever ported client-side. */
export interface Receipt {
  qty: number;
  unitCost: number;
}
export interface WacResult {
  stock: number;
  cost: number;
}
export function weightedAverageCost(
  oldStock: number,
  oldCost: number | null,
  receipts: Receipt[],
): WacResult {
  const addQty = receipts.reduce((n, r) => n + r.qty, 0);
  const costSum = receipts.reduce((s, r) => s + r.qty * r.unitCost, 0);
  if (addQty <= 0) return { stock: oldStock, cost: oldCost ?? 0 };
  const lineAvg = costSum / addQty;
  const cost =
    oldCost == null || oldStock <= 0
      ? lineAvg
      : (oldStock * oldCost + costSum) / (oldStock + addQty);
  return { stock: oldStock + addQty, cost };
}

// ===========================================================================
//  Inventario — stock-adjust validation
// ===========================================================================

export type AdjustMode = "delta" | "set";
/** Validate an inventory stock adjustment. `delta`: integer ≠ 0 (sum/subtract).
 *  `set`: integer ≥ 0 (absolute). null = ok. */
export function validateStockAdjust(mode: AdjustMode, n: number): string | null {
  if (!Number.isInteger(n)) return "Ingresa una cantidad entera.";
  if (mode === "set" && n < 0) return "El stock fijado no puede ser negativo.";
  if (mode === "delta" && n === 0) return "El ajuste no puede ser cero.";
  return null;
}

// ===========================================================================
//  Gastos — expense validation + money-out roll-ups (caja link)
// ===========================================================================

/** Validate + normalise a new-expense form. Amount must be finite > 0;
 *  description required. Amount → truncated integer CLP STRING (server parses
 *  Decimal). */
export function parseExpense(
  rawAmount: string,
  description: string,
): ParseResult<{ amount: string; description: string }> {
  const n = Number(rawAmount.trim());
  if (rawAmount.trim() === "" || !Number.isFinite(n) || n <= 0) {
    return { ok: false, error: "Ingresa un monto válido (mayor a 0)." };
  }
  const desc = description.trim();
  if (desc === "") return { ok: false, error: "Ingresa una descripción del gasto." };
  return { ok: true, value: { amount: String(Math.trunc(n)), description: desc } };
}

/** Sum expense amounts (Decimal STRINGs) for the gastos total KPI. Display only. */
export function expenseTotal(rows: { amount: string | number }[]): number {
  return rows.reduce((s, r) => s + toNumber(r.amount), 0);
}

/** Cash-out egresos: only `cash`-method expenses leave the physical till and
 *  surface in the caja arqueo — bank/card/transfer don't touch the drawer. The
 *  caja↔gasto link the operator sees at cierre. */
export function cashEgresos(
  rows: { amount: string | number; payment_method: string }[],
): number {
  return rows
    .filter((r) => r.payment_method === "cash")
    .reduce((s, r) => s + toNumber(r.amount), 0);
}

// ===========================================================================
//  LANE B — FEFO near-expiry ordering + ABC rotation
// ===========================================================================

/** FEFO (first-expiry-first-out) consume order: batches sorted soonest-expiry
 *  first. Unparseable dates sort last; stable for equal dates (keeps input
 *  order). The order a perishable/drug should be picked + the order near-expiry
 *  alerts should surface. */
export interface FefoBatch {
  expiry_date: string;
  stock: number;
}
export function fefoOrder<T extends FefoBatch>(batches: T[]): T[] {
  return batches
    .map((b, i) => ({ b, i, t: new Date(b.expiry_date).getTime() }))
    .sort((a, z) => {
      const at = Number.isNaN(a.t) ? Infinity : a.t;
      const zt = Number.isNaN(z.t) ? Infinity : z.t;
      return at - zt || a.i - z.i;
    })
    .map((x) => x.b);
}

/** ABC rotation classification (Pareto). Rank items by `value` desc, then bucket
 *  by CUMULATIVE share: A (top, cum ≤ 80%), B (cum ≤ 95%), C (the long tail).
 *  `value` is whatever the caller ranks on (units sold × price, or movement
 *  volume). Ties keep input order. Empty/zero total → everything C (no rotation
 *  signal yet). Negative values are floored to 0. */
export interface AbcItem {
  id: string;
  value: number;
}
export interface AbcRanked extends AbcItem {
  share: number;
  cumShare: number;
  class: "A" | "B" | "C";
}
export function abcClassify(items: AbcItem[]): AbcRanked[] {
  const total = items.reduce((s, it) => s + Math.max(0, it.value), 0);
  const sorted = items
    .map((it, i) => ({ it, i }))
    .sort((a, z) => Math.max(0, z.it.value) - Math.max(0, a.it.value) || a.i - z.i)
    .map((x) => x.it);
  // Bucket on cumulative VALUE vs threshold·total, not on summed shares: adding
  // normalised fractions drifts past the boundary (0.8 + 0.15 = 0.95000…1 > 0.95)
  // and would mis-bucket an item that lands exactly on 95%.
  let cumVal = 0;
  return sorted.map((it) => {
    const v = Math.max(0, it.value);
    cumVal += v;
    const share = total > 0 ? v / total : 0;
    const cls: "A" | "B" | "C" =
      total === 0 ? "C" : cumVal <= 0.8 * total ? "A" : cumVal <= 0.95 * total ? "B" : "C";
    return { ...it, share, cumShare: total > 0 ? cumVal / total : 0, class: cls };
  });
}

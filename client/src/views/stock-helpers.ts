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

// --- CSV / JSON export (vendor-agnostic: the owner owns their data) ---------

/** Escape one value as an RFC-4180 CSV field: wrap in double quotes and double
 *  any embedded quote when the value contains a comma, quote, CR or LF; otherwise
 *  pass it through. `null`/`undefined` → "". Numbers/booleans are stringified.
 *  CSV-injection guard: a value starting with `= + - @` is prefixed with a tab so
 *  a spreadsheet treats it as text, not a formula (product names are operator
 *  input → an injection vector). */
export function csvField(value: unknown): string {
  if (value == null) return "";
  let s = String(value);
  if (/^[=+\-@]/.test(s)) s = `\t${s}`;
  if (/[",\r\n]/.test(s)) s = `"${s.replace(/"/g, '""')}"`;
  return s;
}

/** Join a header row + data rows into a CSV string (CRLF line endings per
 *  RFC-4180, Excel-friendly). No BOM here — the caller prepends `﻿` for the
 *  download so the in-memory string stays clean for tests / round-trips. */
export function toCsv(
  header: readonly string[],
  rows: readonly (readonly unknown[])[],
): string {
  return [header, ...rows].map((r) => r.map(csvField).join(",")).join("\r\n");
}

/** The subset of a product needed to export it (a superset of `api.Product`). */
export interface ExportProduct {
  id: string;
  name: string;
  price: string;
  stock: number;
  active: boolean;
  laboratory?: string | null;
  active_ingredient?: string | null;
}

/** A built export ready to download: the CSV + pretty JSON strings, the row
 *  `count`, and `truncated` = the fetch hit its page cap (more data exists). */
export interface ExportBundle {
  csv: string;
  json: string;
  count: number;
  truncated: boolean;
}

/** Build the inventory export (CSV + pretty JSON) for a product list.
 *  Multi-rubro: `includePharma` adds the pharmacy-only columns (laboratorio,
 *  principio activo) — a minimarket / general vertical omits them so the file
 *  carries only the fields that rubro uses. `cap`, when given, is the page size
 *  the fetch was limited to; a list that fills it exactly is flagged `truncated`
 *  (the server caps a single product page). CSV headers are Spanish (operator);
 *  the JSON keeps stable snake_case keys (machine round-trip). Money stays the
 *  raw Decimal STRING — no locale formatting → re-imports losslessly. */
export function buildInventoryExport(
  products: readonly ExportProduct[],
  includePharma: boolean,
  cap?: number,
): ExportBundle {
  const header = ["id", "nombre", "precio", "stock", "activo"];
  if (includePharma) header.push("laboratorio", "principio_activo");
  const rows = products.map((p) => {
    const row: unknown[] = [p.id, p.name, p.price, p.stock, p.active ? "sí" : "no"];
    if (includePharma) row.push(p.laboratory ?? "", p.active_ingredient ?? "");
    return row;
  });
  const json = JSON.stringify(
    products.map((p) => {
      const base: Record<string, unknown> = {
        id: p.id,
        name: p.name,
        price: p.price,
        stock: p.stock,
        active: p.active,
      };
      if (includePharma) {
        base.laboratory = p.laboratory ?? null;
        base.active_ingredient = p.active_ingredient ?? null;
      }
      return base;
    }),
    null,
    2,
  );
  return {
    csv: toCsv(header, rows),
    json,
    count: products.length,
    truncated: cap != null && products.length >= cap,
  };
}

/** Filename stem for an export, e.g. `inventario-2026-06-14`, in the operator's
 *  local (CL) calendar day. The caller appends the extension. */
export function exportFilename(prefix: string, now: Date = new Date()): string {
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${prefix}-${y}-${m}-${d}`;
}

// --- empty / error states (never a blank screen) ----------------------------

/** Operator-facing copy for an empty list. `filtered` ⇒ a search/filter is
 *  active, so it's "no matches" (the data may still be there) not "nothing yet".
 *  `cta`, when present, labels the action that resolves the empty — the view
 *  wires it to a button so the operator always has a way forward. */
export interface EmptyCopy {
  title: string;
  hint: string;
  cta?: string;
}

export function inventoryEmpty(filtered: boolean): EmptyCopy {
  return filtered
    ? {
        title: "Sin coincidencias",
        hint: "Ningún producto coincide con tu búsqueda. Prueba con otro término.",
      }
    : {
        title: "Aún no hay productos",
        hint: "Crea tu primer producto para controlar stock, lotes y vencimientos.",
        cta: "+ Nuevo producto",
      };
}

export function comprasEmpty(filtered: boolean): EmptyCopy {
  return filtered
    ? {
        title: "Sin órdenes para ese filtro",
        hint: "Ninguna orden de compra coincide con el estado seleccionado.",
      }
    : {
        title: "Aún no hay órdenes de compra",
        hint: "Crea una orden para registrar lo que pides a tus proveedores.",
        cta: "+ Nueva OC",
      };
}

export function gastosEmpty(filtered: boolean): EmptyCopy {
  return filtered
    ? {
        title: "Sin gastos para ese filtro",
        hint: "Ningún gasto coincide con la categoría seleccionada.",
      }
    : {
        title: "Aún no hay gastos",
        hint: "Registra tus egresos y caja chica para llevar el control del día.",
        cta: "Nuevo gasto",
      };
}

/** What kind of failure a fetch hit, for operator-facing copy. */
export type FetchErrorKind = "forbidden" | "offline" | "generic";

export interface ErrorCopy {
  kind: FetchErrorKind;
  title: string;
  hint: string;
}

/** Classify a fetch failure (the api layer rejects with a Spanish string) into
 *  operator copy. A permission problem (403 / "denegado" / "permiso") → "sin
 *  acceso"; a connection / network failure (the Tauri `conn_error` text, or a
 *  timeout) → "sin conexión" with a retry hint; anything else → the raw message
 *  so no real error is swallowed. `resource` customizes the forbidden line
 *  ("a las compras"). Never throws — safe to call in any catch. */
export function classifyFetchError(err: unknown, resource = "esta sección"): ErrorCopy {
  const msg = typeof err === "string" ? err : "";
  const low = msg.toLowerCase();
  if (low.includes("403") || low.includes("denegado") || low.includes("permiso")) {
    return {
      kind: "forbidden",
      title: "Sin acceso",
      hint: `Tu rol no tiene permiso para ver ${resource}. Contacta al administrador.`,
    };
  }
  if (
    low.includes("no se pudo conectar") ||
    low.includes("error de red") ||
    low.includes("conexión") ||
    low.includes("conexion") ||
    low.includes("timeout") ||
    low.includes("timed out")
  ) {
    return {
      kind: "offline",
      title: "Sin conexión al servidor",
      hint: "No se pudo conectar a pharma-server. Verifica que esté corriendo e inténtalo de nuevo.",
    };
  }
  return {
    kind: "generic",
    title: "No se pudo cargar",
    hint: msg || "Ocurrió un error al cargar la información. Inténtalo de nuevo.",
  };
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

/** May a PO be ISSUED to the supplier (draft → sent) in this status? Only a
 *  `draft` can be sent. The server enforces the same guard (409 otherwise) —
 *  this gates the "Enviar al proveedor" action so the operator can walk
 *  draft → sent → recibir without leaving the app. Receiving a draft directly
 *  is a 409 (BUG-bob-002), so the send step is the bridge to a receivable PO. */
export function poIsSendable(status: string): boolean {
  return status.trim().toLowerCase() === "draft";
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

// ===========================================================================
//  LANE B — view presentation logic (inventory.ts delegates to these so the
//  ordering / highlight / reorder / rotation decisions the operator SEES are
//  single-source and unit-tested, not buried inline in the render).
// ===========================================================================

/** A near-expiry row as the view orders + highlights it. The server's
 *  `near-expiry` feed is not guaranteed sorted; the operator must see the most
 *  urgent lote FIRST — expired (most overdue first), then soonest to expire.
 *  `tone`/`label` are precomputed here so the row renderer is dumb. Vertical-
 *  agnostic: a caducated leche and a caducated fármaco surface identically. */
export interface NearExpiryView {
  tone: "danger" | "warn" | "ok";
  label: string;
}
export function nearExpiryView<T extends { days_to_expiry: number; expired: boolean }>(
  rows: T[],
): (T & NearExpiryView)[] {
  return rows
    .map((r, i) => ({ r, i }))
    .sort((a, z) => a.r.days_to_expiry - z.r.days_to_expiry || a.i - z.i)
    .map(({ r }) => {
      const expired = r.expired || r.days_to_expiry < 0;
      const tone: NearExpiryView["tone"] = expired ? "danger" : r.days_to_expiry <= 30 ? "warn" : "ok";
      const label = expired ? "Caducado" : "Por vencer";
      return { ...r, tone, label };
    });
}

/** Default restock target for the reorder suggestion: how many units back on the
 *  shelf a "Reponer" action proposes. The min-stock alert (stockLevel "low"/"out")
 *  tells the operator WHAT to reorder; this tells them HOW MUCH. Conservative
 *  multiple of the low threshold so a one-off restock clears the alert band. */
export const REORDER_TARGET = 20;

/** Units to buy to bring `stock` back up to `target`. Never negative; an item at
 *  or above target suggests 0 (no reorder). NaN/junk stock → full target. */
export function reorderSuggestion(stock: number, target = REORDER_TARGET): number {
  const s = Number.isFinite(stock) ? stock : 0;
  return Math.max(0, Math.ceil(target - s));
}

/** The reorder worklist: products at/below the low-stock threshold, each with a
 *  suggested buy-back quantity, ordered by urgency (agotado first, then lowest
 *  stock first; ties keep input order). Items with healthy stock are excluded —
 *  no noise. Vertical-agnostic: works for fármacos and abarrotes alike. */
export interface ReorderItem<T> {
  item: T;
  stock: number;
  level: StockLevel;
  suggest: number;
}
export function reorderList<T extends { stock: number }>(
  items: T[],
  lowThreshold = 5,
  target = REORDER_TARGET,
): ReorderItem<T>[] {
  return items
    .map((item, i) => ({ item, i, level: stockLevel(item.stock, lowThreshold) }))
    .filter((x) => x.level !== "ok")
    .sort((a, z) => a.item.stock - z.item.stock || a.i - z.i)
    .map(({ item, level }) => ({
      item,
      stock: item.stock,
      level,
      suggest: reorderSuggestion(item.stock, target),
    }));
}

/** Rotation rows ranked for the "Rotación" view: ABC (Pareto) over units sold in
 *  the period. Free-tier rotation signal computed client-side from the
 *  `stock-rotation` feed (qty_sold per product) — what moves (A) vs dead stock
 *  (C). `sharePct` is the participation as a whole-number percent STRING for the
 *  UI. Sorted by qty_sold desc (abcClassify's order). Never-sold items → class C,
 *  0%. Vertical-agnostic. */
export interface RotacionRow {
  id: string;
  name: string;
  qty_sold: number;
  current_stock: number;
  class: "A" | "B" | "C";
  sharePct: string;
}
export function rotacionRows(
  rows: { product_id: string; product_name: string; qty_sold: number; current_stock: number }[],
): RotacionRow[] {
  const ranked = abcClassify(rows.map((r) => ({ id: r.product_id, value: r.qty_sold })));
  const byId = new Map(rows.map((r) => [r.product_id, r]));
  return ranked.map((r) => {
    const src = byId.get(r.id)!;
    return {
      id: r.id,
      name: src.product_name,
      qty_sold: src.qty_sold,
      current_stock: src.current_stock,
      class: r.class,
      sharePct: `${Math.round(r.share * 100)}%`,
    };
  });
}

/** Per-frame render budget for the two UNBOUNDED inventory lists. The product
 *  (`/products`), purchase-order and expense feeds are already server-capped
 *  (PAGE_LIMIT 60/100); the near-expiry and rotation feeds are NOT — at 50k SKUs
 *  they return tens of thousands of rows, and `host.innerHTML = rows.map(...).
 *  join("")` then forces the browser to parse + lay out every <tr> in a single
 *  frame → multi-hundred-ms jank that violates the POS/UX budget. Both feeds
 *  arrive PRE-ORDERED by what the operator acts on (FEFO urgency / ABC share),
 *  so only the head is ever clinically relevant: cap to the top N, show the
 *  total, and point at export for the long tail. 200 keeps the DOM node count
 *  bounded while still showing far more than a screen-full. */
export const LIST_RENDER_CAP = 200;

/** Top-N window over an already-ordered list. Returns the original array
 *  untouched when it's within budget; otherwise the head slice plus the true
 *  total so the footer can say "mostrando los N de M". Pure: never mutates. */
export interface CappedRows<T> {
  rows: T[];
  total: number;
  truncated: boolean;
}
export function capRows<T>(rows: T[], limit = LIST_RENDER_CAP): CappedRows<T> {
  if (rows.length <= limit) return { rows, total: rows.length, truncated: false };
  return { rows: rows.slice(0, limit), total: rows.length, truncated: true };
}

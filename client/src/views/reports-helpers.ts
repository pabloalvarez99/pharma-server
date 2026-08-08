// Pure logic behind the Reportes view (reports.ts delegates to these): the row
// picked for "hoy", the Pro-gate classification for márgenes, the ABC/rotación
// row shaping, and the vendor-agnostic CSV/JSON export bundles. Extracting it
// keeps what the dueño/contador SEES and EXPORTS regression-tested without a
// webview — the same single-source discipline as cashier-loop.ts / stock-helpers.ts.
//
// Money/percent fields stay the raw server Decimal STRING in exports (no locale
// formatting) so a CSV/JSON re-imports losslessly; the view layer formats for
// display via ../format. CSV plumbing (RFC-4180 escaping + injection guard) is
// reused from stock-helpers so reports and inventory export identically.
import { parseSaleError } from "../api";
import type {
  DailySalesRow,
  DailyMarginRow,
  TopProductRow,
  StockRotationRow,
  InventorySummary,
  PurchaseBook,
} from "../api";
import { toCsv, type ExportBundle } from "./stock-helpers";

/** Pick the most recent row whose `date` matches `today` (UTC YYYY-MM-DD); fall
 *  back to the last row the server returned so a TZ edge never blanks the panel.
 *  `today` is injectable for deterministic tests. */
export function pickTodayRow<T extends { date: string }>(
  rows: readonly T[],
  today: string = new Date().toISOString().slice(0, 10),
): T | undefined {
  if (rows.length === 0) return undefined;
  return rows.find((r) => r.date === today) ?? rows[rows.length - 1];
}

/** Classify a márgenes-report rejection. Free rejects with
 *  `"FEATURE_REQUIRES_UPGRADE|msg"` → `gated:true` so the view shows a calm
 *  upsell card (no crash, no dark pattern); any other error → `gated:false`
 *  (a real failure to surface). Pure mirror of the Pro-gate the server enforces. */
export function classifyMarginError(err: unknown): {
  gated: boolean;
  message: string;
} {
  const { code, message } = parseSaleError(err);
  return { gated: code === "FEATURE_REQUIRES_UPGRADE", message };
}

/** Normalize a server ABC label to the `a|b|c` css/badge token (defensive
 *  against case / stray whitespace). Unknown → `c` (least-urgent bucket). */
export function abcToken(abcClass: string): "a" | "b" | "c" {
  const c = abcClass.trim().toLowerCase();
  return c === "a" || c === "b" ? c : "c";
}

/** Display strings for one rotación row: turnover as `N×` and días de inventario
 *  rounded to an integer, both `—` when the server couldn't compute them (null).
 *  Kept here (not in the view) so the "—" fallbacks are tested. */
export function rotationDisplay(r: StockRotationRow): {
  turnover: string;
  days: string;
} {
  const turnover = r.turnover ? `${r.turnover}×` : "—";
  const days =
    r.days_of_inventory != null && r.days_of_inventory !== ""
      ? String(Math.round(Number(r.days_of_inventory)))
      : "—";
  return { turnover, days };
}

// --- Exports (vendor-agnostic CSV + pretty JSON) -----------------------------
// Each builder returns an ExportBundle (reused from stock-helpers): a Spanish-
// header CSV for the operator's spreadsheet + a stable snake_case JSON for a
// machine round-trip. No lock-in: raw money strings, no proprietary format.

/** Ventas diarias → CSV + JSON. Headers Spanish; money stays raw STRING. */
export function buildSalesExport(rows: readonly DailySalesRow[]): ExportBundle {
  const header = ["fecha", "boletas", "ingresos", "efectivo", "tarjeta"];
  const body = rows.map((r) => [r.date, r.orders, r.revenue, r.cash, r.card]);
  return {
    csv: toCsv(header, body),
    json: JSON.stringify(rows, null, 2),
    count: rows.length,
    truncated: false,
  };
}

/** Top productos (ranking Pareto/ABC) → CSV + JSON. */
export function buildTopExport(rows: readonly TopProductRow[]): ExportBundle {
  const header = ["rank", "producto", "unidades", "ingresos", "pct_ingresos", "abc"];
  const body = rows.map((r) => [
    r.rank,
    r.product_name,
    r.qty_sold,
    r.revenue,
    r.revenue_pct,
    abcToken(r.abc_class).toUpperCase(),
  ]);
  return {
    csv: toCsv(header, body),
    json: JSON.stringify(rows, null, 2),
    count: rows.length,
    truncated: false,
  };
}

/** Rotación de stock → CSV + JSON. Nullable turnover/días export as empty. */
export function buildRotationExport(
  rows: readonly StockRotationRow[],
): ExportBundle {
  const header = ["producto", "vendidas", "stock", "rotacion", "dias_inventario"];
  const body = rows.map((r) => [
    r.product_name,
    r.qty_sold,
    r.current_stock,
    r.turnover ?? "",
    r.days_of_inventory ?? "",
  ]);
  return {
    csv: toCsv(header, body),
    json: JSON.stringify(rows, null, 2),
    count: rows.length,
    truncated: false,
  };
}

/** Every panel the Reportes view has loaded, for the "Exportar todo" bundle. A
 *  panel absent (still loading / gated / failed) is simply omitted. */
/** Libro de compras → CSV/JSON. El CSV lleva las columnas que pide el contador
 *  (fecha, tipo DTE, folio, proveedor con RUT, neto, IVA, total) más `estimado`
 *  para distinguir las filas cuya factura aún no se capturó. */
export function buildLibroExport(book: PurchaseBook): ExportBundle {
  const header = ["fecha", "tipo", "folio", "proveedor", "rut", "neto", "iva", "total", "estimado"];
  const body = book.rows.map((r) => [
    r.date,
    r.tipo,
    r.folio ?? "",
    r.supplier_name,
    r.supplier_rut ?? "",
    r.neto,
    r.iva,
    r.total,
    r.declared ? "no" : "si",
  ]);
  return {
    csv: toCsv(header, body),
    json: JSON.stringify(book, null, 2),
    count: book.rows.length,
    truncated: false,
  };
}

export interface LoadedReports {
  sales?: readonly DailySalesRow[];
  margins?: readonly DailyMarginRow[];
  margins_gated?: boolean;
  top?: readonly TopProductRow[];
  inventory?: InventorySummary;
  rotation?: readonly StockRotationRow[];
  /** Libro de compras del mes (V3) — alimenta el export CSV/JSON. */
  libro?: PurchaseBook;
}

/** Combined machine-readable JSON of all loaded reports — the headline "exporta
 *  todo" (vendor-agnostic, raw values). `generated_at` stamps the snapshot;
 *  gated/unloaded panels are omitted, and `margins_gated` records that márgenes
 *  was Pro-locked (not an error) so the file is unambiguous. */
export function buildReportsJson(
  data: LoadedReports,
  now: Date = new Date(),
): string {
  const bundle: Record<string, unknown> = { generated_at: now.toISOString() };
  if (data.sales) bundle.sales_daily = data.sales;
  if (data.margins) bundle.margins_daily = data.margins;
  else if (data.margins_gated) bundle.margins_daily = { gated: true };
  if (data.top) bundle.top_products = data.top;
  if (data.inventory) bundle.inventory = data.inventory;
  if (data.rotation) bundle.stock_rotation = data.rotation;
  return JSON.stringify(bundle, null, 2);
}

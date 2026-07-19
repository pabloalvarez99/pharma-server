// Reports wrappers (client/src-tauri/src/commands/reports.rs).
// Money/percent fields are STRINGS throughout.
import { invoke } from "@tauri-apps/api/core";

/** One day of POS sales. Money fields (`revenue`/`cash`/`card`) are STRINGS. */
export interface DailySalesRow {
  date: string;
  orders: number;
  revenue: string;
  cash: string;
  card: string;
}

/** Pareto ABC ranking row. `revenue`/`revenue_pct` STRINGS; `abc_class` A|B|C. */
export interface TopProductRow {
  rank: number;
  product_id: string | null;
  product_name: string;
  qty_sold: number;
  revenue: string;
  revenue_pct: string;
  abc_class: string;
}

/** GET /api/v1/reports/sales-daily (Bearer). */
export function salesDaily(serverUrl: string): Promise<DailySalesRow[]> {
  return invoke<DailySalesRow[]>("sales_daily", { serverUrl });
}

/** GET /api/v1/reports/top-products?limit=N (Bearer). */
export function topProducts(
  serverUrl: string,
  limit?: number,
): Promise<TopProductRow[]> {
  return invoke<TopProductRow[]>("top_products", { serverUrl, limit });
}

/** One day of gross-margin data (`DailyMarginRow`). Money/percent fields are
 *  STRINGS. Pro-gated: the command rejects with `"FEATURE_REQUIRES_UPGRADE|msg"`
 *  on Free — split it with `parseSaleError` from `./pos`. */
export interface DailyMarginRow {
  date: string;
  revenue: string;
  cost: string;
  margin: string;
  margin_pct: string;
  items_without_cost: number;
}

/** One product's rotation/turnover row (`StockRotationRow`). `turnover` /
 *  `days_of_inventory` are nullable STRINGS (null when not computable). */
export interface StockRotationRow {
  product_id: string;
  product_name: string;
  qty_sold: number;
  current_stock: number;
  turnover: string | null;
  days_of_inventory: string | null;
}

/** GET /api/v1/reports/margins-daily (Bearer, Pro-gated). Rejects with a
 *  `"CODE|message"` string — `FEATURE_REQUIRES_UPGRADE` on Free. */
export function marginsDaily(
  serverUrl: string,
  from?: string,
  to?: string,
): Promise<DailyMarginRow[]> {
  return invoke<DailyMarginRow[]>("margins_daily", { serverUrl, from, to });
}

/** GET /api/v1/reports/stock-rotation (Bearer). */
export function stockRotation(
  serverUrl: string,
  from?: string,
  to?: string,
): Promise<StockRotationRow[]> {
  return invoke<StockRotationRow[]>("stock_rotation", { serverUrl, from, to });
}

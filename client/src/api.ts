// Typed wrappers around the Rust Tauri commands (client/src-tauri/src/lib.rs).
// Field shapes mirror the server contract in crates/api.
import { invoke } from "@tauri-apps/api/core";

export interface SessionInfo {
  user_id: string;
  tenant_id: string;
  roles: string[];
  expires_in: number;
}

export interface LicenseSummary {
  tier: string;
  status: string; // "active" | "grace" | "expired"
  license_id: string;
  features: string[];
  expires_at: string | null;
  key_id: string;
  seat_count: number;
}

export interface HealthInfo {
  status: string; // "ok" | "degraded"
  db: string;
  reachable: boolean;
}

/** A catalog product (trimmed projection from the server `ProductDto`).
 *  `price` is a STRING — money crosses the wire as `rust_decimal::serde::str`. */
export interface Product {
  id: string;
  name: string;
  price: string;
  stock: number;
  active: boolean;
  laboratory: string | null;
  active_ingredient: string | null;
}

/** `/products/stats` payload. `inventory_value` is a STRING (Decimal). */
export interface InventorySummary {
  total: number;
  active: number;
  low_stock: number;
  out_of_stock: number;
  inventory_value: string;
  expired: number;
}

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

/** One cart line for `pos_sale`. `unit_price` is a STRING per server contract. */
export interface PosItem {
  product: string;
  product_name: string;
  quantity: number;
  unit_price: string;
}

/** POS payment methods the server accepts on the counter. */
export type PaymentMethod = "pos_cash" | "pos_debit" | "pos_credit";

/** POST /api/v1/login + GET /api/v1/me. Throws a Spanish error string on failure. */
export function login(
  serverUrl: string,
  tenant: string,
  email: string,
  password: string,
): Promise<SessionInfo> {
  return invoke<SessionInfo>("login", {
    serverUrl,
    tenant,
    email,
    password,
  });
}

/** GET /api/v1/admin/license/status (Bearer, in-memory token). */
export function licenseStatus(serverUrl: string): Promise<LicenseSummary> {
  return invoke<LicenseSummary>("license_status", { serverUrl });
}

/** GET /health/ready. */
export function serverHealth(serverUrl: string): Promise<HealthInfo> {
  return invoke<HealthInfo>("server_health", { serverUrl });
}

/** Forget the in-memory JWT. */
export function logout(): Promise<void> {
  return invoke<void>("logout");
}

/** GET /api/v1/products (Bearer). `search` + `limit` are optional filters. */
export function listProducts(
  serverUrl: string,
  search?: string,
  limit?: number,
): Promise<Product[]> {
  return invoke<Product[]>("list_products", { serverUrl, search, limit });
}

/** GET /api/v1/products/stats (Bearer) — inventory KPIs. */
export function inventorySummary(serverUrl: string): Promise<InventorySummary> {
  return invoke<InventorySummary>("inventory_summary", { serverUrl });
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

/** A POS sale error surfaced from the Tauri layer as `"CODE|message"`. */
export interface SaleError {
  code: string; // "" when the server sent no envelope
  message: string;
}

/** Split the `"CODE|message"` string the `pos_sale` command rejects with. */
export function parseSaleError(err: unknown): SaleError {
  const raw = typeof err === "string" ? err : "Error inesperado al cobrar.";
  const i = raw.indexOf("|");
  if (i === -1) return { code: "", message: raw };
  return { code: raw.slice(0, i), message: raw.slice(i + 1) };
}

/** A low-stock alert the server attaches to a sale response (`LowStockAlert`). */
export interface LowStockAlert {
  product: string;
  product_name: string;
  stock: number;
  threshold: number;
}

/** Raw shape of the `pos_sale` JSON we actually read (the server returns more). */
interface RawSaleResponse {
  order?: { id?: string };
  loyalty_points_awarded?: number;
  low_stock_alerts?: LowStockAlert[];
}

/** Narrowed result of a successful sale: the order id (for the receipt fetch),
 *  loyalty points awarded, and any low-stock alerts to surface afterwards. */
export interface PosSaleResult {
  orderId: string;
  loyaltyPointsAwarded: number;
  lowStockAlerts: LowStockAlert[];
}

/** POST /api/v1/pos/sale (Bearer + fresh Idempotency-Key minted in Rust).
 *  `customer` is an optional record id — when present the server links the sale
 *  and awards loyalty points. Rejects with a `"CODE|message"` string — use
 *  {@link parseSaleError}. */
export async function posSale(
  serverUrl: string,
  items: PosItem[],
  paymentMethod: PaymentMethod,
  cashAmount?: string,
  cardAmount?: string,
  customer?: string,
): Promise<PosSaleResult> {
  const res = await invoke<RawSaleResponse>("pos_sale", {
    serverUrl,
    items,
    paymentMethod,
    cashAmount,
    cardAmount,
    customer,
  });
  return {
    orderId: res?.order?.id ?? "",
    loyaltyPointsAwarded: res?.loyalty_points_awarded ?? 0,
    lowStockAlerts: Array.isArray(res?.low_stock_alerts) ? res.low_stock_alerts : [],
  };
}

/** One printable receipt line (`ReceiptItem`). Money fields are STRINGS. */
export interface ReceiptItem {
  name: string;
  qty: number;
  unit_price: string;
  line_total: string;
}

/** Printable boleta for a completed sale (`ReceiptDto`). Money is STRING;
 *  `cash_amount`/`card_amount`/`change` are null on tenders they don't apply to. */
export interface Receipt {
  order_id: string;
  folio_or_number: string;
  datetime: string;
  tenant_name: string;
  items: ReceiptItem[];
  subtotal: string;
  discount: string;
  total: string;
  payment_method: string;
  cash_amount: string | null;
  card_amount: string | null;
  change: string | null;
  loyalty_points_awarded: number;
  cashier: string | null;
  footer_note: string;
}

/** GET /api/v1/orders/{id}/receipt (Bearer) — boleta for a completed sale. */
export function getReceipt(serverUrl: string, id: string): Promise<Receipt> {
  return invoke<Receipt>("get_receipt", { serverUrl, id });
}

// --- caja / cash register --------------------------------------------------

/** A cash register session (`/cash-sessions`). Money fields are STRINGS
 *  (Decimal). The closing/discrepancy fields are null while `status === "open"`. */
export interface CashSession {
  id: string;
  user: string;
  register_name: string;
  opening_cash: string;
  opening_notes: string | null;
  closing_cash_counted: string | null;
  closing_cash_expected: string | null;
  discrepancia: string | null;
  closing_notes: string | null;
  opened_at: string;
  closed_at: string | null;
  status: string; // "open" | "closed"
}

/** Close summary / arqueo payload (`CloseSummary`). Money fields are STRINGS. */
export interface CashCloseSummary {
  session: CashSession;
  cash_sales: string;
  movements_in: string;
  movements_out: string;
}

/** GET /api/v1/cash-sessions (Bearer). `status`/`limit` optional filters. */
export function cashSessions(
  serverUrl: string,
  status?: string,
  limit?: number,
): Promise<CashSession[]> {
  return invoke<CashSession[]>("cash_sessions", { serverUrl, status, limit });
}

/** POST /api/v1/cash-sessions (Bearer) — open a register. `openingCash` STRING. */
export function openCashSession(
  serverUrl: string,
  registerName: string,
  openingCash: string,
  notes?: string,
): Promise<CashSession> {
  return invoke<CashSession>("open_cash_session", {
    serverUrl,
    registerName,
    openingCash,
    notes,
  });
}

/** GET /api/v1/cash-sessions/{id}/arqueo (Bearer) — non-mutating close preview. */
export function cashArqueo(
  serverUrl: string,
  id: string,
): Promise<CashCloseSummary> {
  return invoke<CashCloseSummary>("cash_arqueo", { serverUrl, id });
}

/** POST /api/v1/cash-sessions/{id}/close (Bearer). `closingCashCounted` STRING. */
export function closeCashSession(
  serverUrl: string,
  id: string,
  closingCashCounted: string,
  notes?: string,
): Promise<CashCloseSummary> {
  return invoke<CashCloseSummary>("close_cash_session", {
    serverUrl,
    id,
    closingCashCounted,
    notes,
  });
}

// --- clientes / customers --------------------------------------------------

/** A customer search result (`CustomerDto`). */
export interface Customer {
  id: string;
  name: string;
  rut: string | null;
  phone: string | null;
  email: string | null;
  loyalty_points: number;
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** Customer detail w/ lifetime aggregates (`CustomerDetailDto`). `total_spent`
 *  is a STRING (Decimal). Served by feat/customers-loyalty-history. */
export interface CustomerDetail {
  id: string;
  name: string;
  rut: string | null;
  phone: string | null;
  email: string | null;
  loyalty_points: number;
  total_spent: string;
  visit_count: number;
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** One purchase-history row (`CustomerOrderDto`). `total` is a STRING (Decimal). */
export interface CustomerOrder {
  id: string;
  status: string;
  payment_method: string;
  total: string;
  items_count: number;
  created_at: string;
}

/** Sentinel the customer commands reject with when the server lacks the
 *  `/customers/*` surface (404). Matches `CUSTOMERS_MISSING` in lib.rs — the
 *  Clientes view shows an upgrade note instead of a hard error. */
export const CUSTOMERS_MODULE_MISSING = "CUSTOMERS_MODULE_MISSING";

/** GET /api/v1/customers/search?q= (Bearer). Rejects with
 *  {@link CUSTOMERS_MODULE_MISSING} when the endpoint is not deployed (404). */
export function customerSearch(
  serverUrl: string,
  q: string,
): Promise<Customer[]> {
  return invoke<Customer[]>("customer_search", { serverUrl, q });
}

/** GET /api/v1/customers/{id} (Bearer). Rejects with
 *  {@link CUSTOMERS_MODULE_MISSING} when not deployed (404). */
export function customerDetail(
  serverUrl: string,
  id: string,
): Promise<CustomerDetail> {
  return invoke<CustomerDetail>("customer_detail", { serverUrl, id });
}

// --- purchase orders (compras) --------------------------------------------

/** Header-only projection of a purchase order (`PurchaseOrderDto`). `total`
 *  is a STRING (Decimal). `items` is omitted — list returns headers only. */
export interface PurchaseOrder {
  id: string;
  supplier: string;
  status: string; // "draft" | "sent" | "received" | "partial" | "cancelled"
  currency: string;
  total: string;
  notes: string | null;
  external_ref: string | null;
  created_at: string;
  updated_at: string;
}

/** GET /api/v1/purchase-orders (Bearer, cashier+). `status` / `limit` optional. */
export function listPurchaseOrders(
  serverUrl: string,
  status?: string,
  limit?: number,
): Promise<PurchaseOrder[]> {
  return invoke<PurchaseOrder[]>("list_purchase_orders", { serverUrl, status, limit });
}

/** GET /api/v1/customers/{id}/history?limit=N (Bearer). Rejects with
 *  {@link CUSTOMERS_MODULE_MISSING} when not deployed (404). */
export function customerHistory(
  serverUrl: string,
  id: string,
  limit?: number,
): Promise<CustomerOrder[]> {
  return invoke<CustomerOrder[]>("customer_history", { serverUrl, id, limit });
}

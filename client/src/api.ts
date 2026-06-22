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

/** POS payment methods the server accepts on the counter. `pos_mixed` is a
 *  split tender (cash + card) — for it `posSale` sends both `cashAmount` and
 *  `cardAmount` and the server only requires their sum to cover the total. */
export type PaymentMethod = "pos_cash" | "pos_debit" | "pos_credit" | "pos_mixed";

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

/** GET /api/v1/setup/status — UNAUTHENTICATED. `needs_setup=true` on a fresh
 *  install with no account yet (login screen offers in-app account creation). */
export interface SetupStatusInfo {
  needs_setup: boolean;
}
export function setupStatus(serverUrl: string): Promise<SetupStatusInfo> {
  return invoke<SetupStatusInfo>("setup_status", { serverUrl });
}

/** Input for the first-run account-creation form. */
export interface SetupInput {
  businessName: string;
  /** Optional branch slug; server derives one from the name when omitted. */
  tenantSlug?: string;
  email: string;
  password: string;
  /** Chosen rubro (e.g. "farmacia" | "minimarket" | "otro"). */
  vertical?: string;
}

/** A live session plus the slug the server assigned (for "Sucursal" pre-fill). */
export interface SetupSession extends SessionInfo {
  tenant_slug: string;
}

/** POST /api/v1/setup — create the first tenant+owner, then log straight in.
 *  Throws a Spanish error string on failure (e.g. 409 if already configured). */
export function setupAccount(serverUrl: string, input: SetupInput): Promise<SetupSession> {
  return invoke<SetupSession>("setup_account", {
    serverUrl,
    businessName: input.businessName,
    tenantSlug: input.tenantSlug ?? null,
    email: input.email,
    password: input.password,
    vertical: input.vertical ?? null,
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

/** One day of gross-margin data (`DailyMarginRow`). Money/percent fields are
 *  STRINGS. Pro-gated: the command rejects with `"FEATURE_REQUIRES_UPGRADE|msg"`
 *  on Free — split it with {@link parseSaleError}. */
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
  discount?: string,
): Promise<PosSaleResult> {
  const res = await invoke<RawSaleResponse>("pos_sale", {
    serverUrl,
    items,
    paymentMethod,
    cashAmount,
    cardAmount,
    customer,
    discount,
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

// --- admin settings (configuración) ----------------------------------------

/** A key/value admin setting (`/settings/{key}`). `value` is always a STRING;
 *  its meaning depends on the key (boolean "true"/"false", number, free text). */
export interface AdminSetting {
  key: string;
  value: string;
  updated_at: string;
}

/** GET /api/v1/settings/{key} (Bearer). Unset key → `null` (404 mapped). */
export function getSetting(
  serverUrl: string,
  key: string,
): Promise<AdminSetting | null> {
  return invoke<AdminSetting | null>("get_setting", { serverUrl, key });
}

/** PUT /api/v1/settings/{key} (Bearer, admin+). Upserts and returns the value. */
export function setSetting(
  serverUrl: string,
  key: string,
  value: string,
): Promise<AdminSetting> {
  return invoke<AdminSetting>("set_setting", { serverUrl, key, value });
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

/** One line of a purchase order (`PurchaseOrderItemDto`). `unit_cost`/`subtotal`
 *  are STRINGS (Decimal). `qty_received` is the cumulative quantity already
 *  received against this line — `quantity - qty_received` is what's left. */
export interface PurchaseOrderItem {
  id: string;
  product: string | null;
  product_name: string;
  quantity: number;
  qty_received: number;
  unit_cost: string;
  subtotal: string;
}

/** Full purchase order WITH line items — the `GET /purchase-orders/{id}` shape.
 *  Same header as {@link PurchaseOrder} plus the populated `items`. */
export interface PurchaseOrderDetail {
  id: string;
  supplier: string;
  status: string;
  currency: string;
  total: string;
  notes: string | null;
  external_ref: string | null;
  items: PurchaseOrderItem[];
  created_at: string;
  updated_at: string;
}

/** One line for {@link createPurchaseOrder}. Field names are snake_case: serde
 *  deserializes the array elements directly (no camelCase conversion — only the
 *  top-level command args are converted). `product` omitted ⇒ free-text line;
 *  `unit_cost` is a STRING (Decimal). */
export interface NewPurchaseOrderItem {
  product?: string;
  product_name: string;
  quantity: number;
  unit_cost: string;
}

/** One line for {@link receivePurchaseOrder}. snake_case for the same reason as
 *  {@link NewPurchaseOrderItem}. `po_line_id` is the {@link PurchaseOrderItem} id. */
export interface ReceiveLine {
  po_line_id: string;
  qty_received: number;
}

/** GET /api/v1/purchase-orders/{id} (Bearer, cashier+) — full PO with items. */
export function getPurchaseOrder(serverUrl: string, id: string): Promise<PurchaseOrderDetail> {
  return invoke<PurchaseOrderDetail>("get_purchase_order", { serverUrl, id });
}

/** POST /api/v1/purchase-orders (Bearer, admin+) — create a draft PO. The server
 *  computes per-line subtotals + header total and defaults `currency` to CLP. */
export function createPurchaseOrder(
  serverUrl: string,
  supplier: string,
  items: NewPurchaseOrderItem[],
  currency?: string,
  notes?: string,
  externalRef?: string,
): Promise<PurchaseOrder> {
  return invoke<PurchaseOrder>("create_purchase_order", {
    serverUrl,
    supplier,
    items,
    currency,
    notes,
    externalRef,
  });
}

/** POST /api/v1/purchase-orders/{id}/send (Bearer, admin+) — issue a draft PO to
 *  the supplier (draft → sent). Only legal from `draft` (any other status → 409).
 *  Sending is the bridge that makes a PO receivable: a draft cannot be received
 *  directly (409), so the operator must send it first. Returns the updated PO. */
export function sendPurchaseOrder(serverUrl: string, id: string): Promise<PurchaseOrder> {
  return invoke<PurchaseOrder>("send_purchase_order", { serverUrl, id });
}

/** POST /api/v1/purchase-orders/{id}/receive (Bearer, admin+) — goods receipt.
 *  Bumps stock, recomputes weighted-average cost, advances each line's
 *  `qty_received`. Only legal from `sent`/`approved`/`partial` (a draft → 409). */
export function receivePurchaseOrder(
  serverUrl: string,
  id: string,
  lines: ReceiveLine[],
  notes?: string,
): Promise<PurchaseOrder> {
  return invoke<PurchaseOrder>("receive_purchase_order", { serverUrl, id, lines, notes });
}

/** One recorded supplier payment against a PO (`PurchasePaymentDto`). Money
 *  (`amount`) is a STRING (Decimal); dates are RFC3339. */
export interface PurchasePayment {
  id: string;
  purchase_order: string;
  amount: string;
  currency: string;
  payment_method: string; // "cash" | "bank" | "card" | "transfer"
  cash_session: string | null;
  reference: string | null;
  note: string | null;
  paid_at: string;
  created_at: string;
}

/** Accounts-payable rollup of a PO + its payments (`PurchasePaymentSummary`).
 *  `total`/`paid`/`balance` are STRING Decimals. */
export interface PurchasePaymentSummary {
  purchase_order: string;
  status: string;
  total: string;
  paid: string;
  balance: string;
  fully_paid: boolean;
  payments: PurchasePayment[];
}

/** GET /api/v1/purchase-orders/{id}/payments (Bearer, cashier+) — AP summary. */
export function getPoPayments(serverUrl: string, id: string): Promise<PurchasePaymentSummary> {
  return invoke<PurchasePaymentSummary>("get_po_payments", { serverUrl, id });
}

/** POST /api/v1/purchase-orders/{id}/payments (Bearer, admin+) — record a
 *  supplier payment. `amount` is a Decimal string; `cashSession` is required by
 *  the server when paying `cash` with an open drawer. */
export function createPoPayment(
  serverUrl: string,
  id: string,
  args: { amount: string; paymentMethod?: string; cashSession?: string; reference?: string },
): Promise<PurchasePayment> {
  return invoke<PurchasePayment>("create_po_payment", {
    serverUrl,
    id,
    amount: args.amount,
    paymentMethod: args.paymentMethod,
    cashSession: args.cashSession,
    reference: args.reference,
  });
}

/** A supplier (`domain::purchasing::model::SupplierDto`). No money fields. */
export interface Supplier {
  id: string;
  name: string;
  rut: string | null;
  contact_name: string | null;
  contact_email: string | null;
  contact_phone: string | null;
  default_invoice_format: string | null;
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** GET /api/v1/suppliers (Bearer, cashier+). `search`/`limit` optional. */
export function listSuppliers(
  serverUrl: string,
  search?: string,
  limit?: number,
): Promise<Supplier[]> {
  return invoke<Supplier[]>("list_suppliers", { serverUrl, search, limit });
}

/** POST /api/v1/suppliers (Bearer) — register a supplier. `name` required;
 *  contact fields optional (empty dropped server-side). */
export function createSupplier(
  serverUrl: string,
  name: string,
  rut?: string,
  contactName?: string,
  contactEmail?: string,
  contactPhone?: string,
): Promise<Supplier> {
  return invoke<Supplier>("create_supplier", {
    serverUrl,
    name,
    rut,
    contactName,
    contactEmail,
    contactPhone,
  });
}

// --- expenses (gastos / caja chica) ----------------------------------------

/** An expense / egreso (`ExpenseDto`). `amount` is a STRING (Decimal).
 *  `cash_session`/`supplier`/`note`/`created_by` are null when unset. */
export interface Expense {
  id: string;
  category: string;
  description: string;
  amount: string;
  payment_method: string; // "cash" | "bank" | "card" | "transfer"
  cash_session: string | null;
  supplier: string | null;
  note: string | null;
  created_by: string | null;
  incurred_at: string;
  created_at: string;
}

/** Fields the Gastos form sends to create an expense (`NewExpense`). `amount` is
 *  a STRING; `paymentMethod` defaults to `cash` server-side when omitted. */
export interface NewExpense {
  category: string;
  description: string;
  amount: string;
  paymentMethod?: string;
  note?: string;
  incurredAt?: string; // RFC3339
}

/** GET /api/v1/expenses (Bearer, cashier+). `category`/`paymentMethod`/`limit`
 *  optional filters. */
export function listExpenses(
  serverUrl: string,
  category?: string,
  paymentMethod?: string,
  limit?: number,
): Promise<Expense[]> {
  return invoke<Expense[]>("list_expenses", { serverUrl, category, paymentMethod, limit });
}

/** POST /api/v1/expenses (Bearer, cashier+) — record an expense. `amount` STRING. */
export function createExpense(serverUrl: string, e: NewExpense): Promise<Expense> {
  return invoke<Expense>("create_expense", {
    serverUrl,
    category: e.category,
    description: e.description,
    amount: e.amount,
    paymentMethod: e.paymentMethod,
    note: e.note,
    incurredAt: e.incurredAt,
  });
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
// --- inventario writes + lotes / vencimientos ------------------------------

/** Full product detail (`ProductDto`). Money (`price`/`cost_price`) are STRINGS. */
export interface ProductDetail {
  id: string;
  name: string;
  slug: string;
  description: string | null;
  price: string;
  cost_price: string | null;
  stock: number;
  category: string | null;
  active: boolean;
  laboratory: string | null;
  therapeutic_action: string | null;
  active_ingredient: string | null;
  prescription_type: string;
  presentation: string | null;
  discount_percent: number | null;
}

/** One product batch / lote (`BatchDto`). `expiry_date` RFC3339; `cost` STRING|null. */
export interface Batch {
  id: string;
  product: string;
  batch_code: string;
  expiry_date: string;
  stock: number;
  cost: string | null;
  notes: string | null;
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** A soon-to-expire / expired batch row (`NearExpiryRow`).
 *  `days_to_expiry` < 0 ⇒ already expired (also `expired === true`). */
export interface NearExpiryRow {
  product_id: string;
  product_name: string;
  batch_id: string;
  batch_code: string;
  expiry_date: string;
  stock: number;
  days_to_expiry: number;
  expired: boolean;
}

/** Fields the "Nuevo producto" form collects. `price`/`costPrice` are STRINGS. */
export interface NewProductInput {
  name: string;
  price: string;
  costPrice?: string;
  stock?: number;
  laboratory?: string;
  activeIngredient?: string;
  presentation?: string;
}

/** POST /api/v1/products (Bearer, admin+). Rejects with a Spanish string
 *  ("Permiso denegado…" on a non-admin 403). */
export function createProduct(
  serverUrl: string,
  input: NewProductInput,
): Promise<ProductDetail> {
  return invoke<ProductDetail>("create_product", {
    serverUrl,
    name: input.name,
    price: input.price,
    costPrice: input.costPrice,
    stock: input.stock,
    laboratory: input.laboratory,
    activeIngredient: input.activeIngredient,
    presentation: input.presentation,
  });
}

/** One rejected row from a bulk CSV import (1-based line + reason). */
export interface ImportRowError {
  line: number;
  message: string;
}

/** Outcome of `POST /products/import` — row counts + per-row errors. */
export interface ImportSummary {
  created: number;
  updated: number;
  failed: number;
  errors: ImportRowError[];
}

/** POST /api/v1/products/import (Bearer, admin+). `csv` = raw file text; the
 *  server upserts by `external_id` (idempotent). Returns the import summary. */
export function importProducts(
  serverUrl: string,
  csv: string,
): Promise<ImportSummary> {
  return invoke<ImportSummary>("import_products", { serverUrl, csv });
}

/** POST /api/v1/products/import?dry_run=true (Bearer, admin+). Validates + counts
 *  the CSV WITHOUT writing anything — powers the preview shown before the operator
 *  confirms a catalog migration. Same {@link ImportSummary} shape as a real run. */
export function importProductsPreview(
  serverUrl: string,
  csv: string,
): Promise<ImportSummary> {
  return invoke<ImportSummary>("import_products_preview", { serverUrl, csv });
}

/** GET /api/v1/products/export (Bearer) — full catalog as CSV text. The webview
 *  wraps it in a Blob for download. Columns round-trip with {@link importProducts}. */
export function exportProducts(serverUrl: string): Promise<string> {
  return invoke<string>("export_products", { serverUrl });
}

/** GET /api/v1/products/{id} (Bearer) — full detail for the drawer. */
export function productDetail(
  serverUrl: string,
  id: string,
): Promise<ProductDetail> {
  return invoke<ProductDetail>("product_detail", { serverUrl, id });
}

/** POST /api/v1/products/{id}/stock (Bearer, admin+). Pass `set` (absolute) or
 *  `delta` (signed) + optional `reason`. Returns the updated product. */
export function adjustProductStock(
  serverUrl: string,
  id: string,
  opts: { set?: number; delta?: number; reason?: string },
): Promise<ProductDetail> {
  return invoke<ProductDetail>("adjust_product_stock", {
    serverUrl,
    id,
    set: opts.set,
    delta: opts.delta,
    reason: opts.reason,
  });
}

/** GET /api/v1/batches (Bearer). Optional `product` + `expiringWithinDays`
 *  + `onlyAvailable` filters. */
export function listBatches(
  serverUrl: string,
  product?: string,
  expiringWithinDays?: number,
  onlyAvailable?: boolean,
  limit?: number,
): Promise<Batch[]> {
  return invoke<Batch[]>("list_batches", {
    serverUrl,
    product,
    expiringWithinDays,
    onlyAvailable,
    limit,
  });
}

/** POST /api/v1/batches (Bearer, admin+). `expiryDate` must be RFC3339
 *  (`YYYY-MM-DDT00:00:00Z`). `cost` is a STRING when present. */
export function createBatch(
  serverUrl: string,
  product: string,
  batchCode: string,
  expiryDate: string,
  opts: { stock?: number; cost?: string; notes?: string } = {},
): Promise<Batch> {
  return invoke<Batch>("create_batch", {
    serverUrl,
    product,
    batchCode,
    expiryDate,
    stock: opts.stock,
    cost: opts.cost,
    notes: opts.notes,
  });
}

/** GET /api/v1/reports/near-expiry?days=N (Bearer). Core/free, not gated. */
export function nearExpiry(
  serverUrl: string,
  days?: number,
): Promise<NearExpiryRow[]> {
  return invoke<NearExpiryRow[]>("near_expiry", { serverUrl, days });
}

/** Fields for creating / editing a customer. `name` is required on create; on
 *  edit every field is optional (only the ones set are sent). */
export interface CustomerInput {
  name?: string;
  rut?: string;
  phone?: string;
  email?: string;
  active?: boolean;
}

/** POST /api/v1/clientes (Bearer, cashier+) — register a new customer. Empty
 *  optional fields are dropped server-side (stored null). Rejects with
 *  {@link CUSTOMERS_MODULE_MISSING} when the module is not deployed (404). */
export function createCustomer(
  serverUrl: string,
  name: string,
  rut?: string,
  phone?: string,
  email?: string,
): Promise<Customer> {
  return invoke<Customer>("create_customer", {
    serverUrl,
    name,
    rut,
    phone,
    email,
  });
}

/** PATCH /api/v1/clientes/{id} (Bearer, cashier+) — edit a customer. Only the
 *  provided fields are forwarded; `active` toggles activar/desactivar. Rejects
 *  with {@link CUSTOMERS_MODULE_MISSING} when not deployed (404). */
export function updateCustomer(
  serverUrl: string,
  id: string,
  patch: CustomerInput,
): Promise<Customer> {
  return invoke<Customer>("update_customer", {
    serverUrl,
    id,
    name: patch.name,
    rut: patch.rut,
    phone: patch.phone,
    email: patch.email,
    active: patch.active,
  });
}

// --- recetas / prescriptions (Ley 20.000) ----------------------------------

/** A prescription row (`PrescriptionDto`). Immutable per Ley 20.000 — the server
 *  exposes only create/get/list (never update/delete). `product`/`customer` are
 *  optional record ids; `controlled = true` marks a controlled-drug entry, which
 *  the server requires `doctor_name` + `doctor_rut` for. Dates are RFC3339. */
export interface Prescription {
  id: string;
  product: string | null;
  customer: string | null;
  patient_name: string;
  patient_rut: string;
  doctor_name: string | null;
  doctor_rut: string | null;
  controlled: boolean;
  folio: string | null;
  dispensed_at: string;
  created_at: string;
}

/** Fields the "Nueva receta" form collects. `patientName`/`patientRut` are
 *  required; `doctorName`/`doctorRut` are required by the server when
 *  `controlled` is true. Empty optionals are dropped server-side. */
export interface NewPrescriptionInput {
  patientName: string;
  patientRut: string;
  controlled: boolean;
  doctorName?: string;
  doctorRut?: string;
  product?: string;
  customer?: string;
  folio?: string;
}

/** GET /api/v1/prescriptions (Bearer). Optional `patientRut` / `controlled`
 *  (true → only controlled) / `limit` filters. Newest first server-side. */
export function listPrescriptions(
  serverUrl: string,
  patientRut?: string,
  controlled?: boolean,
  limit?: number,
): Promise<Prescription[]> {
  return invoke<Prescription[]>("list_prescriptions", {
    serverUrl,
    patientRut,
    controlled,
    limit,
  });
}

/** GET /api/v1/prescriptions/{id} (Bearer) — single prescription detail. */
export function getPrescription(serverUrl: string, id: string): Promise<Prescription> {
  return invoke<Prescription>("get_prescription", { serverUrl, id });
}

/** POST /api/v1/prescriptions (Bearer, pharmacist+) — register a prescription.
 *  Rejects with a Spanish string ("Permiso denegado…" on a non-pharmacist 403). */
export function createPrescription(
  serverUrl: string,
  input: NewPrescriptionInput,
): Promise<Prescription> {
  return invoke<Prescription>("create_prescription", {
    serverUrl,
    patientName: input.patientName,
    patientRut: input.patientRut,
    controlled: input.controlled,
    doctorName: input.doctorName,
    doctorRut: input.doctorRut,
    product: input.product,
    customer: input.customer,
    folio: input.folio,
  });
}

/** GET /api/v1/libro-recetas (Bearer) — controlled-only ledger (Ley 20.000).
 *  Same shape as {@link listPrescriptions} but `controlled = true` is enforced
 *  server-side. Optional `patientRut` / `limit` filters. */
export function libroRecetas(
  serverUrl: string,
  patientRut?: string,
  limit?: number,
): Promise<Prescription[]> {
  return invoke<Prescription[]>("libro_recetas", { serverUrl, patientRut, limit });
}

/** GET /api/v1/libro-recetas/export (Bearer) — raw CSV text of the controlled
 *  ledger (ISP/DEIS). The webview wraps it in a Blob for download. */
export function exportLibroRecetas(serverUrl: string, patientRut?: string): Promise<string> {
  return invoke<string>("export_libro_recetas", { serverUrl, patientRut });
}

// --- devoluciones / returns ------------------------------------------------

/** A refund/devolución header (`DevolucionDto`). `total_devuelto` is a STRING
 *  (Decimal). `order` is the linked sale (null for a standalone refund). */
export interface Devolucion {
  id: string;
  order: string | null;
  tipo: string;
  motivo: string;
  notas: string | null;
  total_devuelto: string;
  metodo_reembolso: string | null;
  procesado_por: string | null;
  created_at: string;
}

/** One refund line for {@link createRefund}. snake_case (serde deserializes the
 *  array elements directly — no camelCase rename, same as `PosItem`).
 *  `unit_price` STRING; `restock` returns the unit to stock — the server REQUIRES
 *  `product` to be set when `restock` is true, so receipt-driven lines (which
 *  carry no product id) must send `restock: false`. */
export interface RefundItem {
  product?: string;
  product_name: string;
  quantity: number;
  unit_price: string;
  restock: boolean;
}

/** Narrowed result of a successful refund: the devolución header + whether the
 *  linked order was flipped to `refunded`. */
export interface RefundResult {
  devolucion: Devolucion;
  orderMarkedRefunded: boolean;
}

/** POST /api/v1/pos/returns (Bearer, cashier+) — create a refund. `tipo` is the
 *  return MOTIVO and must be a value the `devolucion.tipo` schema accepts
 *  (`venta` | `cancelacion` | `garantia` | `error`); defaults to `venta` (a
 *  normal return against a sale). It is NOT the total/parcial scope — that is a
 *  presentational distinction, never persisted on this field. Sending an invalid
 *  tipo (e.g. `total`/`parcial`) made the server reject every return with a 500
 *  (BUG-paul-001). Rejects with a Spanish string (e.g. over-refund / 403). */
export async function createRefund(
  serverUrl: string,
  motivo: string,
  items: RefundItem[],
  opts: { order?: string; tipo?: string; notas?: string; metodoReembolso?: string } = {},
): Promise<RefundResult> {
  const res = await invoke<{ devolucion: Devolucion; order_marked_refunded?: boolean }>(
    "create_refund",
    {
      serverUrl,
      order: opts.order,
      tipo: opts.tipo ?? "venta",
      motivo,
      notas: opts.notas,
      items,
      metodoReembolso: opts.metodoReembolso,
    },
  );
  return {
    devolucion: res.devolucion,
    orderMarkedRefunded: res.order_marked_refunded ?? false,
  };
}

/** GET /api/v1/returns (Bearer). Optional `order` / `tipo` / `limit` filters. */
export function listRefunds(
  serverUrl: string,
  order?: string,
  tipo?: string,
  limit?: number,
): Promise<Devolucion[]> {
  return invoke<Devolucion[]>("list_refunds", { serverUrl, order, tipo, limit });
}

// --- DTE / boleta electrónica SII -------------------------------------------

/** A DTE row (`crates/api/src/v1/dte.rs::DteDto`). `monto_total` is a STRING
 *  (Decimal); `tipo` is the SII code (39 = boleta). `has_xml` flags whether the
 *  signed XML can be downloaded via {@link dteXml}. */
export interface Dte {
  id: string;
  tipo: number;
  folio: number;
  fecha_emision: string;
  rut_emisor: string;
  rut_receptor: string;
  razon_social_receptor: string;
  monto_total: string;
  estado: string; // "draft" | "signed" | "sent" | "accepted" | "rejected" | "cancelled"
  track_id: number | null;
  sii_glosa: string | null;
  order_id: string | null;
  has_xml: boolean;
}

/** One active CAF + remaining folios (`GET /dte/caf-status` row). */
export interface CafInfo {
  id: string;
  folio_desde: number;
  folio_hasta: number;
  next_folio: number;
  restantes: number;
}

/** `GET /dte/caf-status` payload — total folios left for the DTE type. */
export interface CafStatus {
  tipo: number;
  folios_restantes: number;
  cafs: CafInfo[];
}

/** Filters for {@link listDtes}. Dates are `YYYY-MM-DD`. */
export interface DteFilters {
  estado?: string;
  tipo?: number;
  from?: string;
  to?: string;
  limit?: number;
}

/** GET /api/v1/dte (Bearer) — boletas/DTEs del tenant, newest first. */
export function listDtes(serverUrl: string, filters: DteFilters = {}): Promise<Dte[]> {
  return invoke<Dte[]>("list_dtes", {
    serverUrl,
    estado: filters.estado,
    tipo: filters.tipo,
    from: filters.from,
    to: filters.to,
    limit: filters.limit,
  });
}

/** GET /api/v1/dte/caf-status?tipo=N (Bearer) — folios restantes (default 39). */
export function dteCafStatus(serverUrl: string, tipo?: number): Promise<CafStatus> {
  return invoke<CafStatus>("dte_caf_status", { serverUrl, tipo });
}

/** GET /api/v1/dte/{id}/xml (Bearer) — signed XML text for a Blob download. */
export function dteXml(serverUrl: string, id: string): Promise<string> {
  return invoke<string>("dte_xml", { serverUrl, id });
}

/** Receptor completo — obligatorio en factura/notas/guía (33/56/61/52). */
export interface DocReceptor {
  rut: string;
  razon_social: string;
  giro: string;
  direccion: string;
  comuna: string;
}

/** Línea de documento. Precios IVA-incluido; el server calcula neto/IVA. */
export interface DocItem {
  nombre: string;
  cantidad: string;
  precio_unitario: string;
  exento?: boolean;
}

/** Referencia al documento original (obligatoria en notas 56/61). */
export interface DocReferencia {
  tipo_doc_ref: string;
  folio_ref: string;
  fecha_ref: string; // YYYY-MM-DD
  cod_ref?: number; // 1 anula, 2 corrige texto, 3 corrige montos
  razon_ref?: string;
}

/** POST /api/v1/dte/documentos (Bearer, admin+) — emit + sign factura (33),
 *  nota débito (56), nota crédito (61) o guía (52). Rejects `"CODE|message"`
 *  — use {@link parseSaleError}. */
export function emitDocumento(
  serverUrl: string,
  args: {
    tipo: number;
    certPassphrase: string;
    receptor: DocReceptor;
    items: DocItem[];
    referencias?: DocReferencia[];
    indTraslado?: number;
    orderId?: string;
  },
): Promise<Dte> {
  return invoke<Dte>("emit_documento", {
    serverUrl,
    tipo: args.tipo,
    certPassphrase: args.certPassphrase,
    receptor: args.receptor,
    items: args.items,
    referencias: args.referencias,
    indTraslado: args.indTraslado,
    orderId: args.orderId,
  });
}

/** GET /api/v1/dte/libro-ventas?period=YYYY-MM (Bearer, admin+) — monthly
 *  sales book XML (unsigned, accountant review / manual upload). */
export function dteLibroVentas(serverUrl: string, period: string): Promise<string> {
  return invoke<string>("dte_libro_ventas", { serverUrl, period });
}

/** POST /api/v1/dte/libro-ventas/signed (Bearer, admin+) — monthly sales book
 *  signed with the company cert (EnvioLibro), ready for the SII portal. */
export function dteLibroVentasSigned(
  serverUrl: string,
  period: string,
  certPassphrase: string,
): Promise<string> {
  return invoke<string>("dte_libro_ventas_signed", { serverUrl, period, certPassphrase });
}

/** POST /api/v1/dte/boletas (Bearer, cashier+) — emit + sign the boleta of a
 *  paid POS order. Rejects with `"CODE|message"` — use {@link parseSaleError}. */
export function emitBoleta(
  serverUrl: string,
  orderId: string,
  certPassphrase: string,
  receptorRut?: string,
  razonSocialReceptor?: string,
): Promise<Dte> {
  return invoke<Dte>("emit_boleta", {
    serverUrl,
    orderId,
    certPassphrase,
    receptorRut,
    razonSocialReceptor,
  });
}

/** POST /api/v1/dte/{id}/send (Bearer, admin+) — upload to SII. Tier-gated:
 *  rejects `"FEATURE_REQUIRES_UPGRADE|msg"` on Free — use {@link parseSaleError}. */
export function sendDte(serverUrl: string, id: string): Promise<Dte> {
  return invoke<Dte>("send_dte", { serverUrl, id });
}

/** Poll result: the refreshed DTE + the normalized SII verdict of this query. */
export interface DtePollResult extends Dte {
  sii_estado: string; // aceptado|aceptado_con_reparos|rechazado|recibido|en_proceso|error
}

/** POST /api/v1/dte/{id}/poll (Bearer, admin+) — refresh the SII verdict. */
export function pollDte(serverUrl: string, id: string): Promise<DtePollResult> {
  return invoke<DtePollResult>("poll_dte", { serverUrl, id });
}

/** POST /api/v1/dte/{id}/cancel (Bearer, admin+) — anular pre-envío. */
export function cancelDte(serverUrl: string, id: string, reason: string): Promise<Dte> {
  return invoke<Dte>("cancel_dte", { serverUrl, id, reason });
}

// --- auditoría / audit-log -------------------------------------------------

/** One row of the immutable audit trail (`AuditItem`). `before`/`after`/
 *  `metadata`/`record_id`/`table` may be null (server schema gap — today the row
 *  records method/path/status/ip). `status` is the HTTP status of the request. */
export interface AuditEntry {
  id: string;
  created_at: string;
  user: string | null;
  user_email: string | null;
  table: string | null;
  record_id: string | null;
  action: string; // "create" | "update" | "delete" | "other"
  method: string;
  path: string;
  status: number | null;
  ip: string | null;
  user_agent: string | null;
  payload_hash: string | null;
  before: unknown;
  after: unknown;
  metadata: unknown;
}

/** Paginated audit-log response (`AuditResponse`). */
export interface AuditPage {
  total: number;
  items: AuditEntry[];
  limit: number;
  offset: number;
}

/** Filters for {@link queryAuditLog}. Dates are `YYYY-MM-DD`; `action` is
 *  `create|update|delete`; `user` is a record id. */
export interface AuditFilters {
  from?: string;
  to?: string;
  user?: string;
  table?: string;
  action?: string;
  limit?: number;
  offset?: number;
}

/** GET /api/v1/admin/audit-log (Bearer, admin/owner) — immutable audit trail.
 *  Rejects with a Spanish string (e.g. "Permiso denegado…" on a non-admin 403). */
export function queryAuditLog(serverUrl: string, filters: AuditFilters = {}): Promise<AuditPage> {
  return invoke<AuditPage>("query_audit_log", {
    serverUrl,
    from: filters.from,
    to: filters.to,
    user: filters.user,
    table: filters.table,
    action: filters.action,
    limit: filters.limit,
    offset: filters.offset,
  });
}

// --- server base URL (ADR-0015 P0 — configurable server target) -------------
//
// The client is API-first (ADR-0015): every command takes an explicit
// `serverUrl`, which flows from login. These helpers persist/recall the target
// so a LAN terminal (tablet caja) or a relocated desktop can point at the
// on-prem server's IP without rebuilding. localhost stays the default. Shared
// with login.ts via the same localStorage key.

/** localStorage key holding the last/working server URL (shared with login). */
export const SERVER_STORE_KEY = "pharma:last-server";
/** Loopback default — desktop co-installed with the server. */
export const FALLBACK_SERVER_URL = "http://127.0.0.1:8080";

/** Resolve the configured server base URL: persisted value, else loopback.
 *  Never throws — onboarding must not dead-end on a storage hiccup. */
export function storedServerUrl(): string {
  try {
    return localStorage.getItem(SERVER_STORE_KEY)?.trim() || FALLBACK_SERVER_URL;
  } catch {
    return FALLBACK_SERVER_URL;
  }
}

/** Persist the server base URL so the next launch/login pre-fills it. */
export function rememberServerUrl(url: string): void {
  try {
    localStorage.setItem(SERVER_STORE_KEY, url.trim());
  } catch {
    /* noop — running without persistence is still usable this session */
  }
}

// --- demo seeding (multi-rubro onboarding) ----------------------------------

/** Outcome of `POST /api/v1/admin/seed-demo` (`domain::seed::SeedSummary`). */
export interface SeedSummary {
  vertical: string;
  products_created: number;
  batches_created: number;
  movements_emitted: number;
  wiped: number;
}

/** Sentinel the `seed_demo` command rejects with when demo data already exists
 *  and `force` was false (server 409) — the view offers a "regenerar" confirm. */
export const SEED_ALREADY_EXISTS = "SEED_ALREADY_EXISTS";

/** POST /api/v1/admin/seed-demo (Bearer, admin/owner) — fill the tenant with a
 *  believable DEMO catalog for `vertical` (`pharmacy` | `minimarket`). `force`
 *  wipes the prior demo pack before re-seeding. Rejects with
 *  {@link SEED_ALREADY_EXISTS} on a 409, or a Spanish string otherwise. */
export function seedDemo(
  serverUrl: string,
  vertical: string,
  force: boolean,
): Promise<SeedSummary> {
  return invoke<SeedSummary>("seed_demo", { serverUrl, vertical, force });
}

// --- agent assist ("Pregúntale a tu negocio", ADR-0016) ---------------------

/** A WRITE the agent proposes but will NOT run until the owner confirms
 *  (`crates/api/src/v1/assist.rs` propose→confirm contract). `action` is the
 *  machine label (`gasto.crear`, `compra.crear` …); `resumen` is the Spanish
 *  one-liner of exactly what will happen ("Vas a registrar un gasto de $5.000
 *  (nafta)."); `confirm_token` is the opaque, single-use ticket the client hands
 *  back to `/assist/act` to execute. Nothing mutates server-side on `ask` — the
 *  token is the ONLY way to commit. */
export interface AgentProposal {
  action: string;
  resumen: string;
  confirm_token: string;
}

/** The agent's reply (`crates/assist/src/provider.rs::Answer`). `intent` is the
 *  stable machine label (`ventas_hoy` … `desconocido`); `text` the Spanish
 *  answer grounded in the tenant's own data; `data` an optional structured
 *  payload backing the prose (absent when the intent carries no figures).
 *  `proposal` is present ONLY when the question asked for a WRITE — then the UI
 *  must show a confirmation card and run nothing until the owner says yes. */
export interface AssistAnswer {
  intent: string;
  text: string;
  data?: unknown;
  proposal?: AgentProposal;
}

/** Outcome of a confirmed action (`POST /api/v1/assist/act`). `text` is the
 *  Spanish receipt of what just happened ("Gasto de $5.000 registrado."); the
 *  rest of the server payload rides along untyped in `data` (kept loose so the
 *  exact action result shape can grow without breaking the client). */
export interface AssistActResult {
  text?: string;
  data?: unknown;
}

/** POST /api/v1/assist/ask (Bearer, cashier+) — the read-only, offline-first
 *  business agent. Resolves with the answer even when the agent didn't
 *  understand (`intent = "desconocido"` is a normal 200, not an error); a server
 *  failure rejects with a Spanish string. When the owner asks for a write the
 *  answer carries a {@link AgentProposal} — still no mutation until `assistAct`. */
export function assistAsk(serverUrl: string, question: string): Promise<AssistAnswer> {
  return invoke<AssistAnswer>("assist_ask", { serverUrl, question });
}

/** POST /api/v1/assist/act (Bearer) — EXECUTE the write the agent proposed,
 *  authorised by the single-use `confirmToken` from a prior {@link AgentProposal}.
 *  This is the only call that mutates: it must be reachable ONLY from the
 *  owner's explicit "Confirmar" click. A role-denied write (`403`) or an expired
 *  token rejects with the server's Spanish message. */
export function assistAct(
  serverUrl: string,
  confirmToken: string,
): Promise<AssistActResult> {
  return invoke<AssistActResult>("assist_act", { serverUrl, confirmToken });
}

// --- config center: usuarios y roles ----------------------------------------
//
// Maps the `crates/api/src/v1/config.rs` user-management surface. All endpoints
// are admin/owner gated server-side; a 403 surfaces as the Spanish role error.
// `roles` are the canonical English keys (`cashier`/`pharmacist`/`admin`/
// `owner`) — the UI renders Spanish labels via config-center `roleLabel`.

/** A back-office user (`config.rs::UserDto`). `password`/`tenant` never cross
 *  the wire. `roles` is the canonical English key set. */
export interface ConfigUser {
  id: string;
  email: string;
  roles: string[];
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** GET /api/v1/config/users (Bearer, admin+) — every user of the tenant. */
export function listConfigUsers(serverUrl: string): Promise<ConfigUser[]> {
  return invoke<ConfigUser[]>("config_list_users", { serverUrl });
}

/** POST /api/v1/config/users (Bearer, admin+) — create a user. The server
 *  rejects a duplicate email (409) and an `owner` role assigned by a non-owner
 *  (403); both surface as Spanish strings. */
export function createConfigUser(
  serverUrl: string,
  email: string,
  password: string,
  roles: string[],
): Promise<ConfigUser> {
  return invoke<ConfigUser>("config_create_user", { serverUrl, email, password, roles });
}

/** PATCH /api/v1/config/users/{id} (Bearer, admin+) — replace a user's roles.
 *  The server guards the last active owner (409) and owner-grant (403). */
export function updateUserRoles(
  serverUrl: string,
  id: string,
  roles: string[],
): Promise<ConfigUser> {
  return invoke<ConfigUser>("config_update_user_roles", { serverUrl, id, roles });
}

/** POST /api/v1/config/users/{id}/{disable|enable} (Bearer, admin+) — toggle a
 *  user's `active` flag. The server refuses to disable the sole active owner. */
export function setUserActive(
  serverUrl: string,
  id: string,
  active: boolean,
): Promise<ConfigUser> {
  return invoke<ConfigUser>("config_set_user_active", { serverUrl, id, active });
}

// --- config center: sucursales y cajas --------------------------------------
//
// `crates/api/src/v1/branches.rs` — tenant-scoped branch (sucursal) + register
// (caja) CRUD. Reads need any bearer token; writes are admin/owner.

/** A branch / sucursal (`branches::model::BranchDto`). */
export interface Branch {
  id: string;
  name: string;
  code: string | null;
  address: string | null;
  comuna: string | null;
  phone: string | null;
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** A register / caja (`branches::model::RegisterDto`). `branch` is the owning
 *  sucursal record id (`branch:<key>`) or null when unassigned. */
export interface Register {
  id: string;
  branch: string | null;
  name: string;
  code: string | null;
  active: boolean;
  created_at: string;
  updated_at: string;
}

/** Fields the sucursal form sends. `name` required; the rest optional. */
export interface BranchInput {
  name?: string;
  code?: string;
  address?: string;
  comuna?: string;
  phone?: string;
  active?: boolean;
}

/** Fields the caja form sends. `name` required; `branch` is an optional sucursal
 *  record id; `active` only meaningful on update. */
export interface RegisterInput {
  name?: string;
  branch?: string;
  code?: string;
  active?: boolean;
}

/** GET /api/v1/sucursales (Bearer). `active=false` includes inactive ones. */
export function listBranches(serverUrl: string, active?: boolean): Promise<Branch[]> {
  return invoke<Branch[]>("list_branches", { serverUrl, active });
}

/** POST /api/v1/sucursales (Bearer, admin+) — create a branch. */
export function createBranch(serverUrl: string, input: BranchInput): Promise<Branch> {
  return invoke<Branch>("create_branch", {
    serverUrl,
    name: input.name,
    code: input.code,
    address: input.address,
    comuna: input.comuna,
    phone: input.phone,
  });
}

/** PATCH /api/v1/sucursales/{id} (Bearer, admin+) — edit a branch. Only the
 *  provided fields are forwarded; `active` toggles activar/desactivar. */
export function updateBranch(
  serverUrl: string,
  id: string,
  patch: BranchInput,
): Promise<Branch> {
  return invoke<Branch>("update_branch", {
    serverUrl,
    id,
    name: patch.name,
    code: patch.code,
    address: patch.address,
    comuna: patch.comuna,
    phone: patch.phone,
    active: patch.active,
  });
}

/** DELETE /api/v1/sucursales/{id} (Bearer, admin+) — soft-delete (deactivate). */
export function deleteBranch(serverUrl: string, id: string): Promise<void> {
  return invoke<void>("delete_branch", { serverUrl, id });
}

/** GET /api/v1/cajas (Bearer). `active` / `branch` (sucursal id) optional. */
export function listRegisters(
  serverUrl: string,
  opts: { active?: boolean; branch?: string } = {},
): Promise<Register[]> {
  return invoke<Register[]>("list_registers", {
    serverUrl,
    active: opts.active,
    branch: opts.branch,
  });
}

/** POST /api/v1/cajas (Bearer, admin+) — create a register. `branch` (if set)
 *  must belong to the tenant (404 otherwise). */
export function createRegister(serverUrl: string, input: RegisterInput): Promise<Register> {
  return invoke<Register>("create_register", {
    serverUrl,
    name: input.name,
    branch: input.branch,
    code: input.code,
  });
}

/** PATCH /api/v1/cajas/{id} (Bearer, admin+) — edit a register. */
export function updateRegister(
  serverUrl: string,
  id: string,
  patch: RegisterInput,
): Promise<Register> {
  return invoke<Register>("update_register", {
    serverUrl,
    id,
    name: patch.name,
    branch: patch.branch,
    code: patch.code,
    active: patch.active,
  });
}

/** DELETE /api/v1/cajas/{id} (Bearer, admin+) — soft-delete (deactivate). */
export function deleteRegister(serverUrl: string, id: string): Promise<void> {
  return invoke<void>("delete_register", { serverUrl, id });
}

// --- config center: respaldo (backup) ---------------------------------------
//
// `crates/api/src/v1/backup.rs`. NOT tenant-scoped — a snapshot covers the whole
// install. Admin/owner only.

/** Result of an on-demand backup (`backup.rs::BackupReport`). `bytes` is the
 *  artifact size; `path` is the absolute `.tar.gz` location on the server box. */
export interface BackupReport {
  path: string;
  bytes: number;
  sha256: string;
  started_at: string;
  duration_ms: number;
}

/** One audited backup attempt (`db::backup_log::BackupLogEntry`). Artifact
 *  fields are null on a failed attempt, where `error` carries the reason. A
 *  listed `path` may no longer exist on disk (artifacts are pruned separately). */
export interface BackupLogEntry {
  status: string; // "ok" | "failed"
  source: string; // "scheduled" | "manual" | "cli"
  path: string | null;
  bytes: number | null;
  sha256: string | null;
  duration_ms: number | null;
  error: string | null;
  started_at: string;
}

/** POST /api/v1/admin/backup (Bearer, admin+) — tar+gzip the data dir now. */
export function runBackup(serverUrl: string): Promise<BackupReport> {
  return invoke<BackupReport>("run_backup", { serverUrl });
}

/** GET /api/v1/admin/backup?limit=N (Bearer, admin+) — the snapshot audit log,
 *  newest first (default 50, max 500). */
export function listBackups(serverUrl: string, limit?: number): Promise<BackupLogEntry[]> {
  return invoke<BackupLogEntry[]>("list_backups", { serverUrl, limit });
}

// --- config center: medios de pago + ficha de negocio -----------------------
//
// `crates/api/src/v1/config.rs`. Reads are any-auth (the POS needs accepted
// tender + the receipt header); writes are admin/owner. The business profile is
// the general UI/receipt card — SII emisor identity (RUT/comuna for DTE) lives
// separately under the `dte.emisor` setting (Negocio › Datos tributarios).

/** Accepted-tender configuration (`config.rs::PaymentMethodsResponse`).
 *  `methods` is the active selection; `available` the full catalog to offer. */
export interface PaymentMethods {
  methods: string[];
  available: string[];
}

/** GET /api/v1/config/payment-methods (Bearer) — current + available tender. */
export function getPaymentMethods(serverUrl: string): Promise<PaymentMethods> {
  return invoke<PaymentMethods>("config_get_payment_methods", { serverUrl });
}

/** PUT /api/v1/config/payment-methods (Bearer, admin+) — set accepted tender. */
export function setPaymentMethods(serverUrl: string, methods: string[]): Promise<PaymentMethods> {
  return invoke<PaymentMethods>("config_set_payment_methods", { serverUrl, methods });
}

/** General business profile for the UI/receipts (`config.rs::BusinessProfile`).
 *  `razon_social` is required on save; the rest are optional. */
export interface BusinessProfile {
  razon_social: string | null;
  giro: string | null;
  direccion: string | null;
  telefono: string | null;
  email: string | null;
}

/** GET /api/v1/config/business (Bearer) — the business card. */
export function getBusinessProfile(serverUrl: string): Promise<BusinessProfile> {
  return invoke<BusinessProfile>("config_get_business", { serverUrl });
}

/** PUT /api/v1/config/business (Bearer, admin+) — save the business card.
 *  `razonSocial` required; empty optionals are stored null. */
export function setBusinessProfile(
  serverUrl: string,
  profile: {
    razonSocial: string;
    giro?: string;
    direccion?: string;
    telefono?: string;
    email?: string;
  },
): Promise<BusinessProfile> {
  return invoke<BusinessProfile>("config_set_business", {
    serverUrl,
    razonSocial: profile.razonSocial,
    giro: profile.giro,
    direccion: profile.direccion,
    telefono: profile.telefono,
    email: profile.email,
  });
}

// --- config center: SII certificado + CAF (carga desde la UI, #268) ---------
//
// `crates/api/src/v1/dte.rs`. The .pfx travels base64-encoded; the passphrase is
// validated against the key material then used to encrypt-at-rest and is NEVER
// persisted (ADR-0011 §cert). Admin/owner only.

/** Result of a cert (.pfx) upload (`dte.rs::upload_cert` 201 body). */
export interface CertUploadResult {
  id: string;
  rut: string;
  vigencia_desde: string;
  vigencia_hasta: string;
  blob_bytes: number;
}

/** POST /api/v1/dte/cert (Bearer, admin+) — import the digital certificate. The
 *  PFX bytes are base64; the passphrase is verified, used to encrypt, discarded.
 *  Rejects with the server's Spanish message (bad base64/PFX/passphrase, 403…). */
export function uploadCert(
  serverUrl: string,
  args: {
    pfxBase64: string;
    certPassphrase: string;
    rut: string;
    vigenciaDesde: string;
    vigenciaHasta: string;
  },
): Promise<CertUploadResult> {
  return invoke<CertUploadResult>("dte_upload_cert", {
    serverUrl,
    pfxBase64: args.pfxBase64,
    certPassphrase: args.certPassphrase,
    rut: args.rut,
    vigenciaDesde: args.vigenciaDesde,
    vigenciaHasta: args.vigenciaHasta,
  });
}

/** Result of a CAF (folios) upload (`dte.rs::upload_caf` 201 body). */
export interface CafUploadResult {
  id: string;
  tipo: number;
  folio_desde: number;
  folio_hasta: number;
  next_folio: number;
  rut_emisor: string;
}

/** POST /api/v1/dte/caf (Bearer, admin+) — import a CAF (folios) XML from the
 *  SII. The whole XML is kept (the TED embeds it). Rejects with a Spanish
 *  message on an invalid CAF XML / 403. */
export function uploadCaf(serverUrl: string, xml: string): Promise<CafUploadResult> {
  return invoke<CafUploadResult>("dte_upload_caf", { serverUrl, xml });
}

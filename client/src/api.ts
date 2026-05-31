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

/** POST /api/v1/pos/returns (Bearer, cashier+) — create a refund. `tipo` defaults
 *  to `parcial`. Rejects with a Spanish string (e.g. over-refund / 403). */
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
      tipo: opts.tipo ?? "parcial",
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

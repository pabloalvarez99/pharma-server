//! Wire types — shaped to the REAL server contract (read from `crates/api`).
//!
//! Money ALWAYS crosses the wire as a STRING (`rust_decimal::serde::str`) and
//! stays a `String` here — never `f64`. All user-facing error strings are in
//! Spanish (project rule); identifiers and `code` values stay English.

use serde::{Deserialize, Serialize};

/// What the frontend receives after a successful login. We deliberately do NOT
/// return the token to the webview — it stays in `SessionState`.
#[derive(Serialize)]
pub struct SessionInfo {
    /// User record id (server `sub`, e.g. `user:abc`).
    pub user_id: String,
    /// Tenant record id (server `tenant_id`, e.g. `tenant:xyz`).
    pub tenant_id: String,
    pub roles: Vec<String>,
    pub expires_in: u64,
}

/// Mirrors `crates/api/src/v1/license.rs::LicenseSummary` 1:1.
#[derive(Serialize, Deserialize)]
pub struct LicenseSummary {
    pub tier: String,
    pub status: String,
    pub license_id: String,
    pub features: Vec<String>,
    pub expires_at: Option<String>,
    pub key_id: String,
    pub seat_count: u32,
}

/// Mirrors `crates/api/src/health.rs::ReadyResponse` (`{ status, checks: { db } }`).
#[derive(Serialize, Deserialize)]
pub struct HealthInfo {
    pub status: String,
    pub db: String,
    /// True when HTTP 200; `/health/ready` returns 503 when the DB is degraded.
    pub reachable: bool,
}

/// Server `SetupStatus` (`crates/api/src/setup.rs`): `{ needs_setup }`.
#[derive(Serialize, Deserialize)]
pub struct SetupStatusInfo {
    pub needs_setup: bool,
}

/// What the webview receives after first-run setup: a live session plus the
/// slug the server assigned (to pre-fill "Sucursal" on later logins).
#[derive(Serialize)]
pub struct SetupInfo {
    pub user_id: String,
    pub tenant_id: String,
    pub roles: Vec<String>,
    pub expires_in: u64,
    pub tenant_slug: String,
}

/// Subset of `crates/domain/src/catalog/model.rs::ProductDto` the client needs
/// for the POS picker + inventory. Money (`price`) crosses the wire as a STRING
/// (`rust_decimal::serde::str`) — kept as `String` here and parsed/formatted in
/// the webview, never as f64.
#[derive(Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub price: String,
    pub stock: i64,
    pub active: bool,
    pub laboratory: Option<String>,
    pub active_ingredient: Option<String>,
    /// Tenant barcode when present (`ProductDto.barcode`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    /// Sum of active children stock when this row is a multi-SKU parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variants_stock: Option<i64>,
    /// Active children count when multi-SKU parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant_count: Option<i64>,
    /// Multi-SKU child → parent id (migración 0034).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

/// Mirrors `crates/domain/src/catalog/model.rs::ProductStats` (the
/// `/products/stats` payload). `inventory_value` is a STRING (Decimal).
#[derive(Serialize, Deserialize)]
pub struct InventorySummary {
    pub total: i64,
    pub active: i64,
    pub low_stock: i64,
    pub out_of_stock: i64,
    pub inventory_value: String,
    pub expired: i64,
}

/// Mirrors `crates/domain/src/expenses/model.rs::DailySalesRow`. Money fields
/// (`revenue`/`cash`/`card`) are STRINGS.
#[derive(Serialize, Deserialize)]
pub struct DailySalesRow {
    pub date: String,
    pub orders: i64,
    pub revenue: String,
    pub cash: String,
    pub card: String,
}

/// Mirrors `crates/domain/src/expenses/model.rs::TopProductRow`. `revenue` /
/// `revenue_pct` are STRINGS; `abc_class` is `"A" | "B" | "C"`.
#[derive(Serialize, Deserialize)]
pub struct TopProductRow {
    pub rank: i64,
    pub product_id: Option<String>,
    pub product_name: String,
    pub qty_sold: i64,
    pub revenue: String,
    pub revenue_pct: String,
    pub abc_class: String,
}

/// One cart line sent up from the webview → forwarded verbatim to
/// `POST /pos/sale` (`crates/domain/src/sales/model.rs::PosSaleItem`).
/// `unit_price` is a STRING per the server contract.
#[derive(Serialize, Deserialize)]
pub struct PosItem {
    pub product: String,
    pub product_name: String,
    pub quantity: i64,
    pub unit_price: String,
}

/// Mirrors `crates/domain/src/sales/model.rs::DevolucionDto` — a refund/devolución
/// header. `total_devuelto` crosses the wire as a STRING (Decimal); `order` is the
/// linked sale (absent for a standalone refund). Immutable once created.
#[derive(Serialize, Deserialize)]
pub struct Devolucion {
    pub id: String,
    pub order: Option<String>,
    pub tipo: String,
    pub motivo: String,
    pub notas: Option<String>,
    pub total_devuelto: String,
    pub metodo_reembolso: Option<String>,
    pub procesado_por: Option<String>,
    pub created_at: String,
}

/// One line sent up to `create_refund` → server `NewDevolucionItem`. snake_case
/// (serde deserializes the array elements directly — no camelCase rename, same as
/// `PosItem`). `unit_price` is a STRING; `restock` whether to return the unit to
/// stock (server defaults true, we always send it explicit).
#[derive(Serialize, Deserialize)]
pub struct RefundItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    pub product_name: String,
    pub quantity: i64,
    pub unit_price: String,
    pub restock: bool,
}

/// One row of the audit trail (`crate::v1::audit::AuditItem` server-side).
/// `before`/`after`/`metadata`/`record_id`/`table` may be null (schema gap noted
/// server-side — the table records method/path/status/ip today). `status` is the
/// HTTP status of the audited request.
#[derive(Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub created_at: String,
    pub user: Option<String>,
    pub user_email: Option<String>,
    pub table: Option<String>,
    pub record_id: Option<String>,
    pub action: String,
    pub method: String,
    pub path: String,
    pub status: Option<i64>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub payload_hash: Option<String>,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// Paginated audit-log response (`crate::v1::audit::AuditResponse`).
#[derive(Serialize, Deserialize)]
pub struct AuditPage {
    pub total: i64,
    pub items: Vec<AuditEntry>,
    pub limit: u32,
    pub offset: u32,
}

// --- cash register (caja) --------------------------------------------------

/// Mirrors `crates/domain/src/cash_register/model.rs::CashSessionDto`. Money
/// fields cross the wire as STRINGS (`rust_decimal::serde::str*`); the optional
/// closing/discrepancy fields are absent while the session is still open.
#[derive(Serialize, Deserialize)]
pub struct CashSession {
    pub id: String,
    pub user: String,
    pub register_name: String,
    /// Caja física y sucursal de la sesión (V2, migración 0041). Ausentes en
    /// sesiones abiertas antes de la migración y en cajas sueltas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub register: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub opening_cash: String,
    pub opening_notes: Option<String>,
    pub closing_cash_counted: Option<String>,
    pub closing_cash_expected: Option<String>,
    pub discrepancia: Option<String>,
    pub closing_notes: Option<String>,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub status: String,
}

/// Mirrors `crates/domain/src/cash_register/model.rs::CloseSummary`. Returned by
/// both `cash_arqueo` (preview, session still open) and `close_cash_session`.
/// Money fields are STRINGS.
#[derive(Serialize, Deserialize)]
pub struct CashCloseSummary {
    pub session: CashSession,
    pub cash_sales: String,
    pub movements_in: String,
    pub movements_out: String,
}

// --- admin settings --------------------------------------------------------

/// Mirrors `crates/domain/src/sales/model.rs::AdminSettingDto`. Key/value pair
/// (both STRINGS) with the last-write timestamp. `value` semantics depend on the
/// key (boolean "true"/"false", a number, free text — interpreted client-side).
#[derive(Serialize, Deserialize)]
pub struct AdminSetting {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

// --- customers -------------------------------------------------------------

/// Mirrors `crates/domain/src/customers/model.rs::CustomerDto` (search results).
#[derive(Serialize, Deserialize)]
pub struct Customer {
    pub id: String,
    pub name: String,
    pub rut: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub loyalty_points: i64,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Mirrors `customers/model.rs::CustomerDetailDto` (lifetime aggregates).
/// `total_spent` is a STRING (Decimal). Lives on `feat/customers-loyalty-history`.
#[derive(Serialize, Deserialize)]
pub struct CustomerDetail {
    pub id: String,
    pub name: String,
    pub rut: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub loyalty_points: i64,
    pub total_spent: String,
    pub visit_count: i64,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Mirrors `customers/model.rs::CustomerOrderDto` (one purchase-history row).
/// `total` is a STRING (Decimal). Lives on `feat/customers-loyalty-history`.
#[derive(Serialize, Deserialize)]
pub struct CustomerOrder {
    pub id: String,
    pub status: String,
    pub payment_method: String,
    pub total: String,
    pub items_count: i64,
    pub created_at: String,
}

/// Header-only projection of `domain::purchasing::model::PurchaseOrderDto`.
/// `total` is a STRING (`rust_decimal::serde::str`). `items` is omitted — the
/// list endpoint returns an empty vec anyway; detail fetches them separately.
#[derive(Serialize, Deserialize)]
pub struct PurchaseOrder {
    pub id: String,
    pub supplier: String,
    pub status: String,
    pub currency: String,
    pub total: String,
    pub notes: Option<String>,
    pub external_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Mirrors `crates/domain/src/expenses/model.rs::ExpenseDto`. `amount` crosses
/// the wire as a STRING (`rust_decimal::serde::str`); the optional links
/// (`cash_session`/`supplier`/`note`/`created_by`) are absent when unset.
#[derive(Serialize, Deserialize)]
pub struct Expense {
    pub id: String,
    pub category: String,
    pub description: String,
    pub amount: String,
    pub payment_method: String,
    pub cash_session: Option<String>,
    pub supplier: Option<String>,
    pub note: Option<String>,
    pub created_by: Option<String>,
    pub incurred_at: String,
    pub created_at: String,
}

/// Mirrors `domain::purchasing::model::SupplierDto`. No money fields.
#[derive(Serialize, Deserialize)]
pub struct Supplier {
    pub id: String,
    pub name: String,
    pub rut: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub default_invoice_format: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// One line of a purchase order (`domain::purchasing::model::PurchaseOrderItemDto`).
/// `unit_cost`/`subtotal` cross the wire as STRINGS (Decimal). `qty_received` is
/// the cumulative quantity already received against this line — drives the
/// remaining-to-receive math the receive flow sends.
#[derive(Serialize, Deserialize)]
pub struct PurchaseOrderItem {
    pub id: String,
    pub product: Option<String>,
    pub product_name: String,
    pub quantity: i64,
    #[serde(default)]
    pub qty_received: i64,
    pub unit_cost: String,
    pub subtotal: String,
}

/// Full purchase order WITH line items — the `GET /purchase-orders/{id}` shape.
/// Same header as [`PurchaseOrder`] plus the populated `items` vec. Money is
/// STRING throughout.
#[derive(Serialize, Deserialize)]
pub struct PurchaseOrderDetail {
    pub id: String,
    pub supplier: String,
    pub status: String,
    pub currency: String,
    pub total: String,
    pub notes: Option<String>,
    pub external_ref: Option<String>,
    pub items: Vec<PurchaseOrderItem>,
    pub created_at: String,
    pub updated_at: String,
}

/// One line sent up to `create_purchase_order` → server `NewPurchaseOrderItem`.
/// `product` is optional (absent ⇒ free-text off-catalog line); `unit_cost` is a
/// STRING (Decimal) forwarded verbatim.
#[derive(Serialize, Deserialize)]
pub struct NewPurchaseOrderItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    pub product_name: String,
    pub quantity: i64,
    pub unit_cost: String,
}

/// One line sent up to `receive_purchase_order` → server `ReceivePurchaseOrderLine`.
#[derive(Serialize, Deserialize)]
pub struct ReceiveLine {
    pub po_line_id: String,
    pub qty_received: i64,
}

/// Mirrors `crates/domain/src/purchasing/model.rs::PurchasePaymentDto` — one
/// recorded supplier payment against a PO. `amount` crosses the wire as STRING
/// (Decimal); `paid_at`/`created_at` are RFC3339 strings.
#[derive(Serialize, Deserialize)]
pub struct PurchasePayment {
    pub id: String,
    pub purchase_order: String,
    pub amount: String,
    pub currency: String,
    pub payment_method: String,
    pub cash_session: Option<String>,
    pub reference: Option<String>,
    pub note: Option<String>,
    pub paid_at: String,
    pub created_at: String,
}

/// Mirrors `PurchasePaymentSummary` — the accounts-payable rollup of a PO plus
/// its recorded payments. `total`/`paid`/`balance` are STRINGS (Decimal).
#[derive(Serialize, Deserialize)]
pub struct PurchasePaymentSummary {
    pub purchase_order: String,
    pub status: String,
    pub total: String,
    pub paid: String,
    pub balance: String,
    pub fully_paid: bool,
    pub payments: Vec<PurchasePayment>,
}

/// One printable line of a [`Receipt`] (`sales/model.rs::ReceiptItem`). Money
/// fields (`unit_price`/`line_total`) cross the wire as STRINGS.
#[derive(Serialize, Deserialize)]
pub struct ReceiptItem {
    pub name: String,
    pub qty: i64,
    pub unit_price: String,
    pub line_total: String,
}

/// Mirrors `crates/domain/src/sales/model.rs::ReceiptDto` — self-contained
/// printable boleta for a completed sale. Money fields are STRINGS;
/// `cash_amount`/`card_amount`/`change` are absent on tenders they don't apply
/// to (`change` is non-null only for cash sales).
#[derive(Serialize, Deserialize)]
pub struct Receipt {
    pub order_id: String,
    pub folio_or_number: String,
    pub datetime: String,
    pub tenant_name: String,
    pub items: Vec<ReceiptItem>,
    pub subtotal: String,
    pub discount: String,
    pub total: String,
    pub payment_method: String,
    pub cash_amount: Option<String>,
    pub card_amount: Option<String>,
    pub change: Option<String>,
    pub loyalty_points_awarded: i64,
    pub cashier: Option<String>,
    pub footer_note: String,
}

/// Mirrors `crates/domain/src/prescriptions/model.rs::PrescriptionDto`. A
/// prescription row is immutable (Ley 20.000) — the server exposes only
/// create/get/list, never update or delete. `product` / `customer` are optional
/// record ids; `controlled = true` marks a Ley 20.000 controlled-drug entry
/// (which the server requires `doctor_name` + `doctor_rut` for). `dispensed_at`
/// / `created_at` are RFC3339 strings.
#[derive(Serialize, Deserialize)]
pub struct Prescription {
    pub id: String,
    pub product: Option<String>,
    pub customer: Option<String>,
    pub patient_name: String,
    pub patient_rut: String,
    pub doctor_name: Option<String>,
    pub doctor_rut: Option<String>,
    pub controlled: bool,
    pub folio: Option<String>,
    pub dispensed_at: String,
    pub created_at: String,
}

// --- catalog detail / batches / near-expiry (Inventario lane) --------------

/// Mirrors `crates/api/src/v1/catalog.rs::ImportSummary` — outcome of a bulk CSV
/// product import. `created`/`updated`/`failed` are row counts; `errors` carries
/// the 1-based line + message for every rejected row so the view can list them.
#[derive(Serialize, Deserialize)]
pub struct ImportSummary {
    pub created: usize,
    pub updated: usize,
    pub failed: usize,
    pub errors: Vec<ImportRowError>,
}

/// One rejected CSV row (mirror of `catalog.rs::ImportError`).
#[derive(Serialize, Deserialize)]
pub struct ImportRowError {
    pub line: usize,
    pub message: String,
}

/// Full product detail (`crates/domain/src/catalog/model.rs::ProductDto`). Extra
/// server fields (timestamps, image_url, external_id, therapeutic_action are kept;
/// anything we don't render serde simply ignores). Money (`price`/`cost_price`)
/// crosses the wire as STRINGS.
#[derive(Serialize, Deserialize)]
pub struct ProductDetail {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub price: String,
    pub cost_price: Option<String>,
    pub stock: i64,
    pub category: Option<String>,
    pub active: bool,
    pub laboratory: Option<String>,
    pub therapeutic_action: Option<String>,
    pub active_ingredient: Option<String>,
    pub prescription_type: String,
    pub presentation: Option<String>,
    pub discount_percent: Option<i64>,
    /// Per-rubro flexible bag (P0.2). Serde ignores if the server omits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attrs: Option<serde_json::Value>,
    /// Multi-SKU child → parent product id (migración 0034). Absent on planos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Tenant barcode when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    /// Sum of active children stock when this row is a multi-SKU parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variants_stock: Option<i64>,
    /// Active children count when multi-SKU parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant_count: Option<i64>,
}

/// One product batch / lote (`domain::inventory::model::BatchDto`). `expiry_date`
/// is RFC3339; `cost` crosses as a STRING (Decimal) or null.
#[derive(Serialize, Deserialize)]
pub struct Batch {
    pub id: String,
    pub product: String,
    pub batch_code: String,
    pub expiry_date: String,
    pub stock: i64,
    pub cost: Option<String>,
    pub notes: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// One soon-to-expire (or expired) batch (`domain::expenses::model::NearExpiryRow`).
/// `days_to_expiry` < 0 ⇒ already expired (also flagged by `expired`).
#[derive(Serialize, Deserialize)]
pub struct NearExpiryRow {
    pub product_id: String,
    pub product_name: String,
    pub batch_id: String,
    pub batch_code: String,
    pub expiry_date: String,
    pub stock: i64,
    pub days_to_expiry: i64,
    pub expired: bool,
}

// --- reports expansion (margins / rotation / dashboard) --------------------

/// Mirrors `domain::expenses::model::DailyMarginRow`. Money fields
/// (`revenue`/`cost`/`margin`/`margin_pct`) are STRINGS. `items_without_cost`
/// counts line items excluded from `cost` (product unset or `cost_price` null)
/// so the margin reads honestly. The endpoint is Pro-gated — Free tier gets a
/// 402 surfaced as the `FEATURE_REQUIRES_UPGRADE` coded error.
#[derive(Serialize, Deserialize)]
pub struct DailyMarginRow {
    pub date: String,
    pub revenue: String,
    pub cost: String,
    pub margin: String,
    pub margin_pct: String,
    pub items_without_cost: i64,
}

/// Mirrors `domain::expenses::model::StockRotationRow`. `turnover` =
/// `qty_sold / current_stock`; both `turnover` and `days_of_inventory` are
/// STRINGS or null — null when current stock ≤ 0 (can't divide), and
/// `days_of_inventory` is also null unless a date window was supplied.
#[derive(Serialize, Deserialize)]
pub struct StockRotationRow {
    pub product_id: String,
    pub product_name: String,
    pub qty_sold: i64,
    pub current_stock: i64,
    pub turnover: Option<String>,
    pub days_of_inventory: Option<String>,
}

// --- DTE / boleta electrónica SII ------------------------------------------

/// Mirrors `crates/api/src/v1/dte.rs::DteDto`. `monto_total` crosses the wire
/// as a STRING (Decimal); `fecha_emision` is RFC3339. `has_xml` flags whether
/// `GET /dte/{id}/xml` has a signed XML to download.
#[derive(Serialize, Deserialize)]
pub struct Dte {
    pub id: String,
    pub tipo: i32,
    pub folio: i64,
    pub fecha_emision: String,
    pub rut_emisor: String,
    pub rut_receptor: String,
    pub razon_social_receptor: String,
    pub monto_total: String,
    pub estado: String,
    pub track_id: Option<i64>,
    pub sii_glosa: Option<String>,
    pub order_id: Option<String>,
    pub has_xml: bool,
}

/// A write the agent PROPOSES (`crates/api/src/v1/assist.rs` propose→confirm).
/// Present on [`AssistAnswer`] only when the question asked for a mutation. The
/// `confirm_token` is the single-use ticket the client must hand back to
/// `/assist/act` — `ask` itself mutates nothing.
#[derive(Serialize, Deserialize)]
pub struct AgentProposal {
    pub action: String,
    pub resumen: String,
    pub confirm_token: String,
}

/// Mirrors `crates/assist/src/provider.rs::Answer` — the agent's reply to one
/// "Pregúntale a tu negocio" question (ADR-0016). `intent` is the stable machine
/// label (`ventas_hoy` … `desconocido`); `text` is the Spanish prose grounded in
/// the tenant's own data; `data` is the optional structured payload (absent when
/// the intent carries no figures — `#[serde(default)]` so the missing field is
/// `None`, not a deserialize error). `proposal` is present only for a WRITE
/// question — the UI gates it behind an explicit confirmation.
#[derive(Serialize, Deserialize)]
pub struct AssistAnswer {
    pub intent: String,
    pub text: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal: Option<AgentProposal>,
}

// --- fiado / cuenta corriente ----------------------------------------------

/// Mirrors `crates/domain/src/credit/model.rs::LedgerEntryDto`. Money crosses
/// the wire as a STRING (`amount`).
#[derive(Serialize, Deserialize)]
pub struct LedgerEntry {
    pub id: String,
    /// `cargo` (el cliente debe) | `abono` (pagó).
    pub kind: String,
    pub amount: String,
    #[serde(default)]
    pub order: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    pub created_at: String,
}

/// Mirrors `credit/model.rs::CustomerAccountDto` — estado de cuenta del cliente.
#[derive(Serialize, Deserialize)]
pub struct CustomerAccount {
    pub customer: String,
    /// `total_charged - total_paid`. Positivo = el cliente debe.
    pub balance: String,
    pub total_charged: String,
    pub total_paid: String,
    pub entries: Vec<LedgerEntry>,
}

// --- libro de compras / IVA (compliance V3) ---------------------------------

/// Mirrors `domain/compliance/model.rs::PurchaseBookRow`. Money as STRING.
#[derive(Serialize, Deserialize)]
pub struct PurchaseBookRow {
    pub purchase_order: String,
    pub tipo: i32,
    #[serde(default)]
    pub folio: Option<String>,
    pub supplier_name: String,
    #[serde(default)]
    pub supplier_rut: Option<String>,
    pub date: String,
    pub neto: String,
    pub iva: String,
    pub total: String,
    /// `false` = neto/IVA derivados del total (falta capturar la factura real).
    pub declared: bool,
}

/// Mirrors `compliance/model.rs::PurchaseBook`.
#[derive(Serialize, Deserialize)]
pub struct PurchaseBook {
    pub period: String,
    pub rows: Vec<PurchaseBookRow>,
    pub total_neto: String,
    pub total_iva: String,
    pub total: String,
    pub pending_declaration: usize,
}

/// Mirrors `compliance/model.rs::IvaSummary` — la cifra que va al F29.
#[derive(Serialize, Deserialize)]
pub struct IvaSummary {
    pub period: String,
    pub iva_debito: String,
    pub iva_credito: String,
    pub iva_a_pagar: String,
    pub ventas_neto: String,
    pub compras_neto: String,
}

/// Mirrors `credit/model.rs::DebtorRow` — un cliente con deuda vigente.
#[derive(Serialize, Deserialize)]
pub struct DebtorRow {
    pub customer: String,
    pub name: String,
    #[serde(default)]
    pub phone: Option<String>,
    pub balance: String,
    pub last_movement: String,
}

/// Mirrors `credit/model.rs::DebtorsReport` — "¿cuánto me deben?".
#[derive(Serialize, Deserialize)]
pub struct DebtorsReport {
    pub total_por_cobrar: String,
    pub debtor_count: usize,
    pub rows: Vec<DebtorRow>,
}

//! Pharma Client — Tauri 2 backend.
//!
//! Thin HTTP client over the running `pharma-server` (`crates/api`). Three
//! commands wrap the real server contract:
//!   - `login`          → POST `/api/v1/login`   then GET `/api/v1/me`
//!   - `license_status` → GET  `/api/v1/admin/license/status` (Bearer)
//!   - `server_health`  → GET  `/health/ready`
//!
//! The JWT lives ONLY in `SessionState` (in-memory). It is never written to
//! disk — losing it on quit is intentional (re-login each launch, LoL-style).
//!
//! All user-facing error strings are in Spanish (project rule); identifiers and
//! `code` values stay English.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

/// In-memory session. Token is intentionally NOT persisted to disk.
#[derive(Default)]
pub struct SessionState {
    inner: Mutex<Option<Session>>,
}

#[derive(Clone)]
struct Session {
    token: String,
}

// ---------------------------------------------------------------------------
// Wire types — shaped to the REAL server contract (read from crates/api).
// ---------------------------------------------------------------------------

/// Server `LoginResponse` (`crates/api/src/routes.rs`): `{ token, token_type,
/// expires_in }`. Note the login body carries NO tenant/roles — those come from
/// `/api/v1/me`, so `login` makes a second call to enrich `SessionInfo`.
#[derive(Deserialize)]
struct LoginResponse {
    token: String,
    #[allow(dead_code)]
    token_type: String,
    expires_in: u64,
}

/// Server `Me` (`crates/api/src/routes.rs`): `{ sub, tenant_id, roles, exp }`.
#[derive(Deserialize)]
struct MeResponse {
    sub: String,
    tenant_id: String,
    roles: Vec<String>,
    #[allow(dead_code)]
    exp: i64,
}

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

/// Server error envelope (`crates/api/src/error.rs`):
/// `{ "error": { "code", "message", "details"? } }`.
#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    code: String,
    message: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .build()
        .map_err(|e| format!("No se pudo inicializar el cliente HTTP: {e}"))
}

/// Trim a trailing slash so `server_url` + "/path" never doubles up.
fn base(server_url: &str) -> &str {
    server_url.trim_end_matches('/')
}

/// Map a non-2xx response to a Spanish message, parsing the server envelope when
/// present. Falls back to a status-based message for non-JSON bodies.
async fn error_message(resp: reqwest::Response) -> String {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if let Ok(env) = serde_json::from_str::<ErrorEnvelope>(&body) {
        return env.error.message;
    }
    match status.as_u16() {
        401 => "Credenciales inválidas.".to_string(),
        403 => "Permiso denegado para esta operación.".to_string(),
        404 => "Recurso no encontrado en el servidor.".to_string(),
        503 => "Servicio no disponible. Intenta nuevamente.".to_string(),
        other => format!("Error del servidor ({other})."),
    }
}

/// Connection-level failures (server down, wrong URL) → friendly Spanish copy.
fn conn_error(e: reqwest::Error) -> String {
    if e.is_connect() || e.is_timeout() {
        "No se pudo conectar al servidor. Verifica la URL y que pharma-server esté corriendo."
            .to_string()
    } else {
        format!("Error de red: {e}")
    }
}

/// Pull the in-memory JWT or return the Spanish "no session" error. Shared by
/// every authenticated command (license_status, list_products, reports, POS).
fn token_of(state: &State<'_, SessionState>) -> Result<String, String> {
    let guard = state
        .inner
        .lock()
        .map_err(|_| "Estado de sesión corrupto.".to_string())?;
    guard
        .as_ref()
        .map(|s| s.token.clone())
        .ok_or_else(|| "No hay sesión activa. Inicia sesión primero.".to_string())
}

/// Like [`error_message`] but preserves the server `code` so the frontend can
/// branch on it (e.g. `INSUFFICIENT_STOCK`). Encodes as `"CODE|message"`; when
/// no envelope is present the code half is empty (`"|message"`). The JS side
/// splits on the first `|`.
async fn coded_error(resp: reqwest::Response) -> String {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if let Ok(env) = serde_json::from_str::<ErrorEnvelope>(&body) {
        return format!("{}|{}", env.error.code, env.error.message);
    }
    let msg = match status.as_u16() {
        401 => "Credenciales inválidas.".to_string(),
        403 => "Permiso denegado para esta operación.".to_string(),
        404 => "Recurso no encontrado en el servidor.".to_string(),
        422 => "No se pudo procesar la venta.".to_string(),
        503 => "Servicio no disponible. Intenta nuevamente.".to_string(),
        other => format!("Error del servidor ({other})."),
    };
    format!("|{msg}")
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// POST `/api/v1/login` with `{ tenant, email, password }`, then GET
/// `/api/v1/me` to resolve tenant_id + roles. Stores the JWT in `SessionState`.
#[tauri::command]
async fn login(
    state: State<'_, SessionState>,
    server_url: String,
    tenant: String,
    email: String,
    password: String,
) -> Result<SessionInfo, String> {
    let http = client()?;
    let base = base(&server_url);

    let resp = http
        .post(format!("{base}/api/v1/login"))
        .json(&serde_json::json!({
            "tenant": tenant,
            "email": email,
            "password": password,
        }))
        .send()
        .await
        .map_err(conn_error)?;

    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }

    let login: LoginResponse = resp
        .json()
        .await
        .map_err(|e| format!("Respuesta de login inválida del servidor: {e}"))?;

    // Enrich with identity from /me (login body has no tenant/roles).
    let me_resp = http
        .get(format!("{base}/api/v1/me"))
        .bearer_auth(&login.token)
        .send()
        .await
        .map_err(conn_error)?;

    if !me_resp.status().is_success() {
        return Err(error_message(me_resp).await);
    }

    let me: MeResponse = me_resp
        .json()
        .await
        .map_err(|e| format!("Respuesta de sesión inválida del servidor: {e}"))?;

    // Token in memory only.
    *state
        .inner
        .lock()
        .map_err(|_| "Estado de sesión corrupto.".to_string())? =
        Some(Session { token: login.token });

    Ok(SessionInfo {
        user_id: me.sub,
        tenant_id: me.tenant_id,
        roles: me.roles,
        expires_in: login.expires_in,
    })
}

// --- first-run setup (no account yet) --------------------------------------

/// Server `SetupStatus` (`crates/api/src/setup.rs`): `{ needs_setup }`.
#[derive(Serialize, Deserialize)]
pub struct SetupStatusInfo {
    pub needs_setup: bool,
}

/// Server `SetupResponse`: a login token plus the assigned tenant slug.
#[derive(Deserialize)]
struct SetupResponse {
    token: String,
    #[allow(dead_code)]
    token_type: String,
    expires_in: u64,
    tenant_slug: String,
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

/// GET `/api/v1/setup/status` — UNAUTHENTICATED. `needs_setup = true` when the
/// install has no account yet, so the login screen can offer in-app account
/// creation instead of a dead end.
#[tauri::command]
async fn setup_status(server_url: String) -> Result<SetupStatusInfo, String> {
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/setup/status"))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de estado de instalación inválida: {e}"))
}

/// POST `/api/v1/setup` — UNAUTHENTICATED first-run bootstrap. Creates the first
/// tenant + owner, stores the issued token, then GET `/me` to return identity —
/// so the operator is logged straight in. 409 if the install already has an
/// account (surfaces the server's Spanish message).
#[tauri::command]
async fn setup_account(
    state: State<'_, SessionState>,
    server_url: String,
    business_name: String,
    tenant_slug: Option<String>,
    email: String,
    password: String,
    vertical: Option<String>,
) -> Result<SetupInfo, String> {
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/setup"))
        .json(&serde_json::json!({
            "business_name": business_name,
            "tenant_slug": tenant_slug,
            "email": email,
            "password": password,
            "vertical": vertical,
        }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    let setup: SetupResponse = resp
        .json()
        .await
        .map_err(|e| format!("Respuesta de instalación inválida del servidor: {e}"))?;

    let me_resp = http
        .get(format!("{base}/api/v1/me"))
        .bearer_auth(&setup.token)
        .send()
        .await
        .map_err(conn_error)?;
    if !me_resp.status().is_success() {
        return Err(error_message(me_resp).await);
    }
    let me: MeResponse = me_resp
        .json()
        .await
        .map_err(|e| format!("Respuesta de sesión inválida del servidor: {e}"))?;

    *state
        .inner
        .lock()
        .map_err(|_| "Estado de sesión corrupto.".to_string())? =
        Some(Session { token: setup.token });

    Ok(SetupInfo {
        user_id: me.sub,
        tenant_id: me.tenant_id,
        roles: me.roles,
        expires_in: setup.expires_in,
        tenant_slug: setup.tenant_slug,
    })
}

/// GET `/api/v1/admin/license/status` (Bearer). Requires a prior `login`.
/// Admin/owner only on the server side — a 403 surfaces as Spanish copy.
#[tauri::command]
async fn license_status(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<LicenseSummary, String> {
    let token = token_of(&state)?;

    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/admin/license/status"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;

    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }

    resp.json()
        .await
        .map_err(|e| format!("Respuesta de licencia inválida del servidor: {e}"))
}

// --- catalog / inventory ---------------------------------------------------

/// GET `/api/v1/products` (Bearer). `search` filters by name/ingredient on the
/// server; `limit` caps rows. Returns the trimmed [`Product`] projection.
#[tauri::command]
async fn list_products(
    state: State<'_, SessionState>,
    server_url: String,
    search: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Product>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);

    let mut req = http
        .get(format!("{base}/api/v1/products"))
        .bearer_auth(token);
    if let Some(s) = search.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("search", s)]);
    }
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }

    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de productos inválida del servidor: {e}"))
}

/// GET `/api/v1/products/stats` (Bearer) → inventory KPIs (count, low/out of
/// stock, total valuation). Reused by both Inventario and Reportes.
#[tauri::command]
async fn inventory_summary(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<InventorySummary, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/products/stats"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de inventario inválida del servidor: {e}"))
}

// --- catalog writes + batches + near-expiry (Inventario lane) --------------
// Writes (`create_product`, `adjust_product_stock`, `create_batch`) require
// admin/owner server-side; a 403 surfaces as the Spanish permission copy via
// `error_message`. Empty optional fields are omitted so the server applies its
// own defaults (e.g. `prescription_type`, `stock = 0`).

/// POST `/api/v1/products` (Bearer, admin+). Money strings (`price`,
/// `cost_price`) forwarded verbatim. Returns the created product detail.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn create_product(
    state: State<'_, SessionState>,
    server_url: String,
    name: String,
    price: String,
    cost_price: Option<String>,
    stock: Option<i64>,
    category: Option<String>,
    laboratory: Option<String>,
    active_ingredient: Option<String>,
    prescription_type: Option<String>,
    presentation: Option<String>,
) -> Result<ProductDetail, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({ "name": name, "price": price });
    if let Some(v) = cost_price.filter(|s| !s.is_empty()) {
        body["cost_price"] = serde_json::Value::String(v);
    }
    if let Some(n) = stock {
        body["stock"] = serde_json::Value::from(n);
    }
    for (k, v) in [
        ("category", category),
        ("laboratory", laboratory),
        ("active_ingredient", active_ingredient),
        ("prescription_type", prescription_type),
        ("presentation", presentation),
    ] {
        if let Some(s) = v.filter(|s| !s.is_empty()) {
            body[k] = serde_json::Value::String(s);
        }
    }
    let resp = http
        .post(format!("{base}/api/v1/products"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de producto inválida del servidor: {e}"))
}

/// POST `/api/v1/products/import` (Bearer, admin+) — bulk CSV catalog load. The
/// view reads the file text in JS and hands it over as `csv`; we wrap it in a
/// multipart field (the server reads the first field, name-agnostic). Idempotent
/// upsert by `external_id` server-side. Returns per-row created/updated/failed.
#[tauri::command]
async fn import_products(
    state: State<'_, SessionState>,
    server_url: String,
    csv: String,
) -> Result<ImportSummary, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let part = reqwest::multipart::Part::text(csv)
        .file_name("import.csv")
        .mime_str("text/csv")
        .map_err(|e| format!("No se pudo preparar el archivo CSV: {e}"))?;
    let form = reqwest::multipart::Form::new().part("file", part);
    let resp = http
        .post(format!("{base}/api/v1/products/import"))
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de importación inválida del servidor: {e}"))
}

/// POST `/api/v1/products/import?dry_run=true` (Bearer, admin+) — validates and
/// counts the CSV WITHOUT writing anything. Backs the "preview antes de
/// confirmar" step: the operator sees how many rows are OK / rejected (and why)
/// before committing the catalog migration.
#[tauri::command]
async fn import_products_preview(
    state: State<'_, SessionState>,
    server_url: String,
    csv: String,
) -> Result<ImportSummary, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let part = reqwest::multipart::Part::text(csv)
        .file_name("import.csv")
        .mime_str("text/csv")
        .map_err(|e| format!("No se pudo preparar el archivo CSV: {e}"))?;
    let form = reqwest::multipart::Form::new().part("file", part);
    let resp = http
        .post(format!("{base}/api/v1/products/import?dry_run=true"))
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de previsualización inválida del servidor: {e}"))
}

/// GET `/api/v1/products/export` (Bearer) — full catalog as CSV text so the
/// webview can wrap it in a Blob download. Columns match the `import_products`
/// format (export → edit → re-import round-trip). No-lock-in pillar (ADR-0005 #4).
#[tauri::command]
async fn export_products(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<String, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/products/export"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.text()
        .await
        .map_err(|e| format!("Respuesta de exportación inválida del servidor: {e}"))
}

/// GET `/api/v1/products/{id}` (Bearer) — full product detail for the drawer.
#[tauri::command]
async fn product_detail(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<ProductDetail, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/products/{id}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de producto inválida del servidor: {e}"))
}

/// POST `/api/v1/products/{id}/stock` (Bearer, admin+). Body `StockAdjust`:
/// either `set` (absolute) or `delta` (signed) + optional `reason`. Returns the
/// updated product.
#[tauri::command]
async fn adjust_product_stock(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    set: Option<i64>,
    delta: Option<i64>,
    reason: Option<String>,
) -> Result<ProductDetail, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({});
    if let Some(n) = set {
        body["set"] = serde_json::Value::from(n);
    }
    if let Some(n) = delta {
        body["delta"] = serde_json::Value::from(n);
    }
    if let Some(r) = reason.filter(|s| !s.is_empty()) {
        body["reason"] = serde_json::Value::String(r);
    }
    let resp = http
        .post(format!("{base}/api/v1/products/{id}/stock"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de ajuste inválida del servidor: {e}"))
}

/// GET `/api/v1/batches` (Bearer). Filters: `product` (record id),
/// `expiring_within_days`, `only_available`, `limit`. Returns lotes.
#[tauri::command]
async fn list_batches(
    state: State<'_, SessionState>,
    server_url: String,
    product: Option<String>,
    expiring_within_days: Option<i64>,
    only_available: Option<bool>,
    limit: Option<u32>,
) -> Result<Vec<Batch>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/batches"))
        .bearer_auth(token);
    if let Some(p) = product.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("product", p)]);
    }
    if let Some(d) = expiring_within_days {
        req = req.query(&[("expiring_within_days", d)]);
    }
    if let Some(a) = only_available {
        req = req.query(&[("only_available", a)]);
    }
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de lotes inválida del servidor: {e}"))
}

/// POST `/api/v1/batches` (Bearer, admin+). Body `NewBatch`: `product`,
/// `batch_code`, `expiry_date` (RFC3339), optional `stock`/`cost`/`notes`. An
/// initial `stock` > 0 emits a `batch_received` stock movement server-side.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn create_batch(
    state: State<'_, SessionState>,
    server_url: String,
    product: String,
    batch_code: String,
    expiry_date: String,
    stock: Option<i64>,
    cost: Option<String>,
    notes: Option<String>,
) -> Result<Batch, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "product": product,
        "batch_code": batch_code,
        "expiry_date": expiry_date,
    });
    if let Some(n) = stock {
        body["stock"] = serde_json::Value::from(n);
    }
    if let Some(c) = cost.filter(|s| !s.is_empty()) {
        body["cost"] = serde_json::Value::String(c);
    }
    if let Some(t) = notes.filter(|s| !s.is_empty()) {
        body["notes"] = serde_json::Value::String(t);
    }
    let resp = http
        .post(format!("{base}/api/v1/batches"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de lote inválida del servidor: {e}"))
}

/// GET `/api/v1/reports/near-expiry?days=N` (Bearer). Batches expiring within
/// `days` (default 30 server-side) including already-expired, urgent first.
#[tauri::command]
async fn near_expiry(
    state: State<'_, SessionState>,
    server_url: String,
    days: Option<i64>,
) -> Result<Vec<NearExpiryRow>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/reports/near-expiry"))
        .bearer_auth(token);
    if let Some(d) = days {
        req = req.query(&[("days", d)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de vencimientos inválida del servidor: {e}"))
}

// --- reports ---------------------------------------------------------------

/// GET `/api/v1/reports/sales-daily` (Bearer). One row per UTC day with orders
/// + revenue split by tender. Free tier (sales-daily is core, not gated).
#[tauri::command]
async fn sales_daily(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<Vec<DailySalesRow>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/reports/sales-daily"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de ventas inválida del servidor: {e}"))
}

/// GET `/api/v1/reports/top-products?limit=N` (Bearer). Pareto ABC ranking by
/// revenue over the default window.
#[tauri::command]
async fn top_products(
    state: State<'_, SessionState>,
    server_url: String,
    limit: Option<u32>,
) -> Result<Vec<TopProductRow>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/reports/top-products"))
        .bearer_auth(token);
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de ranking inválida del servidor: {e}"))
}

/// GET `/api/v1/reports/margins-daily` (Bearer). **Pro-gated**: Free tier gets a
/// 402 `FEATURE_REQUIRES_UPGRADE`. Rejects with the `"CODE|message"` shape (see
/// [`coded_error`]) so the Reportes view can branch on `FEATURE_REQUIRES_UPGRADE`
/// and render an upgrade note instead of a hard error. `from`/`to` are optional
/// RFC3339 date-range filters forwarded as query params.
#[tauri::command]
async fn margins_daily(
    state: State<'_, SessionState>,
    server_url: String,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<DailyMarginRow>, String> {
    let token = token_of(&state).map_err(|e| format!("|{e}"))?;
    let http = client().map_err(|e| format!("|{e}"))?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/reports/margins-daily"))
        .bearer_auth(token);
    if let Some(f) = from.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("from", f)]);
    }
    if let Some(t) = to.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("to", t)]);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("|{}", conn_error(e)))?;
    // Non-2xx (incl. 402 FEATURE_REQUIRES_UPGRADE) → coded "CODE|message".
    if !resp.status().is_success() {
        return Err(coded_error(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("|Respuesta de márgenes inválida del servidor: {e}"))
}

/// GET `/api/v1/reports/stock-rotation` (Bearer). Inventory turnover per product
/// over the window (`qty_sold / current_stock`). `from`/`to` optional RFC3339.
#[tauri::command]
async fn stock_rotation(
    state: State<'_, SessionState>,
    server_url: String,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<StockRotationRow>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/reports/stock-rotation"))
        .bearer_auth(token);
    if let Some(f) = from.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("from", f)]);
    }
    if let Some(t) = to.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("to", t)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de rotación inválida del servidor: {e}"))
}

/// GET `/api/v1/reports/dashboard` (Bearer, admin/owner/quimico). One aggregate
/// exec summary (ventas hoy/mes, inventario, top productos, por_vencer, margen
/// hoy). `margen_hoy` is `null` on the Free tier (the server degrades that one
/// field instead of 402-ing the whole overview). Returns the raw JSON object —
/// the webview reads the typed `DashboardSummary` shape from it.
#[tauri::command]
async fn dashboard_report(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/reports/dashboard"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta del panel inválida del servidor: {e}"))
}

// --- POS sale --------------------------------------------------------------

/// POST `/api/v1/pos/sale` (Bearer + a FRESH `Idempotency-Key` minted here so
/// every "Cobrar" click is a distinct sale). `cash_amount` / `card_amount` are
/// optional STRINGS forwarded verbatim. On a non-2xx, the error is returned as
/// `"CODE|message"` (see [`coded_error`]) so the UI can special-case
/// `INSUFFICIENT_STOCK`. Returns the raw server JSON (order + items + alerts).
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn pos_sale(
    state: State<'_, SessionState>,
    server_url: String,
    items: Vec<PosItem>,
    payment_method: String,
    cash_amount: Option<String>,
    card_amount: Option<String>,
    customer: Option<String>,
    discount: Option<String>,
) -> Result<serde_json::Value, String> {
    if items.is_empty() {
        return Err("|El carrito está vacío.".to_string());
    }
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);

    let mut body = serde_json::json!({
        "items": items,
        "payment_method": payment_method,
    });
    if let Some(c) = cash_amount.filter(|s| !s.is_empty()) {
        body["cash_amount"] = serde_json::Value::String(c);
    }
    if let Some(c) = card_amount.filter(|s| !s.is_empty()) {
        body["card_amount"] = serde_json::Value::String(c);
    }
    // Single global discount amount (sum of the POS's per-line + global
    // discounts). The server clamps it into [0, subtotal].
    if let Some(d) = discount.filter(|s| !s.is_empty()) {
        body["discount"] = serde_json::Value::String(d);
    }
    // Attach the customer record id so the server awards loyalty for the sale.
    if let Some(c) = customer.filter(|s| !s.is_empty()) {
        body["customer"] = serde_json::Value::String(c);
    }

    let key = uuid::Uuid::new_v4().to_string();
    let resp = http
        .post(format!("{base}/api/v1/pos/sale"))
        .bearer_auth(token)
        .header("Idempotency-Key", key)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;

    if !resp.status().is_success() {
        return Err(coded_error(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("|Respuesta de venta inválida del servidor: {e}"))
}

// --- returns / devoluciones commands ---------------------------------------

/// POST `/api/v1/pos/returns` (Bearer, cashier+) — create a refund/devolución.
/// `items` is forwarded verbatim (each `unit_price` a STRING, `restock` per line);
/// `order` links the devolución to a sale so the server can mark it refunded and
/// restock. Returns the raw `RefundResponse` JSON (`{ devolucion, items,
/// stock_movements, order_marked_refunded }`).
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn create_refund(
    state: State<'_, SessionState>,
    server_url: String,
    order: Option<String>,
    tipo: String,
    motivo: String,
    notas: Option<String>,
    items: Vec<RefundItem>,
    metodo_reembolso: Option<String>,
) -> Result<serde_json::Value, String> {
    if items.is_empty() {
        return Err("La devolución requiere al menos un ítem.".to_string());
    }
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "tipo": tipo,
        "motivo": motivo,
        "items": items,
    });
    if let Some(v) = order.filter(|s| !s.is_empty()) {
        body["order"] = serde_json::Value::String(v);
    }
    if let Some(v) = notas.filter(|s| !s.is_empty()) {
        body["notas"] = serde_json::Value::String(v);
    }
    if let Some(v) = metodo_reembolso.filter(|s| !s.is_empty()) {
        body["metodo_reembolso"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/pos/returns"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de devolución inválida del servidor: {e}"))
}

/// GET `/api/v1/returns` (Bearer) — devoluciones del tenant, newest first.
/// Optional `order` / `tipo` / `limit` filters are forwarded.
#[tauri::command]
async fn list_refunds(
    state: State<'_, SessionState>,
    server_url: String,
    order: Option<String>,
    tipo: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Devolucion>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/returns"))
        .bearer_auth(token);
    if let Some(v) = order.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("order", v)]);
    }
    if let Some(v) = tipo.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("tipo", v)]);
    }
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de devoluciones inválida del servidor: {e}"))
}

// --- auditoría / audit-log command -----------------------------------------

/// GET `/api/v1/admin/audit-log` (Bearer, admin/owner) — immutable audit trail,
/// tenant-scoped. Optional filters: `from`/`to` (YYYY-MM-DD), `user` (record id),
/// `table`, `action` (create|update|delete), `limit` (1..=500), `offset`. A 403
/// (non-admin) surfaces as the server's Spanish message.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn query_audit_log(
    state: State<'_, SessionState>,
    server_url: String,
    from: Option<String>,
    to: Option<String>,
    user: Option<String>,
    table: Option<String>,
    action: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<AuditPage, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/admin/audit-log"))
        .bearer_auth(token);
    if let Some(v) = from.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("from", v)]);
    }
    if let Some(v) = to.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("to", v)]);
    }
    if let Some(v) = user.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("user", v)]);
    }
    if let Some(v) = table.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("table", v)]);
    }
    if let Some(v) = action.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("action", v)]);
    }
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    if let Some(n) = offset {
        req = req.query(&[("offset", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de auditoría inválida del servidor: {e}"))
}

// --- cash register (caja) commands -----------------------------------------

/// GET `/api/v1/cash-sessions?status=open&limit=1` (Bearer). Returns the list
/// (newest first server-side); the view picks `[0]` as the current open session
/// or shows "sin caja abierta" when empty. Caja is core/free on every tier.
#[tauri::command]
async fn cash_sessions(
    state: State<'_, SessionState>,
    server_url: String,
    status: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<CashSession>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/cash-sessions"))
        .bearer_auth(token);
    if let Some(s) = status.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("status", s)]);
    }
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de cajas inválida del servidor: {e}"))
}

/// POST `/api/v1/cash-sessions` (Bearer) — open a register. Body shape:
/// `{ register_name, opening_cash (STRING), notes? }`. Returns the new session.
#[tauri::command]
async fn open_cash_session(
    state: State<'_, SessionState>,
    server_url: String,
    register_name: String,
    opening_cash: String,
    notes: Option<String>,
) -> Result<CashSession, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "register_name": register_name,
        "opening_cash": opening_cash,
    });
    if let Some(n) = notes.filter(|s| !s.is_empty()) {
        body["notes"] = serde_json::Value::String(n);
    }
    let resp = http
        .post(format!("{base}/api/v1/cash-sessions"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de apertura inválida del servidor: {e}"))
}

/// GET `/api/v1/cash-sessions/{id}/arqueo` (Bearer) — non-mutating close
/// preview: expected vs (no count yet). Used to show the operator the expected
/// cash before they count + confirm the close.
#[tauri::command]
async fn cash_arqueo(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<CashCloseSummary, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/cash-sessions/{id}/arqueo"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de arqueo inválida del servidor: {e}"))
}

/// POST `/api/v1/cash-sessions/{id}/close` (Bearer). Body:
/// `{ closing_cash_counted (STRING), notes? }`. Returns the close summary with
/// expected vs counted + the discrepancy on the embedded session.
#[tauri::command]
async fn close_cash_session(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    closing_cash_counted: String,
    notes: Option<String>,
) -> Result<CashCloseSummary, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "closing_cash_counted": closing_cash_counted,
    });
    if let Some(n) = notes.filter(|s| !s.is_empty()) {
        body["notes"] = serde_json::Value::String(n);
    }
    let resp = http
        .post(format!("{base}/api/v1/cash-sessions/{id}/close"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de cierre inválida del servidor: {e}"))
}

// --- admin settings commands -----------------------------------------------

/// GET `/api/v1/settings/{key}` (Bearer). The setting is optional: an unset key
/// returns 404 server-side, which we map to `Ok(None)` so the Configuración view
/// renders the default/empty state instead of a hard error.
#[tauri::command]
async fn get_setting(
    state: State<'_, SessionState>,
    server_url: String,
    key: String,
) -> Result<Option<AdminSetting>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/settings/{key}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if resp.status().as_u16() == 404 {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map(Some)
        .map_err(|e| format!("Respuesta de configuración inválida del servidor: {e}"))
}

/// PUT `/api/v1/settings/{key}` (Bearer, admin+). Body `{ value }`. Upserts the
/// key and returns the stored setting. 403 surfaces as the server's role error.
#[tauri::command]
async fn set_setting(
    state: State<'_, SessionState>,
    server_url: String,
    key: String,
    value: String,
) -> Result<AdminSetting, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .put(format!("{base}/api/v1/settings/{key}"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "value": value }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de configuración inválida del servidor: {e}"))
}

// --- demo seeding (multi-rubro onboarding) ----------------------------------

/// Sentinel the `seed_demo` command rejects with on 409 (demo data already
/// present, `force` false) so the Configuración view can offer a "regenerar"
/// confirm instead of surfacing a raw conflict error.
const SEED_ALREADY_EXISTS: &str = "SEED_ALREADY_EXISTS";

/// POST `/api/v1/admin/seed-demo` (Bearer, admin/owner). Fills the JWT's tenant
/// with a believable DEMO catalog for `vertical` (`pharmacy` | `minimarket`).
/// `force` wipes the prior demo pack before re-seeding. A 409 (data exists,
/// `force` false) maps to [`SEED_ALREADY_EXISTS`]; other failures surface the
/// server's Spanish message.
#[tauri::command]
async fn seed_demo(
    state: State<'_, SessionState>,
    server_url: String,
    vertical: String,
    force: bool,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/admin/seed-demo"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "vertical": vertical, "force": force }))
        .send()
        .await
        .map_err(conn_error)?;
    if resp.status().as_u16() == 409 {
        return Err(SEED_ALREADY_EXISTS.to_string());
    }
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de seed inválida del servidor: {e}"))
}

// --- customers commands -----------------------------------------------------
//
// The `/api/v1/customers/{search,{id},{id}/history}` surface lives on
// `feat/customers-loyalty-history`, which is NOT guaranteed to be merged into
// this client's server. So every customer command treats a 404 as a soft
// "module not deployed" signal and rejects with a STABLE sentinel string the
// view recognises — the Clientes view then renders a friendly upgrade note
// instead of a hard error. Any other status maps through `error_message`.

/// Sentinel the customer commands reject with on 404 so the view can branch and
/// show "módulo requiere merge de customers-loyalty" instead of a raw error.
const CUSTOMERS_MISSING: &str = "CUSTOMERS_MODULE_MISSING";

/// GET `/api/v1/customers/search?q=` (Bearer). 404 → [`CUSTOMERS_MISSING`].
#[tauri::command]
async fn customer_search(
    state: State<'_, SessionState>,
    server_url: String,
    q: String,
) -> Result<Vec<Customer>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/customers/search"))
        .query(&[("q", &q)])
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if resp.status().as_u16() == 404 {
        return Err(CUSTOMERS_MISSING.to_string());
    }
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de clientes inválida del servidor: {e}"))
}

/// GET `/api/v1/customers/{id}` (Bearer) — detail w/ lifetime aggregates.
/// 404 → [`CUSTOMERS_MISSING`] (the endpoint itself, not a missing customer;
/// a real missing-customer comes back through the server envelope on the merged
/// branch, but here a 404 most likely means the route doesn't exist).
#[tauri::command]
async fn customer_detail(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<CustomerDetail, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/customers/{id}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if resp.status().as_u16() == 404 {
        return Err(CUSTOMERS_MISSING.to_string());
    }
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de cliente inválida del servidor: {e}"))
}

/// POST `/api/v1/clientes` (Bearer, cashier+) — register a new customer at the
/// counter. Body `NewCustomer`: `name` (required) + optional `rut`/`phone`/
/// `email`; empty optionals are omitted so the server stores `null`. A 404 means
/// the customers module isn't deployed → [`CUSTOMERS_MISSING`] (same soft-degrade
/// as the read commands; this Spanish write surface ships on the same branch).
/// Returns the created [`Customer`].
#[tauri::command]
async fn create_customer(
    state: State<'_, SessionState>,
    server_url: String,
    name: String,
    rut: Option<String>,
    phone: Option<String>,
    email: Option<String>,
) -> Result<Customer, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({ "name": name });
    for (k, v) in [("rut", rut), ("phone", phone), ("email", email)] {
        if let Some(s) = v.filter(|s| !s.is_empty()) {
            body[k] = serde_json::Value::String(s);
        }
    }
    let resp = http
        .post(format!("{base}/api/v1/clientes"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if resp.status().as_u16() == 404 {
        return Err(CUSTOMERS_MISSING.to_string());
    }
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de cliente inválida del servidor: {e}"))
}

/// PATCH `/api/v1/clientes/{id}` (Bearer, cashier+) — edit a customer. Body
/// `UpdateCustomer`: every field optional (`name`/`rut`/`phone`/`email`/`active`).
/// Only fields explicitly provided are sent so omitted ones stay untouched; text
/// fields are forwarded verbatim (an empty string clears them), and `active` is
/// sent as a bool when present (activar/desactivar). 404 → [`CUSTOMERS_MISSING`].
/// Returns the updated [`Customer`].
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn update_customer(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    name: Option<String>,
    rut: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    active: Option<bool>,
) -> Result<Customer, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({});
    for (k, v) in [
        ("name", name),
        ("rut", rut),
        ("phone", phone),
        ("email", email),
    ] {
        if let Some(s) = v {
            body[k] = serde_json::Value::String(s);
        }
    }
    if let Some(a) = active {
        body["active"] = serde_json::Value::Bool(a);
    }
    let resp = http
        .patch(format!("{base}/api/v1/clientes/{id}"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if resp.status().as_u16() == 404 {
        return Err(CUSTOMERS_MISSING.to_string());
    }
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de cliente inválida del servidor: {e}"))
}

/// GET `/api/v1/customers/{id}/history?limit=N` (Bearer). 404 →
/// [`CUSTOMERS_MISSING`]. Read-only projection of the customer's orders.
#[tauri::command]
async fn customer_history(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    limit: Option<u32>,
) -> Result<Vec<CustomerOrder>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/customers/{id}/history"))
        .bearer_auth(token);
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if resp.status().as_u16() == 404 {
        return Err(CUSTOMERS_MISSING.to_string());
    }
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de historial inválida del servidor: {e}"))
}

// --- purchasing commands ---------------------------------------------------

/// GET `/api/v1/purchase-orders` (Bearer, cashier+). `status` / `limit` are
/// optional query params forwarded to the server. Returns header-only rows
/// (no line items). Requires cashier+ role — identical to cash-sessions access.
#[tauri::command]
async fn list_purchase_orders(
    state: State<'_, SessionState>,
    server_url: String,
    status: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<PurchaseOrder>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/purchase-orders"))
        .bearer_auth(token);
    if let Some(s) = status.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("status", s)]);
    }
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de órdenes de compra inválida del servidor: {e}"))
}

// --- expenses (gastos / caja chica) commands -------------------------------

/// GET `/api/v1/expenses` (Bearer, cashier+). Optional `category` /
/// `payment_method` filters + `limit`. Returns the tenant's expenses
/// (egresos / caja chica). Requires cashier+ role — same ladder as the POS.
#[tauri::command]
async fn list_expenses(
    state: State<'_, SessionState>,
    server_url: String,
    category: Option<String>,
    payment_method: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Expense>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/expenses"))
        .bearer_auth(token);
    if let Some(c) = category.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("category", c)]);
    }
    if let Some(p) = payment_method.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("payment_method", p)]);
    }
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de gastos inválida del servidor: {e}"))
}

/// POST `/api/v1/expenses` (Bearer, cashier+). Body `NewExpense`: `category`,
/// `description`, `amount` (STRING, forwarded verbatim), optional
/// `payment_method` (defaults to `cash` server-side), `note`, and `incurred_at`
/// (RFC3339). Returns the created expense.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn create_expense(
    state: State<'_, SessionState>,
    server_url: String,
    category: String,
    description: String,
    amount: String,
    payment_method: Option<String>,
    note: Option<String>,
    incurred_at: Option<String>,
) -> Result<Expense, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "category": category,
        "description": description,
        "amount": amount,
    });
    if let Some(p) = payment_method.filter(|s| !s.is_empty()) {
        body["payment_method"] = serde_json::Value::String(p);
    }
    if let Some(n) = note.filter(|s| !s.is_empty()) {
        body["note"] = serde_json::Value::String(n);
    }
    if let Some(t) = incurred_at.filter(|s| !s.is_empty()) {
        body["incurred_at"] = serde_json::Value::String(t);
    }
    let resp = http
        .post(format!("{base}/api/v1/expenses"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de gasto inválida del servidor: {e}"))
}

// --- prescriptions (recetas / controlados) ---------------------------------
//
// Ley 20.000 immutable log. The server exposes create/get/list for all
// prescriptions plus a controlled-only ledger (`/libro-recetas`) and its CSV
// export (ISP/DEIS). Creating a prescription requires pharmacist+ on the server
// (a 403 surfaces as Spanish copy); reads are open to any authenticated user.

/// GET `/api/v1/prescriptions` (Bearer). Optional filters: `patient_rut`,
/// `controlled` (true → only controlled), `limit`. Newest first server-side.
#[tauri::command]
async fn list_prescriptions(
    state: State<'_, SessionState>,
    server_url: String,
    patient_rut: Option<String>,
    controlled: Option<bool>,
    limit: Option<u32>,
) -> Result<Vec<Prescription>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/prescriptions"))
        .bearer_auth(token);
    if let Some(r) = patient_rut.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("patient_rut", r)]);
    }
    if let Some(c) = controlled {
        req = req.query(&[("controlled", c)]);
    }
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de recetas inválida del servidor: {e}"))
}

/// GET `/api/v1/prescriptions/{id}` (Bearer) — single prescription detail.
#[tauri::command]
async fn get_prescription(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<Prescription, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/prescriptions/{id}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de receta inválida del servidor: {e}"))
}

/// POST `/api/v1/prescriptions` (Bearer, pharmacist+). Body mirrors
/// `NewPrescription`: `patient_name` / `patient_rut` required; `doctor_name` +
/// `doctor_rut` required by the server when `controlled = true`. `product` /
/// `customer` are optional record ids; `dispensed_at` defaults to now server-side.
/// Empty optional strings are dropped so the server applies its own defaults.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn create_prescription(
    state: State<'_, SessionState>,
    server_url: String,
    patient_name: String,
    patient_rut: String,
    controlled: bool,
    doctor_name: Option<String>,
    doctor_rut: Option<String>,
    product: Option<String>,
    customer: Option<String>,
    folio: Option<String>,
) -> Result<Prescription, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "patient_name": patient_name,
        "patient_rut": patient_rut,
        "controlled": controlled,
    });
    for (key, val) in [
        ("doctor_name", doctor_name),
        ("doctor_rut", doctor_rut),
        ("product", product),
        ("customer", customer),
        ("folio", folio),
    ] {
        if let Some(v) = val.filter(|s| !s.is_empty()) {
            body[key] = serde_json::Value::String(v);
        }
    }
    let resp = http
        .post(format!("{base}/api/v1/prescriptions"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de receta inválida del servidor: {e}"))
}

/// GET `/api/v1/libro-recetas` (Bearer) — controlled-only ledger (Ley 20.000).
/// Same shape as `list_prescriptions` but `controlled = true` is enforced
/// server-side. Optional `patient_rut` / `limit` filters are forwarded.
#[tauri::command]
async fn libro_recetas(
    state: State<'_, SessionState>,
    server_url: String,
    patient_rut: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Prescription>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/libro-recetas"))
        .bearer_auth(token);
    if let Some(r) = patient_rut.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("patient_rut", r)]);
    }
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta del libro de recetas inválida del servidor: {e}"))
}

/// GET `/api/v1/libro-recetas/export` (Bearer) — CSV of the controlled-drug
/// ledger (ISP/DEIS). The server responds with `text/csv`; we return the raw
/// CSV text so the webview can trigger a Blob download. Optional `patient_rut`
/// filter is forwarded.
#[tauri::command]
async fn export_libro_recetas(
    state: State<'_, SessionState>,
    server_url: String,
    patient_rut: Option<String>,
) -> Result<String, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/libro-recetas/export"))
        .bearer_auth(token);
    if let Some(r) = patient_rut.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("patient_rut", r)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.text()
        .await
        .map_err(|e| format!("Respuesta de exportación inválida del servidor: {e}"))
}

/// GET `/api/v1/purchase-orders/{id}` (Bearer, cashier+) — full PO WITH line
/// items. Unlike `list_purchase_orders` (header-only), this populates `items`
/// so the detail drawer can show what was ordered and what's left to receive.
#[tauri::command]
async fn get_purchase_order(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<PurchaseOrderDetail, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/purchase-orders/{id}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de orden de compra inválida del servidor: {e}"))
}

/// GET `/api/v1/purchase-orders/{id}/payments` (Bearer, cashier+) — accounts-
/// payable rollup of a PO: `total` / `paid` / `balance` (STRING Decimals),
/// `fully_paid`, and the recorded payments. Drives the "Cuenta por pagar" block
/// of the PO detail drawer.
#[tauri::command]
async fn get_po_payments(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<PurchasePaymentSummary, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/purchase-orders/{id}/payments"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de pagos de OC inválida del servidor: {e}"))
}

/// POST `/api/v1/purchase-orders/{id}/payments` (Bearer, admin+) — record a
/// supplier payment against a PO. `amount` is a STRING (Decimal) forwarded
/// verbatim; `payment_method` is `cash`/`bank`/`card`/`transfer`. `cash_session`
/// is required by the server when paying `cash` with an open drawer (so the
/// outflow shows in the arqueo). Empty optionals are omitted. Returns the
/// created payment.
#[tauri::command]
async fn create_po_payment(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    amount: String,
    payment_method: Option<String>,
    cash_session: Option<String>,
    reference: Option<String>,
) -> Result<PurchasePayment, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({ "amount": amount });
    if let Some(v) = payment_method.filter(|s| !s.is_empty()) {
        body["payment_method"] = serde_json::Value::String(v);
    }
    if let Some(v) = cash_session.filter(|s| !s.is_empty()) {
        body["cash_session"] = serde_json::Value::String(v);
    }
    if let Some(v) = reference.filter(|s| !s.is_empty()) {
        body["reference"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/purchase-orders/{id}/payments"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de pago de OC inválida del servidor: {e}"))
}

/// GET `/api/v1/suppliers` (Bearer, cashier+). `search` filters by name on the
/// server; `limit` caps rows. Used by the suppliers list and the "Nueva OC"
/// supplier picker.
#[tauri::command]
async fn list_suppliers(
    state: State<'_, SessionState>,
    server_url: String,
    search: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Supplier>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/suppliers"))
        .bearer_auth(token);
    if let Some(s) = search.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("search", s)]);
    }
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de proveedores inválida del servidor: {e}"))
}

/// POST `/api/v1/suppliers` (Bearer, admin+) — create a supplier. Body:
/// `{ name, rut?, contact_name?, contact_email?, contact_phone? }`. Only `name`
/// is required; empty optionals are omitted. Returns the created [`Supplier`].
#[tauri::command]
async fn create_supplier(
    state: State<'_, SessionState>,
    server_url: String,
    name: String,
    rut: Option<String>,
    contact_name: Option<String>,
    contact_email: Option<String>,
    contact_phone: Option<String>,
) -> Result<Supplier, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({ "name": name });
    if let Some(v) = rut.filter(|s| !s.is_empty()) {
        body["rut"] = serde_json::Value::String(v);
    }
    if let Some(v) = contact_name.filter(|s| !s.is_empty()) {
        body["contact_name"] = serde_json::Value::String(v);
    }
    if let Some(v) = contact_email.filter(|s| !s.is_empty()) {
        body["contact_email"] = serde_json::Value::String(v);
    }
    if let Some(v) = contact_phone.filter(|s| !s.is_empty()) {
        body["contact_phone"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/suppliers"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de proveedor inválida del servidor: {e}"))
}

/// POST `/api/v1/purchase-orders` (Bearer, admin+) — create a draft PO. `items`
/// is forwarded verbatim; each line's `unit_cost` is a STRING (Decimal) and the
/// server computes the per-line `subtotal` + header `total`. `currency` defaults
/// to CLP server-side when omitted. Returns the created PO header.
#[tauri::command]
async fn create_purchase_order(
    state: State<'_, SessionState>,
    server_url: String,
    supplier: String,
    items: Vec<NewPurchaseOrderItem>,
    currency: Option<String>,
    notes: Option<String>,
    external_ref: Option<String>,
) -> Result<PurchaseOrder, String> {
    if items.is_empty() {
        return Err("La orden de compra requiere al menos un ítem.".to_string());
    }
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "supplier": supplier,
        "items": items,
    });
    if let Some(v) = currency.filter(|s| !s.is_empty()) {
        body["currency"] = serde_json::Value::String(v);
    }
    if let Some(v) = notes.filter(|s| !s.is_empty()) {
        body["notes"] = serde_json::Value::String(v);
    }
    if let Some(v) = external_ref.filter(|s| !s.is_empty()) {
        body["external_ref"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/purchase-orders"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de creación de OC inválida del servidor: {e}"))
}

/// POST `/api/v1/purchase-orders/{id}/receive` (Bearer, admin+) — goods receipt.
/// The server bumps stock, recomputes weighted-average cost, appends a
/// `stock_movement`, advances each line's `qty_received`, and flips the PO to
/// `received` (or `partially_received`). Body: `{ lines: [{ po_line_id,
/// qty_received }], notes? }`. Receiving is only legal from `sent` / `approved`
/// / `partially_received` — a `draft` PO comes back 409 (server message
/// surfaced as-is). Returns the updated PO header.
#[tauri::command]
async fn receive_purchase_order(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    lines: Vec<ReceiveLine>,
    notes: Option<String>,
) -> Result<PurchaseOrder, String> {
    if lines.is_empty() {
        return Err("La recepción requiere al menos una línea.".to_string());
    }
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut body = serde_json::json!({ "lines": lines });
    if let Some(v) = notes.filter(|s| !s.is_empty()) {
        body["notes"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/purchase-orders/{id}/receive"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de recepción inválida del servidor: {e}"))
}

// --- receipt / boleta ------------------------------------------------------

/// GET `/api/v1/orders/{id}/receipt` (Bearer) — printable boleta for a completed
/// sale (tenant, folio, items, totals, vuelto, loyalty). Called right after a
/// successful `pos_sale` to show/print the ticket. A failure here never blocks
/// the sale itself (the order is already committed server-side).
#[tauri::command]
async fn get_receipt(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<Receipt, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/orders/{id}/receipt"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de boleta inválida del servidor: {e}"))
}

// --- DTE / boleta electrónica SII commands ----------------------------------
//
// Wire to `crates/api/src/v1/dte.rs`. Emission is cashier+; send/poll/cancel
// are admin+ server-side (403 surfaces as Spanish copy). `send_dte` and
// `emit_boleta` reject with the coded `"CODE|message"` shape so the view can
// branch on `FEATURE_REQUIRES_UPGRADE` (Free tier = local-only, ADR-0005).

/// GET `/api/v1/dte` (Bearer). Optional `estado` / `tipo` / `from` / `to`
/// (YYYY-MM-DD) / `limit` filters. Newest first server-side.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn list_dtes(
    state: State<'_, SessionState>,
    server_url: String,
    estado: Option<String>,
    tipo: Option<i32>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Dte>, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http.get(format!("{base}/api/v1/dte")).bearer_auth(token);
    if let Some(v) = estado.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("estado", v)]);
    }
    if let Some(t) = tipo {
        req = req.query(&[("tipo", t)]);
    }
    if let Some(v) = from.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("from", v)]);
    }
    if let Some(v) = to.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("to", v)]);
    }
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de boletas inválida del servidor: {e}"))
}

/// GET `/api/v1/dte/caf-status?tipo=N` (Bearer) — folios restantes por CAF
/// activo. Returns the raw `{ tipo, folios_restantes, cafs }` JSON.
#[tauri::command]
async fn dte_caf_status(
    state: State<'_, SessionState>,
    server_url: String,
    tipo: Option<i32>,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/dte/caf-status"))
        .bearer_auth(token);
    if let Some(t) = tipo {
        req = req.query(&[("tipo", t)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de folios inválida del servidor: {e}"))
}

/// GET `/api/v1/dte/{id}/xml` (Bearer) — signed XML as raw text so the webview
/// can wrap it in a Blob download (Free-tier export path, ADR-0005 no lock-in).
#[tauri::command]
async fn dte_xml(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<String, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/dte/{id}/xml"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.text()
        .await
        .map_err(|e| format!("Respuesta de XML inválida del servidor: {e}"))
}

/// GET `/api/v1/dte/libro-ventas?period=YYYY-MM` (Bearer, admin+) — monthly
/// sales book XML (unsigned; accountant review / manual upload).
#[tauri::command]
async fn dte_libro_ventas(
    state: State<'_, SessionState>,
    server_url: String,
    period: String,
) -> Result<String, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/dte/libro-ventas"))
        .query(&[("period", period)])
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.text()
        .await
        .map_err(|e| format!("Respuesta de libro de ventas inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/libro-ventas/signed` (Bearer, admin+) — monthly sales
/// book signed with the company cert (EnvioLibro XML-DSig), ready for the SII
/// portal. Passphrase travels in the body, never in the query string.
#[tauri::command]
async fn dte_libro_ventas_signed(
    state: State<'_, SessionState>,
    server_url: String,
    period: String,
    cert_passphrase: String,
) -> Result<String, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/dte/libro-ventas/signed"))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "period": period,
            "cert_passphrase": cert_passphrase,
        }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.text()
        .await
        .map_err(|e| format!("Respuesta de libro firmado inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/boletas` (Bearer, cashier+) — emit + sign the boleta of a
/// paid POS order. `cert_passphrase` decrypts the stored cert (never persisted).
/// Rejects coded (`"CODE|message"`) so the view can show config errors nicely.
#[tauri::command]
async fn emit_boleta(
    state: State<'_, SessionState>,
    server_url: String,
    order_id: String,
    cert_passphrase: String,
    receptor_rut: Option<String>,
    razon_social_receptor: Option<String>,
) -> Result<Dte, String> {
    let token = token_of(&state).map_err(|e| format!("|{e}"))?;
    let http = client().map_err(|e| format!("|{e}"))?;
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "order_id": order_id,
        "cert_passphrase": cert_passphrase,
    });
    if let Some(v) = receptor_rut.filter(|s| !s.is_empty()) {
        body["receptor_rut"] = serde_json::Value::String(v);
    }
    if let Some(v) = razon_social_receptor.filter(|s| !s.is_empty()) {
        body["razon_social_receptor"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/dte/boletas"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("|{}", conn_error(e)))?;
    if !resp.status().is_success() {
        return Err(coded_error(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("|Respuesta de emisión inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/documentos` (Bearer, admin+) — emit + sign factura (33),
/// nota de débito (56), nota de crédito (61) or guía de despacho (52).
/// Server computes totals from the items (IVA-included prices). Rejects coded
/// (`"CODE|message"`) like `emit_boleta` so the view can render config errors.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn emit_documento(
    state: State<'_, SessionState>,
    server_url: String,
    tipo: i32,
    cert_passphrase: String,
    receptor: serde_json::Value,
    items: Vec<serde_json::Value>,
    referencias: Option<Vec<serde_json::Value>>,
    ind_traslado: Option<i32>,
    order_id: Option<String>,
) -> Result<Dte, String> {
    let token = token_of(&state).map_err(|e| format!("|{e}"))?;
    let http = client().map_err(|e| format!("|{e}"))?;
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "tipo": tipo,
        "cert_passphrase": cert_passphrase,
        "receptor": receptor,
        "items": items,
    });
    if let Some(refs) = referencias.filter(|r| !r.is_empty()) {
        body["referencias"] = serde_json::Value::Array(refs);
    }
    if let Some(it) = ind_traslado {
        body["ind_traslado"] = serde_json::Value::from(it);
    }
    if let Some(ord) = order_id.filter(|s| !s.is_empty()) {
        body["order_id"] = serde_json::Value::String(ord);
    }
    let resp = http
        .post(format!("{base}/api/v1/dte/documentos"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("|{}", conn_error(e)))?;
    if !resp.status().is_success() {
        return Err(coded_error(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("|Respuesta de emisión inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/{id}/send` (Bearer, admin+) — upload to SII. **Tier-gated**:
/// Free gets 402 `FEATURE_REQUIRES_UPGRADE` (coded so the view shows an upgrade
/// note instead of a hard error).
#[tauri::command]
async fn send_dte(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<Dte, String> {
    let token = token_of(&state).map_err(|e| format!("|{e}"))?;
    let http = client().map_err(|e| format!("|{e}"))?;
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/dte/{id}/send"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("|{}", conn_error(e)))?;
    if !resp.status().is_success() {
        return Err(coded_error(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("|Respuesta de envío inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/{id}/poll` (Bearer, admin+) — refresh the SII verdict.
/// Returns the raw JSON (`DteDto` flattened + `sii_estado`).
#[tauri::command]
async fn poll_dte(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/dte/{id}/poll"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de consulta SII inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/{id}/cancel` (Bearer, admin+) — anular pre-envío
/// (`draft|signed → cancelled`). Body `{ reason }` (required).
#[tauri::command]
async fn cancel_dte(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    reason: String,
) -> Result<Dte, String> {
    let token = token_of(&state)?;
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/dte/{id}/cancel"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "reason": reason }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de anulación inválida del servidor: {e}"))
}

/// GET `/health/ready`. Public (no token). 200 → healthy; 503 → degraded but
/// still "reachable" (server is up). Connection errors → `Err`.
#[tauri::command]
async fn server_health(server_url: String) -> Result<HealthInfo, String> {
    let http = client()?;
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/health/ready"))
        .send()
        .await
        .map_err(conn_error)?;

    let reachable = true; // we got an HTTP response at all
    let ok = resp.status().is_success();

    // /health/ready returns the same JSON body for 200 and 503.
    #[derive(Deserialize)]
    struct Ready {
        status: String,
        checks: Checks,
    }
    #[derive(Deserialize)]
    struct Checks {
        db: String,
    }

    match resp.json::<Ready>().await {
        Ok(r) => Ok(HealthInfo {
            status: r.status,
            db: r.checks.db,
            reachable,
        }),
        Err(_) => Ok(HealthInfo {
            status: if ok { "ok".into() } else { "degraded".into() },
            db: "desconocido".into(),
            reachable,
        }),
    }
}

/// Forget the in-memory session (logout / return to LoginView).
#[tauri::command]
fn logout(state: State<'_, SessionState>) -> Result<(), String> {
    *state
        .inner
        .lock()
        .map_err(|_| "Estado de sesión corrupto.".to_string())? = None;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(SessionState::default())
        .setup(|app| {
            // Touch the state so `Manager` import is used even if commands are
            // tree-shaken in a future refactor; also a cheap sanity init.
            let _ = app.state::<SessionState>();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            login,
            setup_status,
            setup_account,
            license_status,
            server_health,
            logout,
            list_products,
            inventory_summary,
            sales_daily,
            top_products,
            pos_sale,
            create_refund,
            list_refunds,
            query_audit_log,
            cash_sessions,
            open_cash_session,
            cash_arqueo,
            close_cash_session,
            get_setting,
            set_setting,
            customer_search,
            customer_detail,
            customer_history,
            create_customer,
            update_customer,
            list_purchase_orders,
            get_purchase_order,
            list_suppliers,
            create_supplier,
            create_purchase_order,
            receive_purchase_order,
            get_po_payments,
            create_po_payment,
            list_expenses,
            create_expense,
            margins_daily,
            stock_rotation,
            dashboard_report,
            list_prescriptions,
            get_prescription,
            create_prescription,
            libro_recetas,
            export_libro_recetas,
            get_receipt,
            create_product,
            import_products,
            import_products_preview,
            export_products,
            product_detail,
            adjust_product_stock,
            list_batches,
            create_batch,
            near_expiry,
            list_dtes,
            dte_caf_status,
            dte_xml,
            dte_libro_ventas,
            dte_libro_ventas_signed,
            emit_documento,
            emit_boleta,
            send_dte,
            poll_dte,
            cancel_dte,
            seed_demo
        ])
        .run(tauri::generate_context!())
        .expect("error while running pharma-client");
}

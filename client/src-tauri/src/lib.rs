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

// --- catalog detail / batches / near-expiry (Inventario lane) --------------

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

// --- POS sale --------------------------------------------------------------

/// POST `/api/v1/pos/sale` (Bearer + a FRESH `Idempotency-Key` minted here so
/// every "Cobrar" click is a distinct sale). `cash_amount` / `card_amount` are
/// optional STRINGS forwarded verbatim. On a non-2xx, the error is returned as
/// `"CODE|message"` (see [`coded_error`]) so the UI can special-case
/// `INSUFFICIENT_STOCK`. Returns the raw server JSON (order + items + alerts).
#[tauri::command]
async fn pos_sale(
    state: State<'_, SessionState>,
    server_url: String,
    items: Vec<PosItem>,
    payment_method: String,
    cash_amount: Option<String>,
    card_amount: Option<String>,
    customer: Option<String>,
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
            license_status,
            server_health,
            logout,
            list_products,
            inventory_summary,
            sales_daily,
            top_products,
            pos_sale,
            cash_sessions,
            open_cash_session,
            cash_arqueo,
            close_cash_session,
            customer_search,
            customer_detail,
            customer_history,
            list_purchase_orders,
            list_expenses,
            create_expense,
            get_receipt,
            create_product,
            product_detail,
            adjust_product_stock,
            list_batches,
            create_batch,
            near_expiry
        ])
        .run(tauri::generate_context!())
        .expect("error while running pharma-client");
}

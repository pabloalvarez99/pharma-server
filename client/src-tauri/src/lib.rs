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
        "No se pudo conectar al servidor. Verifica la URL y que pharma-server esté corriendo.".to_string()
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
    *state.inner.lock().map_err(|_| "Estado de sesión corrupto.".to_string())? =
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
            pos_sale
        ])
        .run(tauri::generate_context!())
        .expect("error while running pharma-client");
}

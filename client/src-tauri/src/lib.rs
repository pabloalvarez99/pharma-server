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

/// Server error envelope (`crates/api/src/error.rs`):
/// `{ "error": { "code", "message", "details"? } }`.
#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[allow(dead_code)]
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
    let token = {
        let guard = state
            .inner
            .lock()
            .map_err(|_| "Estado de sesión corrupto.".to_string())?;
        guard
            .as_ref()
            .map(|s| s.token.clone())
            .ok_or_else(|| "No hay sesión activa. Inicia sesión primero.".to_string())?
    };

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
            logout
        ])
        .run(tauri::generate_context!())
        .expect("error while running pharma-client");
}

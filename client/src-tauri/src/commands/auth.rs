//! Auth commands: login, first-run setup, logout.
//!
//! Passwords arrive from the webview as plain IPC strings and are immediately
//! wrapped in [`SecretString`] (serde-transparent — the IPC contract is
//! unchanged); they are zeroed on drop and never debug-printed.

use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{self, SessionState};
use crate::types::{HealthInfo, SessionInfo, SetupInfo, SetupStatusInfo};

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

/// Server `SetupResponse`: a login token plus the assigned tenant slug.
#[derive(Deserialize)]
struct SetupResponse {
    token: String,
    #[allow(dead_code)]
    token_type: String,
    expires_in: u64,
    tenant_slug: String,
}

/// POST `/api/v1/login` with `{ tenant, email, password }`, then GET
/// `/api/v1/me` to resolve tenant_id + roles. Stores the JWT in `SessionState`.
#[tauri::command]
pub async fn login(
    state: State<'_, SessionState>,
    server_url: String,
    tenant: String,
    email: String,
    password: SecretString,
) -> Result<SessionInfo, String> {
    let http = client();
    let base = base(&server_url);

    let resp = http
        .post(format!("{base}/api/v1/login"))
        .json(&serde_json::json!({
            "tenant": tenant,
            "email": email,
            "password": password.expose_secret(),
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
    state::store_token(&state, login.token)?;

    Ok(SessionInfo {
        user_id: me.sub,
        tenant_id: me.tenant_id,
        roles: me.roles,
        expires_in: login.expires_in,
    })
}

/// GET `/api/v1/setup/status` — UNAUTHENTICATED. `needs_setup = true` when the
/// install has no account yet, so the login screen can offer in-app account
/// creation instead of a dead end.
#[tauri::command]
pub async fn setup_status(server_url: String) -> Result<SetupStatusInfo, String> {
    let http = client();
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
pub async fn setup_account(
    state: State<'_, SessionState>,
    server_url: String,
    business_name: String,
    tenant_slug: Option<String>,
    email: String,
    password: SecretString,
    vertical: Option<String>,
) -> Result<SetupInfo, String> {
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/setup"))
        .json(&serde_json::json!({
            "business_name": business_name,
            "tenant_slug": tenant_slug,
            "email": email,
            "password": password.expose_secret(),
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

    state::store_token(&state, setup.token)?;

    Ok(SetupInfo {
        user_id: me.sub,
        tenant_id: me.tenant_id,
        roles: me.roles,
        expires_in: setup.expires_in,
        tenant_slug: setup.tenant_slug,
    })
}

/// Forget the in-memory session (logout / return to LoginView). The server JWT
/// is stateless (HS256, TTL 3600s — `config/default.toml`); server-side
/// revocation is pending in the server bitácora, so the token technically
/// stays valid until it expires. Locally the `SecretString` is zeroed on drop.
#[tauri::command]
pub fn logout(state: State<'_, SessionState>) -> Result<(), String> {
    state::clear(&state)
}

/// GET `/health/ready`. Public (no token). 200 → healthy; 503 → degraded but
/// still "reachable" (server is up). Connection errors → `Err`. Uses the
/// short-timeout health client so a dead server surfaces fast on the login
/// screen.
#[tauri::command]
pub async fn server_health(server_url: String) -> Result<HealthInfo, String> {
    let http = crate::http::health_client();
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

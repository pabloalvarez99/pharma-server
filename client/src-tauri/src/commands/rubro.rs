//! Rubro pack command — fetches the tenant's declarative vertical pack
//! (`GET /api/v1/rubro-pack`, `domain::rubro::RubroPack`).
//!
//! Returns the raw JSON: the pack shape is meant to grow server-side (new
//! rubros, new vocab keys) without breaking older clients — the webview reads
//! the fields it knows and ignores the rest (same philosophy as
//! `dashboard_report`).

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};

/// GET `/api/v1/rubro-pack` (Bearer, any role). The pack for the tenant's
/// stored `business.vertical` (generic `otro` when unset). Open to every
/// authenticated user — every view gates its modules from it.
#[tauri::command]
pub async fn rubro_pack(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/rubro-pack"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de pack de rubro inválida del servidor: {e}"))
}

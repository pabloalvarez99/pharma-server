//! Admin settings commands.

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::AdminSetting;

/// GET `/api/v1/settings/{key}` (Bearer). The setting is optional: an unset key
/// returns 404 server-side, which we map to `Ok(None)` so the Configuración view
/// renders the default/empty state instead of a hard error.
#[tauri::command]
pub async fn get_setting(
    state: State<'_, SessionState>,
    server_url: String,
    key: String,
) -> Result<Option<AdminSetting>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/settings/{key}"))
        .bearer_auth(token.expose_secret())
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
pub async fn set_setting(
    state: State<'_, SessionState>,
    server_url: String,
    key: String,
    value: String,
) -> Result<AdminSetting, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .put(format!("{base}/api/v1/settings/{key}"))
        .bearer_auth(token.expose_secret())
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

//! License status command.

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::LicenseSummary;

/// GET `/api/v1/admin/license/status` (Bearer). Requires a prior `login`.
/// Admin/owner only on the server side — a 403 surfaces as Spanish copy.
#[tauri::command]
pub async fn license_status(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<LicenseSummary, String> {
    let token = token_of(&state)?;

    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/admin/license/status"))
        .bearer_auth(token.expose_secret())
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

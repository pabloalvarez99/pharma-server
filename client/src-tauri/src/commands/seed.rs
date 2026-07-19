//! Demo seeding command (multi-rubro onboarding).

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};

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
pub async fn seed_demo(
    state: State<'_, SessionState>,
    server_url: String,
    vertical: String,
    force: bool,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/admin/seed-demo"))
        .bearer_auth(token.expose_secret())
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

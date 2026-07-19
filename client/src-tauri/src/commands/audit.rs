//! Auditoría / audit-log command.

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::AuditPage;

/// GET `/api/v1/admin/audit-log` (Bearer, admin/owner) — immutable audit trail,
/// tenant-scoped. Optional filters: `from`/`to` (YYYY-MM-DD), `user` (record id),
/// `table`, `action` (create|update|delete), `limit` (1..=500), `offset`. A 403
/// (non-admin) surfaces as the server's Spanish message.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn query_audit_log(
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
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/admin/audit-log"))
        .bearer_auth(token.expose_secret());
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

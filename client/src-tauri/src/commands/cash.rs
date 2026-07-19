//! Cash register (caja) commands.

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::{CashCloseSummary, CashSession};

/// GET `/api/v1/cash-sessions?status=open&limit=1` (Bearer). Returns the list
/// (newest first server-side); the view picks `[0]` as the current open session
/// or shows "sin caja abierta" when empty. Caja is core/free on every tier.
#[tauri::command]
pub async fn cash_sessions(
    state: State<'_, SessionState>,
    server_url: String,
    status: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<CashSession>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/cash-sessions"))
        .bearer_auth(token.expose_secret());
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
pub async fn open_cash_session(
    state: State<'_, SessionState>,
    server_url: String,
    register_name: String,
    opening_cash: String,
    notes: Option<String>,
) -> Result<CashSession, String> {
    let token = token_of(&state)?;
    let http = client();
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
        .bearer_auth(token.expose_secret())
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
pub async fn cash_arqueo(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<CashCloseSummary, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/cash-sessions/{id}/arqueo"))
        .bearer_auth(token.expose_secret())
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
pub async fn close_cash_session(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    closing_cash_counted: String,
    notes: Option<String>,
) -> Result<CashCloseSummary, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "closing_cash_counted": closing_cash_counted,
    });
    if let Some(n) = notes.filter(|s| !s.is_empty()) {
        body["notes"] = serde_json::Value::String(n);
    }
    let resp = http
        .post(format!("{base}/api/v1/cash-sessions/{id}/close"))
        .bearer_auth(token.expose_secret())
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

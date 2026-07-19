//! Agent assist commands ("Pregúntale a tu negocio", ADR-0016).

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::AssistAnswer;

/// POST `/api/v1/assist/ask` (Bearer, cashier+) — the read-only, offline-first
/// business agent. Forwards the owner's raw Spanish question; the server parses
/// a deterministic intent and answers from the tenant's OWN data (no LLM, no
/// network beyond this LAN call). Always resolves with an [`AssistAnswer`] on a
/// 2xx — note that "no entendí" is itself a 200 (`intent = "desconocido"`), not
/// an error. A non-2xx surfaces the server's Spanish message.
#[tauri::command]
pub async fn assist_ask(
    state: State<'_, SessionState>,
    server_url: String,
    question: String,
) -> Result<AssistAnswer, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/assist/ask"))
        .bearer_auth(token.expose_secret())
        .json(&serde_json::json!({ "question": question }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta del agente inválida del servidor: {e}"))
}

/// POST `/api/v1/assist/act` (Bearer) — EXECUTE the write the agent proposed,
/// authorised by the single-use `confirm_token` from a prior `assist_ask`
/// proposal. This is the ONLY agent call that mutates tenant data; the webview
/// reaches it solely from the owner's explicit "Confirmar" click. A role-denied
/// write (403) or an expired/spent token surfaces the server's Spanish message
/// via [`error_message`]. The action result rides back as raw JSON (`{ text,
/// .. }`) so the exact shape can grow server-side without breaking the client.
#[tauri::command]
pub async fn assist_act(
    state: State<'_, SessionState>,
    server_url: String,
    confirm_token: String,
) -> Result<serde_json::Value, String> {
    if confirm_token.trim().is_empty() {
        return Err("No hay una acción para confirmar.".to_string());
    }
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/assist/act"))
        .bearer_auth(token.expose_secret())
        .json(&serde_json::json!({ "confirm_token": confirm_token }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta del agente inválida del servidor: {e}"))
}

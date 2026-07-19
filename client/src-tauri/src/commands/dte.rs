//! DTE / boleta electrónica SII commands.
//!
//! Wire to `crates/api/src/v1/dte.rs`. Emission is cashier+; send/poll/cancel
//! are admin+ server-side (403 surfaces as Spanish copy). `send_dte` and
//! `emit_boleta` reject with the coded `"CODE|message"` shape so the view can
//! branch on `FEATURE_REQUIRES_UPGRADE` (Free tier = local-only, ADR-0005).
//!
//! Cert passphrases arrive as plain IPC strings and are wrapped in
//! [`SecretString`] (serde-transparent — IPC contract unchanged): zeroed on
//! drop, never debug-printed, forwarded only inside the request body.

use secrecy::{ExposeSecret, SecretString};
use tauri::State;

use crate::http::{base, client, coded_error, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::Dte;

/// GET `/api/v1/dte` (Bearer). Optional `estado` / `tipo` / `from` / `to`
/// (YYYY-MM-DD) / `limit` filters. Newest first server-side.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn list_dtes(
    state: State<'_, SessionState>,
    server_url: String,
    estado: Option<String>,
    tipo: Option<i32>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Dte>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http.get(format!("{base}/api/v1/dte")).bearer_auth(token.expose_secret());
    if let Some(v) = estado.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("estado", v)]);
    }
    if let Some(t) = tipo {
        req = req.query(&[("tipo", t)]);
    }
    if let Some(v) = from.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("from", v)]);
    }
    if let Some(v) = to.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("to", v)]);
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
        .map_err(|e| format!("Respuesta de boletas inválida del servidor: {e}"))
}

/// GET `/api/v1/dte/caf-status?tipo=N` (Bearer) — folios restantes por CAF
/// activo. Returns the raw `{ tipo, folios_restantes, cafs }` JSON.
#[tauri::command]
pub async fn dte_caf_status(
    state: State<'_, SessionState>,
    server_url: String,
    tipo: Option<i32>,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/dte/caf-status"))
        .bearer_auth(token.expose_secret());
    if let Some(t) = tipo {
        req = req.query(&[("tipo", t)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de folios inválida del servidor: {e}"))
}

/// GET `/api/v1/dte/{id}/xml` (Bearer) — signed XML as raw text so the webview
/// can wrap it in a Blob download (Free-tier export path, ADR-0005 no lock-in).
#[tauri::command]
pub async fn dte_xml(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<String, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/dte/{id}/xml"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.text()
        .await
        .map_err(|e| format!("Respuesta de XML inválida del servidor: {e}"))
}

/// GET `/api/v1/dte/libro-ventas?period=YYYY-MM` (Bearer, admin+) — monthly
/// sales book XML (unsigned; accountant review / manual upload).
#[tauri::command]
pub async fn dte_libro_ventas(
    state: State<'_, SessionState>,
    server_url: String,
    period: String,
) -> Result<String, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/dte/libro-ventas"))
        .query(&[("period", period)])
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.text()
        .await
        .map_err(|e| format!("Respuesta de libro de ventas inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/libro-ventas/signed` (Bearer, admin+) — monthly sales
/// book signed with the company cert (EnvioLibro XML-DSig), ready for the SII
/// portal. Passphrase travels in the body, never in the query string.
#[tauri::command]
pub async fn dte_libro_ventas_signed(
    state: State<'_, SessionState>,
    server_url: String,
    period: String,
    cert_passphrase: SecretString,
) -> Result<String, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/dte/libro-ventas/signed"))
        .bearer_auth(token.expose_secret())
        .json(&serde_json::json!({
            "period": period,
            "cert_passphrase": cert_passphrase.expose_secret(),
        }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.text()
        .await
        .map_err(|e| format!("Respuesta de libro firmado inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/boletas` (Bearer, cashier+) — emit + sign the boleta of a
/// paid POS order. `cert_passphrase` decrypts the stored cert (never persisted).
/// Rejects coded (`"CODE|message"`) so the view can show config errors nicely.
#[tauri::command]
pub async fn emit_boleta(
    state: State<'_, SessionState>,
    server_url: String,
    order_id: String,
    cert_passphrase: SecretString,
    receptor_rut: Option<String>,
    razon_social_receptor: Option<String>,
) -> Result<Dte, String> {
    let token = token_of(&state).map_err(|e| format!("|{e}"))?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "order_id": order_id,
        "cert_passphrase": cert_passphrase.expose_secret(),
    });
    if let Some(v) = receptor_rut.filter(|s| !s.is_empty()) {
        body["receptor_rut"] = serde_json::Value::String(v);
    }
    if let Some(v) = razon_social_receptor.filter(|s| !s.is_empty()) {
        body["razon_social_receptor"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/dte/boletas"))
        .bearer_auth(token.expose_secret())
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("|{}", conn_error(e)))?;
    if !resp.status().is_success() {
        return Err(coded_error(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("|Respuesta de emisión inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/documentos` (Bearer, admin+) — emit + sign factura (33),
/// nota de débito (56), nota de crédito (61) or guía de despacho (52).
/// Server computes totals from the items (IVA-included prices). Rejects coded
/// (`"CODE|message"`) like `emit_boleta` so the view can render config errors.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn emit_documento(
    state: State<'_, SessionState>,
    server_url: String,
    tipo: i32,
    cert_passphrase: SecretString,
    receptor: serde_json::Value,
    items: Vec<serde_json::Value>,
    referencias: Option<Vec<serde_json::Value>>,
    ind_traslado: Option<i32>,
    order_id: Option<String>,
) -> Result<Dte, String> {
    let token = token_of(&state).map_err(|e| format!("|{e}"))?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "tipo": tipo,
        "cert_passphrase": cert_passphrase.expose_secret(),
        "receptor": receptor,
        "items": items,
    });
    if let Some(refs) = referencias.filter(|r| !r.is_empty()) {
        body["referencias"] = serde_json::Value::Array(refs);
    }
    if let Some(it) = ind_traslado {
        body["ind_traslado"] = serde_json::Value::from(it);
    }
    if let Some(ord) = order_id.filter(|s| !s.is_empty()) {
        body["order_id"] = serde_json::Value::String(ord);
    }
    let resp = http
        .post(format!("{base}/api/v1/dte/documentos"))
        .bearer_auth(token.expose_secret())
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("|{}", conn_error(e)))?;
    if !resp.status().is_success() {
        return Err(coded_error(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("|Respuesta de emisión inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/{id}/send` (Bearer, admin+) — upload to SII. **Tier-gated**:
/// Free gets 402 `FEATURE_REQUIRES_UPGRADE` (coded so the view shows an upgrade
/// note instead of a hard error).
#[tauri::command]
pub async fn send_dte(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<Dte, String> {
    let token = token_of(&state).map_err(|e| format!("|{e}"))?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/dte/{id}/send"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(|e| format!("|{}", conn_error(e)))?;
    if !resp.status().is_success() {
        return Err(coded_error(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("|Respuesta de envío inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/{id}/poll` (Bearer, admin+) — refresh the SII verdict.
/// Returns the raw JSON (`DteDto` flattened + `sii_estado`).
#[tauri::command]
pub async fn poll_dte(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/dte/{id}/poll"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de consulta SII inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/{id}/cancel` (Bearer, admin+) — anular pre-envío
/// (`draft|signed → cancelled`). Body `{ reason }` (required).
#[tauri::command]
pub async fn cancel_dte(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    reason: String,
) -> Result<Dte, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/dte/{id}/cancel"))
        .bearer_auth(token.expose_secret())
        .json(&serde_json::json!({ "reason": reason }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de anulación inválida del servidor: {e}"))
}

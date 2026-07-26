//! Libro de compras + resumen IVA (F29) y captura de la factura del proveedor.
//! Todo admin+ server-side (son cifras tributarias del negocio).

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{token_of, SessionState};
use crate::types::{IvaSummary, PurchaseBook};

/// GET `/api/v1/reports/libro-compras?period=YYYY-MM` (Bearer, admin+).
/// Sin `period` el server usa el mes en curso.
#[tauri::command]
pub async fn libro_compras(
    state: State<'_, SessionState>,
    server_url: String,
    period: Option<String>,
) -> Result<PurchaseBook, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/reports/libro-compras"))
        .bearer_auth(token.expose_secret());
    if let Some(p) = period.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("period", p)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de libro de compras inválida del servidor: {e}"))
}

/// GET `/api/v1/reports/iva?period=YYYY-MM` (Bearer, admin+) — débito − crédito.
#[tauri::command]
pub async fn iva_summary(
    state: State<'_, SessionState>,
    server_url: String,
    period: Option<String>,
) -> Result<IvaSummary, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/reports/iva"))
        .bearer_auth(token.expose_secret());
    if let Some(p) = period.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("period", p)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de IVA inválida del servidor: {e}"))
}

/// PATCH `/api/v1/purchase-orders/{id}/factura` (Bearer, admin+) — capturar el
/// documento del proveedor. Sólo se envía lo que el operador tenga a mano;
/// devuelve el libro del período para refrescar sin segunda llamada.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn set_po_invoice(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    folio: Option<String>,
    date: Option<String>,
    tipo: Option<i32>,
    neto: Option<String>,
    iva: Option<String>,
    total: Option<String>,
) -> Result<PurchaseBook, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({});
    if let Some(v) = folio.filter(|s| !s.is_empty()) {
        body["folio"] = serde_json::Value::String(v);
    }
    if let Some(v) = date.filter(|s| !s.is_empty()) {
        body["date"] = serde_json::Value::String(v);
    }
    if let Some(v) = tipo {
        body["tipo"] = serde_json::Value::from(v);
    }
    for (k, v) in [("neto", neto), ("iva", iva), ("total", total)] {
        if let Some(s) = v.filter(|s| !s.is_empty()) {
            body[k] = serde_json::Value::String(s);
        }
    }
    let resp = http
        .patch(format!("{base}/api/v1/purchase-orders/{id}/factura"))
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
        .map_err(|e| format!("Respuesta de factura inválida del servidor: {e}"))
}

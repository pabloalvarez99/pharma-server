//! Sucursales (branch) + cajas (register) + stock por sucursal.
//!
//! Multi-sucursal OPERATIVO (V2): el operador elige en qué local está trabajando,
//! ve cuánto tiene en cada uno y mueve mercadería entre ellos. Todos son wrappers
//! HTTP finos sobre `crates/api` — el JWT vive en [`crate::state::SessionState`].

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, coded_error, conn_error, error_message};
use crate::state::{SessionState, token_of};

/// GET `/api/v1/sucursales` (Bearer). Devuelve el JSON crudo del servidor
/// (`[{ id, name, code, address, comuna, phone, active, … }]`) — el selector del
/// shell sólo necesita `id`/`name`/`active`.
#[tauri::command]
pub async fn sucursales(
    state: State<'_, SessionState>,
    server_url: String,
    active: Option<bool>,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/sucursales"))
        .bearer_auth(token.expose_secret());
    if let Some(a) = active {
        req = req.query(&[("active", a)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de sucursales inválida del servidor: {e}"))
}

/// GET `/api/v1/cajas` (Bearer). `branch` filtra las cajas de una sucursal —
/// es lo que necesita la apertura de caja para ofrecer sólo las de este local.
#[tauri::command]
pub async fn cajas(
    state: State<'_, SessionState>,
    server_url: String,
    branch: Option<String>,
    active: Option<bool>,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/cajas"))
        .bearer_auth(token.expose_secret());
    if let Some(b) = branch.filter(|s| !s.is_empty()) {
        req = req.query(&[("branch", b)]);
    }
    if let Some(a) = active {
        req = req.query(&[("active", a)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de cajas inválida del servidor: {e}"))
}

/// GET `/api/v1/stock/sucursales` (Bearer). On-hand por (producto, sucursal).
/// `branch = "none"` aísla la casa matriz.
#[tauri::command]
pub async fn stock_por_sucursal(
    state: State<'_, SessionState>,
    server_url: String,
    product: Option<String>,
    branch: Option<String>,
    non_zero: Option<bool>,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/stock/sucursales"))
        .bearer_auth(token.expose_secret());
    if let Some(p) = product.filter(|s| !s.is_empty()) {
        req = req.query(&[("product", p)]);
    }
    if let Some(b) = branch.filter(|s| !s.is_empty()) {
        req = req.query(&[("branch", b)]);
    }
    if let Some(nz) = non_zero {
        req = req.query(&[("non_zero", nz)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de stock por sucursal inválida del servidor: {e}"))
}

/// GET `/api/v1/stock/sucursales/reporte` (Bearer). Una fila por producto con el
/// desglose por local + total.
#[tauri::command]
pub async fn stock_por_sucursal_reporte(
    state: State<'_, SessionState>,
    server_url: String,
    branch: Option<String>,
    non_zero: Option<bool>,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/stock/sucursales/reporte"))
        .bearer_auth(token.expose_secret());
    if let Some(b) = branch.filter(|s| !s.is_empty()) {
        req = req.query(&[("branch", b)]);
    }
    if let Some(nz) = non_zero {
        req = req.query(&[("non_zero", nz)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta del reporte por sucursal inválida del servidor: {e}"))
}

/// POST `/api/v1/stock/transferencias` (Bearer, admin+). Mueve `qty` de un
/// producto entre dos locales. El error vuelve como `"CODE|mensaje"` para que la
/// UI pueda distinguir `INSUFFICIENT_STOCK` (el origen no tiene tanto) del resto.
#[tauri::command]
pub async fn transferir_stock(
    state: State<'_, SessionState>,
    server_url: String,
    product: String,
    from_branch: Option<String>,
    to_branch: Option<String>,
    qty: i64,
    notes: Option<String>,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state).map_err(|e| format!("|{e}"))?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({ "product": product, "qty": qty });
    if let Some(f) = from_branch.filter(|s| !s.is_empty() && s != "none") {
        body["from_branch"] = serde_json::Value::String(f);
    }
    if let Some(t) = to_branch.filter(|s| !s.is_empty() && s != "none") {
        body["to_branch"] = serde_json::Value::String(t);
    }
    if let Some(n) = notes.filter(|s| !s.is_empty()) {
        body["notes"] = serde_json::Value::String(n);
    }
    let resp = http
        .post(format!("{base}/api/v1/stock/transferencias"))
        .bearer_auth(token.expose_secret())
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(coded_error(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("|Respuesta de transferencia inválida del servidor: {e}"))
}

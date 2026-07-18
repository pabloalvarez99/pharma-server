//! Reports commands: sales, top products, margins, rotation, dashboard.

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, coded_error, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::{DailyMarginRow, DailySalesRow, StockRotationRow, TopProductRow};

/// GET `/api/v1/reports/sales-daily` (Bearer). One row per UTC day with orders
/// + revenue split by tender. Free tier (sales-daily is core, not gated).
#[tauri::command]
pub async fn sales_daily(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<Vec<DailySalesRow>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/reports/sales-daily"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de ventas inválida del servidor: {e}"))
}

/// GET `/api/v1/reports/top-products?limit=N` (Bearer). Pareto ABC ranking by
/// revenue over the default window.
#[tauri::command]
pub async fn top_products(
    state: State<'_, SessionState>,
    server_url: String,
    limit: Option<u32>,
) -> Result<Vec<TopProductRow>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/reports/top-products"))
        .bearer_auth(token.expose_secret());
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de ranking inválida del servidor: {e}"))
}

/// GET `/api/v1/reports/margins-daily` (Bearer). **Pro-gated**: Free tier gets a
/// 402 `FEATURE_REQUIRES_UPGRADE`. Rejects with the `"CODE|message"` shape (see
/// [`coded_error`]) so the Reportes view can branch on `FEATURE_REQUIRES_UPGRADE`
/// and render an upgrade note instead of a hard error. `from`/`to` are optional
/// RFC3339 date-range filters forwarded as query params.
#[tauri::command]
pub async fn margins_daily(
    state: State<'_, SessionState>,
    server_url: String,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<DailyMarginRow>, String> {
    let token = token_of(&state).map_err(|e| format!("|{e}"))?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/reports/margins-daily"))
        .bearer_auth(token.expose_secret());
    if let Some(f) = from.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("from", f)]);
    }
    if let Some(t) = to.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("to", t)]);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("|{}", conn_error(e)))?;
    // Non-2xx (incl. 402 FEATURE_REQUIRES_UPGRADE) → coded "CODE|message".
    if !resp.status().is_success() {
        return Err(coded_error(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("|Respuesta de márgenes inválida del servidor: {e}"))
}

/// GET `/api/v1/reports/stock-rotation` (Bearer). Inventory turnover per product
/// over the window (`qty_sold / current_stock`). `from`/`to` optional RFC3339.
#[tauri::command]
pub async fn stock_rotation(
    state: State<'_, SessionState>,
    server_url: String,
    from: Option<String>,
    to: Option<String>,
) -> Result<Vec<StockRotationRow>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/reports/stock-rotation"))
        .bearer_auth(token.expose_secret());
    if let Some(f) = from.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("from", f)]);
    }
    if let Some(t) = to.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("to", t)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de rotación inválida del servidor: {e}"))
}

/// GET `/api/v1/reports/dashboard` (Bearer, admin/owner/quimico). One aggregate
/// exec summary (ventas hoy/mes, inventario, top productos, por_vencer, margen
/// hoy). `margen_hoy` is `null` on the Free tier (the server degrades that one
/// field instead of 402-ing the whole overview). Returns the raw JSON object —
/// the webview reads the typed `DashboardSummary` shape from it.
#[tauri::command]
pub async fn dashboard_report(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<serde_json::Value, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/reports/dashboard"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta del panel inválida del servidor: {e}"))
}

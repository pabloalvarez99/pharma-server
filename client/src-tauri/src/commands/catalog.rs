//! Catalog / inventory commands: products, batches, near-expiry, CSV import/export.
//!
//! Writes (`create_product`, `adjust_product_stock`, `create_batch`) require
//! admin/owner server-side; a 403 surfaces as the Spanish permission copy via
//! `error_message`. Empty optional fields are omitted so the server applies its
//! own defaults (e.g. `prescription_type`, `stock = 0`).

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::{
    Batch, ImportSummary, InventorySummary, NearExpiryRow, Product, ProductDetail,
};

/// GET `/api/v1/products` (Bearer). `search` filters by name/ingredient on the
/// server; `limit` caps rows. Returns the trimmed [`Product`] projection.
#[tauri::command]
pub async fn list_products(
    state: State<'_, SessionState>,
    server_url: String,
    search: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Product>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);

    let mut req = http
        .get(format!("{base}/api/v1/products"))
        .bearer_auth(token.expose_secret());
    if let Some(s) = search.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("search", s)]);
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
        .map_err(|e| format!("Respuesta de productos inválida del servidor: {e}"))
}

/// GET `/api/v1/products/stats` (Bearer) → inventory KPIs (count, low/out of
/// stock, total valuation). Reused by both Inventario and Reportes.
#[tauri::command]
pub async fn inventory_summary(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<InventorySummary, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/products/stats"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de inventario inválida del servidor: {e}"))
}

/// POST `/api/v1/products` (Bearer, admin+). Money strings (`price`,
/// `cost_price`) forwarded verbatim.
///
/// Optional `attrs` pack bag (talla/color/sku/…). Wire key **`attrs`**
/// (`NewProduct.attrs` + migr. 0035 FLEXIBLE). Forwarded as JSON object.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_product(
    state: State<'_, SessionState>,
    server_url: String,
    name: String,
    price: String,
    cost_price: Option<String>,
    stock: Option<i64>,
    category: Option<String>,
    laboratory: Option<String>,
    active_ingredient: Option<String>,
    prescription_type: Option<String>,
    presentation: Option<String>,
    attrs: Option<serde_json::Value>,
) -> Result<ProductDetail, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({ "name": name, "price": price });
    if let Some(v) = cost_price.filter(|s| !s.is_empty()) {
        body["cost_price"] = serde_json::Value::String(v);
    }
    // El lote nace en la sucursal activa del shell; `"none"` = casa matriz y el
    // server ya lo interpreta así, pero no lo mandamos para no ensuciar el body.
    if let Some(b) = branch.filter(|s| !s.is_empty() && s != "none") {
        body["branch"] = serde_json::Value::String(b);
    }
    if let Some(n) = stock {
        body["stock"] = serde_json::Value::from(n);
    }
    for (k, v) in [
        ("category", category),
        ("laboratory", laboratory),
        ("active_ingredient", active_ingredient),
        ("prescription_type", prescription_type),
        ("presentation", presentation),
    ] {
        if let Some(s) = v.filter(|s| !s.is_empty()) {
            body[k] = serde_json::Value::String(s);
        }
    }
    // Pack attrs bag — only attach a non-empty object so we never send `null`.
    if let Some(a) = attrs {
        if a.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
            body["attrs"] = a;
        }
    }
    let resp = http
        .post(format!("{base}/api/v1/products"))
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
        .map_err(|e| format!("Respuesta de producto inválida del servidor: {e}"))
}

/// Build the multipart form wrapping a CSV payload (shared by the real import
/// and the dry-run preview). The server reads the first field, name-agnostic.
fn csv_form(csv: String) -> Result<reqwest::multipart::Form, String> {
    let part = reqwest::multipart::Part::text(csv)
        .file_name("import.csv")
        .mime_str("text/csv")
        .map_err(|e| format!("No se pudo preparar el archivo CSV: {e}"))?;
    Ok(reqwest::multipart::Form::new().part("file", part))
}

/// POST `/api/v1/products/import` (Bearer, admin+) — bulk CSV catalog load. The
/// view reads the file text in JS and hands it over as `csv`; we wrap it in a
/// multipart field. Idempotent upsert by `external_id` server-side. Returns
/// per-row created/updated/failed.
#[tauri::command]
pub async fn import_products(
    state: State<'_, SessionState>,
    server_url: String,
    csv: String,
) -> Result<ImportSummary, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let form = csv_form(csv)?;
    let resp = http
        .post(format!("{base}/api/v1/products/import"))
        .bearer_auth(token.expose_secret())
        .multipart(form)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de importación inválida del servidor: {e}"))
}

/// POST `/api/v1/products/import?dry_run=true` (Bearer, admin+) — validates and
/// counts the CSV WITHOUT writing anything. Backs the "preview antes de
/// confirmar" step: the operator sees how many rows are OK / rejected (and why)
/// before committing the catalog migration.
#[tauri::command]
pub async fn import_products_preview(
    state: State<'_, SessionState>,
    server_url: String,
    csv: String,
) -> Result<ImportSummary, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let form = csv_form(csv)?;
    let resp = http
        .post(format!("{base}/api/v1/products/import?dry_run=true"))
        .bearer_auth(token.expose_secret())
        .multipart(form)
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de previsualización inválida del servidor: {e}"))
}

/// GET `/api/v1/products/export` (Bearer) — full catalog as CSV text so the
/// webview can wrap it in a Blob download. Columns match the `import_products`
/// format (export → edit → re-import round-trip). No-lock-in pillar (ADR-0005 #4).
#[tauri::command]
pub async fn export_products(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<String, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/products/export"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.text()
        .await
        .map_err(|e| format!("Respuesta de exportación inválida del servidor: {e}"))
}

/// GET `/api/v1/products/{id}` (Bearer) — full product detail for the drawer.
#[tauri::command]
pub async fn product_detail(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<ProductDetail, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/products/{id}"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de producto inválida del servidor: {e}"))
}

/// GET `/api/v1/products/by-barcode/{code}` — resolve EAN/barcode to the sellable
/// product (variant child or plain SKU). Used by POS scan path.
#[tauri::command]
pub async fn product_by_barcode(
    state: State<'_, SessionState>,
    server_url: String,
    code: String,
) -> Result<ProductDetail, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let code = code.trim();
    if code.is_empty() {
        return Err("Ingresa un código de barras.".into());
    }
    // Path-encode so spaces / special chars never break the route (no extra crate).
    let enc = encode_path_segment(code);
    let resp = http
        .get(format!("{base}/api/v1/products/by-barcode/{enc}"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de barcode inválida del servidor: {e}"))
}

/// GET `/api/v1/products/{id}/variants` — multi-SKU children of a parent.
#[tauri::command]
pub async fn list_product_variants(
    state: State<'_, SessionState>,
    server_url: String,
    product_id: String,
) -> Result<Vec<ProductDetail>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/products/{product_id}/variants"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de variantes inválida del servidor: {e}"))
}

/// POST `/api/v1/products/{id}/variants` (admin+) — create a sellable child SKU.
/// Money strings forwarded verbatim; optional `attrs` bag (talla/color/sku).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_product_variant(
    state: State<'_, SessionState>,
    server_url: String,
    parent_id: String,
    name: Option<String>,
    price: Option<String>,
    cost_price: Option<String>,
    stock: Option<i64>,
    barcode: Option<String>,
    attrs: Option<serde_json::Value>,
) -> Result<ProductDetail, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({});
    if let Some(v) = name.filter(|s| !s.is_empty()) {
        body["name"] = serde_json::Value::String(v);
    }
    if let Some(v) = price.filter(|s| !s.is_empty()) {
        body["price"] = serde_json::Value::String(v);
    }
    if let Some(v) = cost_price.filter(|s| !s.is_empty()) {
        body["cost_price"] = serde_json::Value::String(v);
    }
    // El lote nace en la sucursal activa del shell; `"none"` = casa matriz y el
    // server ya lo interpreta así, pero no lo mandamos para no ensuciar el body.
    if let Some(b) = branch.filter(|s| !s.is_empty() && s != "none") {
        body["branch"] = serde_json::Value::String(b);
    }
    if let Some(n) = stock {
        body["stock"] = serde_json::Value::from(n);
    }
    if let Some(v) = barcode.filter(|s| !s.is_empty()) {
        body["barcode"] = serde_json::Value::String(v);
    }
    if let Some(a) = attrs {
        if a.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
            body["attrs"] = a;
        }
    }
    let resp = http
        .post(format!("{base}/api/v1/products/{parent_id}/variants"))
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
        .map_err(|e| format!("Respuesta de variante inválida del servidor: {e}"))
}

/// POST `/api/v1/products/{id}/stock` (Bearer, admin+). Body `StockAdjust`:
/// either `set` (absolute) or `delta` (signed) + optional `reason`. Returns the
/// updated product.
#[tauri::command]
pub async fn adjust_product_stock(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    set: Option<i64>,
    delta: Option<i64>,
    reason: Option<String>,
) -> Result<ProductDetail, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({});
    if let Some(n) = set {
        body["set"] = serde_json::Value::from(n);
    }
    if let Some(n) = delta {
        body["delta"] = serde_json::Value::from(n);
    }
    if let Some(r) = reason.filter(|s| !s.is_empty()) {
        body["reason"] = serde_json::Value::String(r);
    }
    let resp = http
        .post(format!("{base}/api/v1/products/{id}/stock"))
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
        .map_err(|e| format!("Respuesta de ajuste inválida del servidor: {e}"))
}

/// GET `/api/v1/batches` (Bearer). Filters: `product` (record id), `branch`
/// (sucursal del lote; `"none"` = casa matriz, ausente = todos los locales),
/// `expiring_within_days`, `only_available`, `limit`. Returns lotes.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn list_batches(
    state: State<'_, SessionState>,
    server_url: String,
    product: Option<String>,
    branch: Option<String>,
    expiring_within_days: Option<i64>,
    only_available: Option<bool>,
    limit: Option<u32>,
) -> Result<Vec<Batch>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/batches"))
        .bearer_auth(token.expose_secret());
    if let Some(p) = product.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("product", p)]);
    }
    if let Some(b) = branch.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("branch", b)]);
    }
    if let Some(d) = expiring_within_days {
        req = req.query(&[("expiring_within_days", d)]);
    }
    if let Some(a) = only_available {
        req = req.query(&[("only_available", a)]);
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
        .map_err(|e| format!("Respuesta de lotes inválida del servidor: {e}"))
}

/// POST `/api/v1/batches` (Bearer, admin+). Body `NewBatch`: `product`,
/// `batch_code`, `expiry_date` (RFC3339), optional `stock`/`cost`/`notes`. An
/// initial `stock` > 0 emits a `batch_received` stock movement server-side.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_batch(
    state: State<'_, SessionState>,
    server_url: String,
    product: String,
    branch: Option<String>,
    batch_code: String,
    expiry_date: String,
    stock: Option<i64>,
    cost: Option<String>,
    notes: Option<String>,
) -> Result<Batch, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "product": product,
        "batch_code": batch_code,
        "expiry_date": expiry_date,
    });
    // El lote nace en la sucursal activa del shell; `"none"` = casa matriz y el
    // server ya lo interpreta así, pero no lo mandamos para no ensuciar el body.
    if let Some(b) = branch.filter(|s| !s.is_empty() && s != "none") {
        body["branch"] = serde_json::Value::String(b);
    }
    if let Some(n) = stock {
        body["stock"] = serde_json::Value::from(n);
    }
    if let Some(c) = cost.filter(|s| !s.is_empty()) {
        body["cost"] = serde_json::Value::String(c);
    }
    if let Some(t) = notes.filter(|s| !s.is_empty()) {
        body["notes"] = serde_json::Value::String(t);
    }
    let resp = http
        .post(format!("{base}/api/v1/batches"))
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
        .map_err(|e| format!("Respuesta de lote inválida del servidor: {e}"))
}

/// GET `/api/v1/reports/near-expiry?days=N&branch=X` (Bearer). Batches expiring
/// within `days` (default 30 server-side) including already-expired, urgent
/// first. `branch` acota al local (`"none"` = casa matriz, ausente = todos).
#[tauri::command]
pub async fn near_expiry(
    state: State<'_, SessionState>,
    server_url: String,
    days: Option<i64>,
    branch: Option<String>,
) -> Result<Vec<NearExpiryRow>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/reports/near-expiry"))
        .bearer_auth(token.expose_secret());
    if let Some(d) = days {
        req = req.query(&[("days", d)]);
    }
    if let Some(b) = branch.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("branch", b)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de vencimientos inválida del servidor: {e}"))
}

/// Percent-encode a single URL path segment (barcode). Unreserved ASCII stays
/// literal; everything else → `%XX` so the axum route receives the raw code.
fn encode_path_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

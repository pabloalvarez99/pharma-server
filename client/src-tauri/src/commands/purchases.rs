//! Purchasing commands: purchase orders, suppliers, payments, receiving.

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::{
    NewPurchaseOrderItem, PurchaseOrder, PurchaseOrderDetail, PurchasePayment,
    PurchasePaymentSummary, ReceiveLine, Supplier,
};

/// GET `/api/v1/purchase-orders` (Bearer, cashier+). `status` / `limit` are
/// optional query params forwarded to the server. Returns header-only rows
/// (no line items). Requires cashier+ role — identical to cash-sessions access.
#[tauri::command]
pub async fn list_purchase_orders(
    state: State<'_, SessionState>,
    server_url: String,
    status: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<PurchaseOrder>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/purchase-orders"))
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
        .map_err(|e| format!("Respuesta de órdenes de compra inválida del servidor: {e}"))
}

/// GET `/api/v1/purchase-orders/{id}` (Bearer, cashier+) — full PO WITH line
/// items. Unlike `list_purchase_orders` (header-only), this populates `items`
/// so the detail drawer can show what was ordered and what's left to receive.
#[tauri::command]
pub async fn get_purchase_order(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<PurchaseOrderDetail, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/purchase-orders/{id}"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de orden de compra inválida del servidor: {e}"))
}

/// GET `/api/v1/purchase-orders/{id}/payments` (Bearer, cashier+) — accounts-
/// payable rollup of a PO: `total` / `paid` / `balance` (STRING Decimals),
/// `fully_paid`, and the recorded payments. Drives the "Cuenta por pagar" block
/// of the PO detail drawer.
#[tauri::command]
pub async fn get_po_payments(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<PurchasePaymentSummary, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/purchase-orders/{id}/payments"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de pagos de OC inválida del servidor: {e}"))
}

/// POST `/api/v1/purchase-orders/{id}/payments` (Bearer, admin+) — record a
/// supplier payment against a PO. `amount` is a STRING (Decimal) forwarded
/// verbatim; `payment_method` is `cash`/`bank`/`card`/`transfer`. `cash_session`
/// is required by the server when paying `cash` with an open drawer (so the
/// outflow shows in the arqueo). Empty optionals are omitted. Returns the
/// created payment.
#[tauri::command]
pub async fn create_po_payment(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    amount: String,
    payment_method: Option<String>,
    cash_session: Option<String>,
    reference: Option<String>,
) -> Result<PurchasePayment, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({ "amount": amount });
    if let Some(v) = payment_method.filter(|s| !s.is_empty()) {
        body["payment_method"] = serde_json::Value::String(v);
    }
    if let Some(v) = cash_session.filter(|s| !s.is_empty()) {
        body["cash_session"] = serde_json::Value::String(v);
    }
    if let Some(v) = reference.filter(|s| !s.is_empty()) {
        body["reference"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/purchase-orders/{id}/payments"))
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
        .map_err(|e| format!("Respuesta de pago de OC inválida del servidor: {e}"))
}

/// GET `/api/v1/suppliers` (Bearer, cashier+). `search` filters by name on the
/// server; `limit` caps rows. Used by the suppliers list and the "Nueva OC"
/// supplier picker.
#[tauri::command]
pub async fn list_suppliers(
    state: State<'_, SessionState>,
    server_url: String,
    search: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Supplier>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/suppliers"))
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
        .map_err(|e| format!("Respuesta de proveedores inválida del servidor: {e}"))
}

/// POST `/api/v1/suppliers` (Bearer, admin+) — create a supplier. Body:
/// `{ name, rut?, contact_name?, contact_email?, contact_phone? }`. Only `name`
/// is required; empty optionals are omitted. Returns the created [`Supplier`].
#[tauri::command]
pub async fn create_supplier(
    state: State<'_, SessionState>,
    server_url: String,
    name: String,
    rut: Option<String>,
    contact_name: Option<String>,
    contact_email: Option<String>,
    contact_phone: Option<String>,
) -> Result<Supplier, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({ "name": name });
    if let Some(v) = rut.filter(|s| !s.is_empty()) {
        body["rut"] = serde_json::Value::String(v);
    }
    if let Some(v) = contact_name.filter(|s| !s.is_empty()) {
        body["contact_name"] = serde_json::Value::String(v);
    }
    if let Some(v) = contact_email.filter(|s| !s.is_empty()) {
        body["contact_email"] = serde_json::Value::String(v);
    }
    if let Some(v) = contact_phone.filter(|s| !s.is_empty()) {
        body["contact_phone"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/suppliers"))
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
        .map_err(|e| format!("Respuesta de proveedor inválida del servidor: {e}"))
}

/// POST `/api/v1/purchase-orders` (Bearer, admin+) — create a draft PO. `items`
/// is forwarded verbatim; each line's `unit_cost` is a STRING (Decimal) and the
/// server computes the per-line `subtotal` + header `total`. `currency` defaults
/// to CLP server-side when omitted. Returns the created PO header.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_purchase_order(
    state: State<'_, SessionState>,
    server_url: String,
    supplier: String,
    items: Vec<NewPurchaseOrderItem>,
    currency: Option<String>,
    notes: Option<String>,
    external_ref: Option<String>,
    branch: Option<String>,
) -> Result<PurchaseOrder, String> {
    if items.is_empty() {
        return Err("La orden de compra requiere al menos un ítem.".to_string());
    }
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "supplier": supplier,
        "items": items,
    });
    if let Some(v) = currency.filter(|s| !s.is_empty()) {
        body["currency"] = serde_json::Value::String(v);
    }
    if let Some(v) = notes.filter(|s| !s.is_empty()) {
        body["notes"] = serde_json::Value::String(v);
    }
    if let Some(v) = external_ref.filter(|s| !s.is_empty()) {
        body["external_ref"] = serde_json::Value::String(v);
    }
    // Sucursal que recibe la mercadería (V2.1): se fija al CREAR la OC porque
    // el comprador ya sabe para qué local compra, y así dos recepciones
    // parciales no pueden contradecirse de local.
    if let Some(v) = branch.filter(|s| !s.is_empty() && s != "none") {
        body["branch"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/purchase-orders"))
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
        .map_err(|e| format!("Respuesta de creación de OC inválida del servidor: {e}"))
}

/// POST `/api/v1/purchase-orders/{id}/receive` (Bearer, admin+) — goods receipt.
/// The server bumps stock, recomputes weighted-average cost, appends a
/// `stock_movement`, advances each line's `qty_received`, and flips the PO to
/// `received` (or `partially_received`). Body: `{ lines: [{ po_line_id,
/// qty_received }], notes? }`. Receiving is only legal from `sent` / `approved`
/// / `partially_received` — a `draft` PO comes back 409 (server message
/// surfaced as-is). Returns the updated PO header.
#[tauri::command]
pub async fn receive_purchase_order(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    lines: Vec<ReceiveLine>,
    notes: Option<String>,
) -> Result<PurchaseOrder, String> {
    if lines.is_empty() {
        return Err("La recepción requiere al menos una línea.".to_string());
    }
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({ "lines": lines });
    if let Some(v) = notes.filter(|s| !s.is_empty()) {
        body["notes"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/purchase-orders/{id}/receive"))
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
        .map_err(|e| format!("Respuesta de recepción inválida del servidor: {e}"))
}

/// POST `/api/v1/purchase-orders/{id}/send` (Bearer, admin+) — issue a draft PO
/// to the supplier (status `draft → sent`). This is the bridge that makes a PO
/// receivable: the server refuses to receive a `draft` (409, BUG-bob-002), so
/// the operator must send it first. No body. Only legal from `draft` — any other
/// status comes back 409 (server message surfaced as-is). Returns the PO header.
#[tauri::command]
pub async fn send_purchase_order(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<PurchaseOrder, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/purchase-orders/{id}/send"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de emisión de OC inválida del servidor: {e}"))
}

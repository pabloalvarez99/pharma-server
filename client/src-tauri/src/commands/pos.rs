//! POS commands: sales, refunds/devoluciones, receipts.

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, coded_error, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::{Devolucion, PosItem, Receipt, RefundItem};

/// POST `/api/v1/pos/sale` (Bearer + a FRESH `Idempotency-Key` minted here so
/// every "Cobrar" click is a distinct sale). `cash_amount` / `card_amount` are
/// optional STRINGS forwarded verbatim. On a non-2xx, the error is returned as
/// `"CODE|message"` (see [`coded_error`]) so the UI can special-case
/// `INSUFFICIENT_STOCK`. Returns the raw server JSON (order + items + alerts).
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn pos_sale(
    state: State<'_, SessionState>,
    server_url: String,
    items: Vec<PosItem>,
    payment_method: String,
    cash_amount: Option<String>,
    card_amount: Option<String>,
    customer: Option<String>,
    discount: Option<String>,
) -> Result<serde_json::Value, String> {
    if items.is_empty() {
        return Err("|El carrito está vacío.".to_string());
    }
    let token = token_of(&state).map_err(|e| format!("|{e}"))?;
    let http = client();
    let base = base(&server_url);

    let mut body = serde_json::json!({
        "items": items,
        "payment_method": payment_method,
    });
    if let Some(c) = cash_amount.filter(|s| !s.is_empty()) {
        body["cash_amount"] = serde_json::Value::String(c);
    }
    if let Some(c) = card_amount.filter(|s| !s.is_empty()) {
        body["card_amount"] = serde_json::Value::String(c);
    }
    // Single global discount amount (sum of the POS's per-line + global
    // discounts). The server clamps it into [0, subtotal].
    if let Some(d) = discount.filter(|s| !s.is_empty()) {
        body["discount"] = serde_json::Value::String(d);
    }
    // Attach the customer record id so the server awards loyalty for the sale.
    if let Some(c) = customer.filter(|s| !s.is_empty()) {
        body["customer"] = serde_json::Value::String(c);
    }

    let key = uuid::Uuid::new_v4().to_string();
    let resp = http
        .post(format!("{base}/api/v1/pos/sale"))
        .bearer_auth(token.expose_secret())
        .header("Idempotency-Key", key)
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;

    if !resp.status().is_success() {
        return Err(coded_error(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("|Respuesta de venta inválida del servidor: {e}"))
}

/// POST `/api/v1/pos/returns` (Bearer, cashier+) — create a refund/devolución.
/// `items` is forwarded verbatim (each `unit_price` a STRING, `restock` per line);
/// `order` links the devolución to a sale so the server can mark it refunded and
/// restock. Returns the raw `RefundResponse` JSON (`{ devolucion, items,
/// stock_movements, order_marked_refunded }`).
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn create_refund(
    state: State<'_, SessionState>,
    server_url: String,
    order: Option<String>,
    tipo: String,
    motivo: String,
    notas: Option<String>,
    items: Vec<RefundItem>,
    metodo_reembolso: Option<String>,
) -> Result<serde_json::Value, String> {
    if items.is_empty() {
        return Err("La devolución requiere al menos un ítem.".to_string());
    }
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "tipo": tipo,
        "motivo": motivo,
        "items": items,
    });
    if let Some(v) = order.filter(|s| !s.is_empty()) {
        body["order"] = serde_json::Value::String(v);
    }
    if let Some(v) = notas.filter(|s| !s.is_empty()) {
        body["notas"] = serde_json::Value::String(v);
    }
    if let Some(v) = metodo_reembolso.filter(|s| !s.is_empty()) {
        body["metodo_reembolso"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/pos/returns"))
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
        .map_err(|e| format!("Respuesta de devolución inválida del servidor: {e}"))
}

/// GET `/api/v1/returns` (Bearer) — devoluciones del tenant, newest first.
/// Optional `order` / `tipo` / `limit` filters are forwarded.
#[tauri::command]
pub async fn list_refunds(
    state: State<'_, SessionState>,
    server_url: String,
    order: Option<String>,
    tipo: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Devolucion>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/returns"))
        .bearer_auth(token.expose_secret());
    if let Some(v) = order.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("order", v)]);
    }
    if let Some(v) = tipo.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("tipo", v)]);
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
        .map_err(|e| format!("Respuesta de devoluciones inválida del servidor: {e}"))
}

/// GET `/api/v1/orders/{id}/receipt` (Bearer) — printable boleta for a completed
/// sale (tenant, folio, items, totals, vuelto, loyalty). Called right after a
/// successful `pos_sale` to show/print the ticket. A failure here never blocks
/// the sale itself (the order is already committed server-side).
#[tauri::command]
pub async fn get_receipt(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<Receipt, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/orders/{id}/receipt"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de boleta inválida del servidor: {e}"))
}

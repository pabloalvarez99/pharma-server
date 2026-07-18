//! Expenses (gastos / caja chica) commands.

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::Expense;

/// GET `/api/v1/expenses` (Bearer, cashier+). Optional `category` /
/// `payment_method` filters + `limit`. Returns the tenant's expenses
/// (egresos / caja chica). Requires cashier+ role — same ladder as the POS.
#[tauri::command]
pub async fn list_expenses(
    state: State<'_, SessionState>,
    server_url: String,
    category: Option<String>,
    payment_method: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Expense>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/expenses"))
        .bearer_auth(token.expose_secret());
    if let Some(c) = category.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("category", c)]);
    }
    if let Some(p) = payment_method.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("payment_method", p)]);
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
        .map_err(|e| format!("Respuesta de gastos inválida del servidor: {e}"))
}

/// POST `/api/v1/expenses` (Bearer, cashier+). Body `NewExpense`: `category`,
/// `description`, `amount` (STRING, forwarded verbatim), optional
/// `payment_method` (defaults to `cash` server-side), `note`, and `incurred_at`
/// (RFC3339). Returns the created expense.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn create_expense(
    state: State<'_, SessionState>,
    server_url: String,
    category: String,
    description: String,
    amount: String,
    payment_method: Option<String>,
    note: Option<String>,
    incurred_at: Option<String>,
) -> Result<Expense, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "category": category,
        "description": description,
        "amount": amount,
    });
    if let Some(p) = payment_method.filter(|s| !s.is_empty()) {
        body["payment_method"] = serde_json::Value::String(p);
    }
    if let Some(n) = note.filter(|s| !s.is_empty()) {
        body["note"] = serde_json::Value::String(n);
    }
    if let Some(t) = incurred_at.filter(|s| !s.is_empty()) {
        body["incurred_at"] = serde_json::Value::String(t);
    }
    let resp = http
        .post(format!("{base}/api/v1/expenses"))
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
        .map_err(|e| format!("Respuesta de gasto inválida del servidor: {e}"))
}

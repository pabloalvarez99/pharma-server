//! Cuenta corriente / fiado commands (V1).
//!
//! El CARGO lo genera el POS al vender con `pos_fiado` (no hay comando aparte).
//! Acá: leer el estado de cuenta y registrar abonos. Ambos cashier+ server-side.

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{token_of, SessionState};
use crate::types::{CustomerAccount, LedgerEntry};

/// GET `/api/v1/customers/{id}/cuenta` (Bearer, cashier+) — saldo, total fiado,
/// total abonado y los movimientos del ledger (más recientes primero).
#[tauri::command]
pub async fn customer_account(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<CustomerAccount, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/customers/{id}/cuenta"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de cuenta corriente inválida del servidor: {e}"))
}

/// POST `/api/v1/customers/{id}/abono` (Bearer, cashier+) — registrar un pago
/// del cliente contra su deuda. `amount` es STRING (Decimal) verbatim.
/// `cash_session` opcional: si el abono entra en efectivo con caja abierta, el
/// server lo liga para que salga en el arqueo.
#[tauri::command]
pub async fn record_abono(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    amount: String,
    cash_session: Option<String>,
    note: Option<String>,
) -> Result<LedgerEntry, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({ "amount": amount });
    if let Some(v) = cash_session.filter(|s| !s.is_empty()) {
        body["cash_session"] = serde_json::Value::String(v);
    }
    if let Some(v) = note.filter(|s| !s.is_empty()) {
        body["note"] = serde_json::Value::String(v);
    }
    let resp = http
        .post(format!("{base}/api/v1/customers/{id}/abono"))
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
        .map_err(|e| format!("Respuesta de abono inválida del servidor: {e}"))
}

/// GET `/api/v1/reports/por-cobrar` (Bearer, cashier+) — total por cobrar del
/// negocio + deudores ordenados por saldo (mayor primero).
#[tauri::command]
pub async fn debtors_report(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<crate::types::DebtorsReport, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/reports/por-cobrar"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de cuentas por cobrar inválida del servidor: {e}"))
}

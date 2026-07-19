//! Customers commands.
//!
//! The `/api/v1/customers/{search,{id},{id}/history}` surface lives on
//! `feat/customers-loyalty-history`, which is NOT guaranteed to be merged into
//! this client's server. So every customer command treats a 404 as a soft
//! "module not deployed" signal and rejects with a STABLE sentinel string the
//! view recognises — the Clientes view then renders a friendly upgrade note
//! instead of a hard error. Any other status maps through `error_message`.

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::{Customer, CustomerDetail, CustomerOrder};

/// Sentinel the customer commands reject with on 404 so the view can branch and
/// show "módulo requiere merge de customers-loyalty" instead of a raw error.
const CUSTOMERS_MISSING: &str = "CUSTOMERS_MODULE_MISSING";

/// GET `/api/v1/customers/search?q=` (Bearer). 404 → [`CUSTOMERS_MISSING`].
#[tauri::command]
pub async fn customer_search(
    state: State<'_, SessionState>,
    server_url: String,
    q: String,
) -> Result<Vec<Customer>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/customers/search"))
        .query(&[("q", &q)])
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if resp.status().as_u16() == 404 {
        return Err(CUSTOMERS_MISSING.to_string());
    }
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de clientes inválida del servidor: {e}"))
}

/// GET `/api/v1/customers/{id}` (Bearer) — detail w/ lifetime aggregates.
/// 404 → [`CUSTOMERS_MISSING`] (the endpoint itself, not a missing customer;
/// a real missing-customer comes back through the server envelope on the merged
/// branch, but here a 404 most likely means the route doesn't exist).
#[tauri::command]
pub async fn customer_detail(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<CustomerDetail, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/customers/{id}"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if resp.status().as_u16() == 404 {
        return Err(CUSTOMERS_MISSING.to_string());
    }
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de cliente inválida del servidor: {e}"))
}

/// POST `/api/v1/clientes` (Bearer, cashier+) — register a new customer at the
/// counter. Body `NewCustomer`: `name` (required) + optional `rut`/`phone`/
/// `email`; empty optionals are omitted so the server stores `null`. A 404 means
/// the customers module isn't deployed → [`CUSTOMERS_MISSING`] (same soft-degrade
/// as the read commands; this Spanish write surface ships on the same branch).
/// Returns the created [`Customer`].
#[tauri::command]
pub async fn create_customer(
    state: State<'_, SessionState>,
    server_url: String,
    name: String,
    rut: Option<String>,
    phone: Option<String>,
    email: Option<String>,
) -> Result<Customer, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({ "name": name });
    for (k, v) in [("rut", rut), ("phone", phone), ("email", email)] {
        if let Some(s) = v.filter(|s| !s.is_empty()) {
            body[k] = serde_json::Value::String(s);
        }
    }
    let resp = http
        .post(format!("{base}/api/v1/clientes"))
        .bearer_auth(token.expose_secret())
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if resp.status().as_u16() == 404 {
        return Err(CUSTOMERS_MISSING.to_string());
    }
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de cliente inválida del servidor: {e}"))
}

/// PATCH `/api/v1/clientes/{id}` (Bearer, cashier+) — edit a customer. Body
/// `UpdateCustomer`: every field optional (`name`/`rut`/`phone`/`email`/`active`).
/// Only fields explicitly provided are sent so omitted ones stay untouched; text
/// fields are forwarded verbatim (an empty string clears them), and `active` is
/// sent as a bool when present (activar/desactivar). 404 → [`CUSTOMERS_MISSING`].
/// Returns the updated [`Customer`].
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn update_customer(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    name: Option<String>,
    rut: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    active: Option<bool>,
) -> Result<Customer, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({});
    for (k, v) in [
        ("name", name),
        ("rut", rut),
        ("phone", phone),
        ("email", email),
    ] {
        if let Some(s) = v {
            body[k] = serde_json::Value::String(s);
        }
    }
    if let Some(a) = active {
        body["active"] = serde_json::Value::Bool(a);
    }
    let resp = http
        .patch(format!("{base}/api/v1/clientes/{id}"))
        .bearer_auth(token.expose_secret())
        .json(&body)
        .send()
        .await
        .map_err(conn_error)?;
    if resp.status().as_u16() == 404 {
        return Err(CUSTOMERS_MISSING.to_string());
    }
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de cliente inválida del servidor: {e}"))
}

/// GET `/api/v1/customers/{id}/history?limit=N` (Bearer). 404 →
/// [`CUSTOMERS_MISSING`]. Read-only projection of the customer's orders.
#[tauri::command]
pub async fn customer_history(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    limit: Option<u32>,
) -> Result<Vec<CustomerOrder>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/customers/{id}/history"))
        .bearer_auth(token.expose_secret());
    if let Some(n) = limit {
        req = req.query(&[("limit", n)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if resp.status().as_u16() == 404 {
        return Err(CUSTOMERS_MISSING.to_string());
    }
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de historial inválida del servidor: {e}"))
}

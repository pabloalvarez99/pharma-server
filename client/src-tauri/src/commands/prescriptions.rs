//! Prescriptions (recetas / controlados) commands.
//!
//! Ley 20.000 immutable log. The server exposes create/get/list for all
//! prescriptions plus a controlled-only ledger (`/libro-recetas`) and its CSV
//! export (ISP/DEIS). Creating a prescription requires pharmacist+ on the server
//! (a 403 surfaces as Spanish copy); reads are open to any authenticated user.

use secrecy::ExposeSecret;
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};
use crate::types::Prescription;

/// GET `/api/v1/prescriptions` (Bearer). Optional filters: `patient_rut`,
/// `controlled` (true → only controlled), `limit`. Newest first server-side.
#[tauri::command]
pub async fn list_prescriptions(
    state: State<'_, SessionState>,
    server_url: String,
    patient_rut: Option<String>,
    controlled: Option<bool>,
    limit: Option<u32>,
) -> Result<Vec<Prescription>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/prescriptions"))
        .bearer_auth(token.expose_secret());
    if let Some(r) = patient_rut.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("patient_rut", r)]);
    }
    if let Some(c) = controlled {
        req = req.query(&[("controlled", c)]);
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
        .map_err(|e| format!("Respuesta de recetas inválida del servidor: {e}"))
}

/// GET `/api/v1/prescriptions/{id}` (Bearer) — single prescription detail.
#[tauri::command]
pub async fn get_prescription(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<Prescription, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/prescriptions/{id}"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de receta inválida del servidor: {e}"))
}

/// POST `/api/v1/prescriptions` (Bearer, pharmacist+). Body mirrors
/// `NewPrescription`: `patient_name` / `patient_rut` required; `doctor_name` +
/// `doctor_rut` required by the server when `controlled = true`. `product` /
/// `customer` are optional record ids; `dispensed_at` defaults to now server-side.
/// Empty optional strings are dropped so the server applies its own defaults.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn create_prescription(
    state: State<'_, SessionState>,
    server_url: String,
    patient_name: String,
    patient_rut: String,
    controlled: bool,
    doctor_name: Option<String>,
    doctor_rut: Option<String>,
    product: Option<String>,
    customer: Option<String>,
    folio: Option<String>,
) -> Result<Prescription, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({
        "patient_name": patient_name,
        "patient_rut": patient_rut,
        "controlled": controlled,
    });
    for (key, val) in [
        ("doctor_name", doctor_name),
        ("doctor_rut", doctor_rut),
        ("product", product),
        ("customer", customer),
        ("folio", folio),
    ] {
        if let Some(v) = val.filter(|s| !s.is_empty()) {
            body[key] = serde_json::Value::String(v);
        }
    }
    let resp = http
        .post(format!("{base}/api/v1/prescriptions"))
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
        .map_err(|e| format!("Respuesta de receta inválida del servidor: {e}"))
}

/// GET `/api/v1/libro-recetas` (Bearer) — controlled-only ledger (Ley 20.000).
/// Same shape as `list_prescriptions` but `controlled = true` is enforced
/// server-side. Optional `patient_rut` / `limit` filters are forwarded.
#[tauri::command]
pub async fn libro_recetas(
    state: State<'_, SessionState>,
    server_url: String,
    patient_rut: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<Prescription>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/libro-recetas"))
        .bearer_auth(token.expose_secret());
    if let Some(r) = patient_rut.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("patient_rut", r)]);
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
        .map_err(|e| format!("Respuesta del libro de recetas inválida del servidor: {e}"))
}

/// GET `/api/v1/libro-recetas/export` (Bearer) — CSV of the controlled-drug
/// ledger (ISP/DEIS). The server responds with `text/csv`; we return the raw
/// CSV text so the webview can trigger a Blob download. Optional `patient_rut`
/// filter is forwarded.
#[tauri::command]
pub async fn export_libro_recetas(
    state: State<'_, SessionState>,
    server_url: String,
    patient_rut: Option<String>,
) -> Result<String, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/libro-recetas/export"))
        .bearer_auth(token.expose_secret());
    if let Some(r) = patient_rut.as_ref().filter(|s| !s.is_empty()) {
        req = req.query(&[("patient_rut", r)]);
    }
    let resp = req.send().await.map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.text()
        .await
        .map_err(|e| format!("Respuesta de exportación inválida del servidor: {e}"))
}

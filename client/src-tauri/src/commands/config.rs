//! Config Center commands: usuarios y roles, sucursales y cajas, respaldo
//! (PR #270, portado al árbol de módulos de feature/erp-parity).

use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::http::{base, client, conn_error, error_message};
use crate::state::{SessionState, token_of};

// --- config center: usuarios, sucursales, cajas, respaldo ------------------

/// Mirrors `crates/api/src/v1/config.rs::UserDto`. `password`/`tenant` are never
/// serialized server-side. `roles` are the canonical English keys.
#[derive(Serialize, Deserialize)]
pub struct ConfigUser {
    pub id: String,
    pub email: String,
    pub roles: Vec<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Mirrors `crates/domain/src/branches/model.rs::BranchDto` (sucursal).
#[derive(Serialize, Deserialize)]
pub struct Branch {
    pub id: String,
    pub name: String,
    pub code: Option<String>,
    pub address: Option<String>,
    pub comuna: Option<String>,
    pub phone: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Mirrors `crates/domain/src/branches/model.rs::RegisterDto` (caja). `branch`
/// is the owning sucursal record id or absent when unassigned.
#[derive(Serialize, Deserialize)]
pub struct Register {
    pub id: String,
    pub branch: Option<String>,
    pub name: String,
    pub code: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Mirrors `crates/api/src/v1/backup.rs::BackupReport` — an on-demand snapshot.
/// `duration_ms` is a server `u128` but always a small value; read as i64.
#[derive(Serialize, Deserialize)]
pub struct BackupReport {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub started_at: String,
    pub duration_ms: i64,
}

/// Mirrors `crates/db/src/backup_log.rs::BackupLogEntry` — one audited backup
/// attempt. Artifact fields are null on a failed attempt (`error` carries why).
#[derive(Serialize, Deserialize)]
pub struct BackupLogEntry {
    pub status: String,
    pub source: String,
    pub path: Option<String>,
    pub bytes: Option<i64>,
    pub sha256: Option<String>,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
    pub started_at: String,
}

// ---------------------------------------------------------------------------
// Config center commands — usuarios, sucursales, cajas, respaldo
// ---------------------------------------------------------------------------

/// GET `/api/v1/config/users` (Bearer, admin+) — every user of the tenant.
#[tauri::command]
pub async fn config_list_users(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<Vec<ConfigUser>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/config/users"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de usuarios inválida del servidor: {e}"))
}

/// POST `/api/v1/config/users` (Bearer, admin+). Body `{ email, password, roles }`.
/// 409 on a duplicate email, 403 when a non-owner assigns `owner` — both surface
/// as the server's Spanish message.
#[tauri::command]
pub async fn config_create_user(
    state: State<'_, SessionState>,
    server_url: String,
    email: String,
    password: String,
    roles: Vec<String>,
) -> Result<ConfigUser, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/config/users"))
        .bearer_auth(token.expose_secret())
        .json(&serde_json::json!({ "email": email, "password": password, "roles": roles }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de usuario inválida del servidor: {e}"))
}

/// PATCH `/api/v1/config/users/{id}` (Bearer, admin+). Body `{ roles }`. Guards
/// last-owner (409) and owner-grant (403) server-side.
#[tauri::command]
pub async fn config_update_user_roles(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    roles: Vec<String>,
) -> Result<ConfigUser, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .patch(format!("{base}/api/v1/config/users/{id}"))
        .bearer_auth(token.expose_secret())
        .json(&serde_json::json!({ "roles": roles }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de usuario inválida del servidor: {e}"))
}

/// POST `/api/v1/config/users/{id}/{enable|disable}` (Bearer, admin+) — toggle
/// `active`. The server refuses to disable the sole active owner (409).
#[tauri::command]
pub async fn config_set_user_active(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    active: bool,
) -> Result<ConfigUser, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let action = if active { "enable" } else { "disable" };
    let resp = http
        .post(format!("{base}/api/v1/config/users/{id}/{action}"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de usuario inválida del servidor: {e}"))
}

/// GET `/api/v1/sucursales` (Bearer). `active = Some(false)` includes inactive.
#[tauri::command]
pub async fn list_branches(
    state: State<'_, SessionState>,
    server_url: String,
    active: Option<bool>,
) -> Result<Vec<Branch>, String> {
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

/// POST `/api/v1/sucursales` (Bearer, admin+). `name` required; empty optionals
/// are omitted so the server stores null instead of an empty string.
#[tauri::command]
pub async fn create_branch(
    state: State<'_, SessionState>,
    server_url: String,
    name: String,
    code: Option<String>,
    address: Option<String>,
    comuna: Option<String>,
    phone: Option<String>,
) -> Result<Branch, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({ "name": name });
    for (k, v) in [
        ("code", code),
        ("address", address),
        ("comuna", comuna),
        ("phone", phone),
    ] {
        if let Some(s) = v.filter(|s| !s.is_empty()) {
            body[k] = serde_json::Value::String(s);
        }
    }
    let resp = http
        .post(format!("{base}/api/v1/sucursales"))
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
        .map_err(|e| format!("Respuesta de sucursal inválida del servidor: {e}"))
}

/// PATCH `/api/v1/sucursales/{id}` (Bearer, admin+). Only the supplied fields are
/// forwarded; the server skips `None`/empty (no change). `active` toggles it.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_branch(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    name: Option<String>,
    code: Option<String>,
    address: Option<String>,
    comuna: Option<String>,
    phone: Option<String>,
    active: Option<bool>,
) -> Result<Branch, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({});
    for (k, v) in [
        ("name", name),
        ("code", code),
        ("address", address),
        ("comuna", comuna),
        ("phone", phone),
    ] {
        if let Some(s) = v {
            body[k] = serde_json::Value::String(s);
        }
    }
    if let Some(a) = active {
        body["active"] = serde_json::Value::Bool(a);
    }
    let resp = http
        .patch(format!("{base}/api/v1/sucursales/{id}"))
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
        .map_err(|e| format!("Respuesta de sucursal inválida del servidor: {e}"))
}

/// DELETE `/api/v1/sucursales/{id}` (Bearer, admin+) — soft-delete (deactivate).
#[tauri::command]
pub async fn delete_branch(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<(), String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .delete(format!("{base}/api/v1/sucursales/{id}"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    Ok(())
}

/// GET `/api/v1/cajas` (Bearer). `active` / `branch` (sucursal id) optional.
#[tauri::command]
pub async fn list_registers(
    state: State<'_, SessionState>,
    server_url: String,
    active: Option<bool>,
    branch: Option<String>,
) -> Result<Vec<Register>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http.get(format!("{base}/api/v1/cajas")).bearer_auth(token.expose_secret());
    if let Some(a) = active {
        req = req.query(&[("active", a.to_string())]);
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
        .map_err(|e| format!("Respuesta de cajas inválida del servidor: {e}"))
}

/// POST `/api/v1/cajas` (Bearer, admin+). `name` required; `branch` (sucursal id)
/// optional and must belong to the tenant (404 otherwise).
#[tauri::command]
pub async fn create_register(
    state: State<'_, SessionState>,
    server_url: String,
    name: String,
    branch: Option<String>,
    code: Option<String>,
) -> Result<Register, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({ "name": name });
    if let Some(b) = branch.filter(|s| !s.is_empty()) {
        body["branch"] = serde_json::Value::String(b);
    }
    if let Some(c) = code.filter(|s| !s.is_empty()) {
        body["code"] = serde_json::Value::String(c);
    }
    let resp = http
        .post(format!("{base}/api/v1/cajas"))
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
        .map_err(|e| format!("Respuesta de caja inválida del servidor: {e}"))
}

/// PATCH `/api/v1/cajas/{id}` (Bearer, admin+). Supplied fields only; `branch`
/// reassigns (must be the tenant's); `active` toggles it.
#[tauri::command]
pub async fn update_register(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
    name: Option<String>,
    branch: Option<String>,
    code: Option<String>,
    active: Option<bool>,
) -> Result<Register, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({});
    for (k, v) in [("name", name), ("branch", branch), ("code", code)] {
        if let Some(s) = v.filter(|s| !s.is_empty()) {
            body[k] = serde_json::Value::String(s);
        }
    }
    if let Some(a) = active {
        body["active"] = serde_json::Value::Bool(a);
    }
    let resp = http
        .patch(format!("{base}/api/v1/cajas/{id}"))
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
        .map_err(|e| format!("Respuesta de caja inválida del servidor: {e}"))
}

/// DELETE `/api/v1/cajas/{id}` (Bearer, admin+) — soft-delete (deactivate).
#[tauri::command]
pub async fn delete_register(
    state: State<'_, SessionState>,
    server_url: String,
    id: String,
) -> Result<(), String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .delete(format!("{base}/api/v1/cajas/{id}"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    Ok(())
}

/// POST `/api/v1/admin/backup` (Bearer, admin+) — tar+gzip the data dir now.
/// Returns the artifact path + size + sha256.
#[tauri::command]
pub async fn run_backup(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<BackupReport, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/admin/backup"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de respaldo inválida del servidor: {e}"))
}

/// GET `/api/v1/admin/backup?limit=N` (Bearer, admin+) — the snapshot audit log,
/// newest first.
#[tauri::command]
pub async fn list_backups(
    state: State<'_, SessionState>,
    server_url: String,
    limit: Option<u32>,
) -> Result<Vec<BackupLogEntry>, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut req = http
        .get(format!("{base}/api/v1/admin/backup"))
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
        .map_err(|e| format!("Respuesta de respaldos inválida del servidor: {e}"))
}

// --- PR #272: SII (certificado y CAF), medios de pago, ficha del negocio ---

/// Mirrors `config.rs::PaymentMethodsResponse` — active + catalog tender keys.
#[derive(Serialize, Deserialize)]
pub struct PaymentMethods {
    pub methods: Vec<String>,
    pub available: Vec<String>,
}

/// Mirrors `config.rs::BusinessProfile` — the general business card (UI/receipt).
/// SII emisor identity lives separately under the `dte.emisor` setting.
#[derive(Serialize, Deserialize)]
pub struct BusinessProfile {
    pub razon_social: Option<String>,
    pub giro: Option<String>,
    pub direccion: Option<String>,
    pub telefono: Option<String>,
    pub email: Option<String>,
}

/// Mirrors the `dte.rs::upload_cert` 201 body. Dates are `YYYY-MM-DD`.
#[derive(Serialize, Deserialize)]
pub struct CertUploadResult {
    pub id: String,
    pub rut: String,
    pub vigencia_desde: String,
    pub vigencia_hasta: String,
    pub blob_bytes: u64,
}

/// Mirrors the `dte.rs::upload_caf` 201 body — the imported folio range.
#[derive(Serialize, Deserialize)]
pub struct CafUploadResult {
    pub id: String,
    pub tipo: i32,
    pub folio_desde: i64,
    pub folio_hasta: i64,
    pub next_folio: i64,
    pub rut_emisor: String,
}

// ---------------------------------------------------------------------------
// Config center commands — medios de pago, ficha de negocio, SII cert + CAF
// ---------------------------------------------------------------------------

/// GET `/api/v1/config/payment-methods` (Bearer) — active + available tender.
#[tauri::command]
pub async fn config_get_payment_methods(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<PaymentMethods, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/config/payment-methods"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de medios de pago inválida del servidor: {e}"))
}

/// PUT `/api/v1/config/payment-methods` (Bearer, admin+). Body `{ methods }`.
#[tauri::command]
pub async fn config_set_payment_methods(
    state: State<'_, SessionState>,
    server_url: String,
    methods: Vec<String>,
) -> Result<PaymentMethods, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .put(format!("{base}/api/v1/config/payment-methods"))
        .bearer_auth(token.expose_secret())
        .json(&serde_json::json!({ "methods": methods }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de medios de pago inválida del servidor: {e}"))
}

/// GET `/api/v1/config/business` (Bearer) — the business card.
#[tauri::command]
pub async fn config_get_business(
    state: State<'_, SessionState>,
    server_url: String,
) -> Result<BusinessProfile, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .get(format!("{base}/api/v1/config/business"))
        .bearer_auth(token.expose_secret())
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de negocio inválida del servidor: {e}"))
}

/// PUT `/api/v1/config/business` (Bearer, admin+). `razonSocial` required; empty
/// optionals are omitted so the server stores null.
#[tauri::command]
pub async fn config_set_business(
    state: State<'_, SessionState>,
    server_url: String,
    razon_social: String,
    giro: Option<String>,
    direccion: Option<String>,
    telefono: Option<String>,
    email: Option<String>,
) -> Result<BusinessProfile, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let mut body = serde_json::json!({ "razon_social": razon_social });
    for (k, v) in [
        ("giro", giro),
        ("direccion", direccion),
        ("telefono", telefono),
        ("email", email),
    ] {
        if let Some(s) = v.filter(|s| !s.is_empty()) {
            body[k] = serde_json::Value::String(s);
        }
    }
    let resp = http
        .put(format!("{base}/api/v1/config/business"))
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
        .map_err(|e| format!("Respuesta de negocio inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/cert` (Bearer, admin+) — import the digital certificate.
/// The PFX bytes are base64; the passphrase is verified server-side, used to
/// encrypt-at-rest and never persisted.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn dte_upload_cert(
    state: State<'_, SessionState>,
    server_url: String,
    pfx_base64: String,
    cert_passphrase: String,
    rut: String,
    vigencia_desde: String,
    vigencia_hasta: String,
) -> Result<CertUploadResult, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/dte/cert"))
        .bearer_auth(token.expose_secret())
        .json(&serde_json::json!({
            "pfx_base64": pfx_base64,
            "cert_passphrase": cert_passphrase,
            "rut": rut,
            "vigencia_desde": vigencia_desde,
            "vigencia_hasta": vigencia_hasta,
        }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de certificado inválida del servidor: {e}"))
}

/// POST `/api/v1/dte/caf` (Bearer, admin+) — import a CAF (folios) XML.
#[tauri::command]
pub async fn dte_upload_caf(
    state: State<'_, SessionState>,
    server_url: String,
    xml: String,
) -> Result<CafUploadResult, String> {
    let token = token_of(&state)?;
    let http = client();
    let base = base(&server_url);
    let resp = http
        .post(format!("{base}/api/v1/dte/caf"))
        .bearer_auth(token.expose_secret())
        .json(&serde_json::json!({ "xml": xml }))
        .send()
        .await
        .map_err(conn_error)?;
    if !resp.status().is_success() {
        return Err(error_message(resp).await);
    }
    resp.json()
        .await
        .map_err(|e| format!("Respuesta de CAF inválida del servidor: {e}"))
}

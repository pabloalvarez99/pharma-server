//! `POST /admin/v1/tenants` — SaaS signup provisioning (SP2).
//!
//! The license-server (web signup) calls this to create a tenant + admin user
//! on the cloud node. NOT part of `/api/v1`: it is machine-to-machine, gated by
//! a shared secret (`PHARMA__PROVISIONING__KEY`) compared constant-time against
//! the `X-Provisioning-Key` header — never JWT.
//!
//! FAIL-CLOSED / invisible-by-default: when no key is configured (every on-prem
//! install) the handler answers 404 exactly like an unknown route, so a probe
//! cannot tell the feature exists. Wrong key ⇒ 401 `PROVISIONING_KEY_INVALID`.
//!
//! Business logic lives in `domain::provisioning` (shared with the CLI
//! `tenant-create`/`user-create` commands); this handler only validates,
//! hashes the password and maps errors to the envelope:
//! 201 `{tenant_id, slug}` · 409 `TENANT_EXISTS` (slug o RUT duplicado) ·
//! 422 `VALIDATION_ERROR` (RUT/vertical/password) · audit line via tracing
//! (slug + rut, never the password).

use axum::{extract::State, http::HeaderMap, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::AppState;

/// Mirrors `setup.rs` — same minimum the first-run onboarding enforces.
const MIN_PASSWORD_LEN: usize = 8;

pub const PROVISIONING_HEADER: &str = "x-provisioning-key";

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/v1/tenants", post(create_tenant))
}

#[derive(Debug, Deserialize)]
struct CreateTenantRequest {
    /// Optional; derived from `business_name` when absent.
    #[serde(default)]
    slug: Option<String>,
    business_name: String,
    rut: String,
    vertical: String,
    admin_email: String,
    admin_password: String,
}

#[derive(Debug, Serialize)]
struct CreateTenantResponse {
    tenant_id: String,
    slug: String,
}

fn unprocessable(msg: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION_ERROR", msg)
}

/// Constant-time byte comparison (same routine as the `/metrics` token gate).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

async fn create_tenant(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateTenantRequest>,
) -> Result<(StatusCode, Json<CreateTenantResponse>), ApiError> {
    // No key configured ⇒ the route "does not exist" (on-prem default).
    let expected = s
        .provisioning_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or_else(ApiError::not_found)?;

    let supplied = headers
        .get(PROVISIONING_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !constant_time_eq(expected.as_bytes(), supplied.as_bytes()) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "PROVISIONING_KEY_INVALID",
            "Clave de aprovisionamiento inválida.",
        ));
    }

    let db = s.db.as_ref().ok_or_else(ApiError::service_unavailable)?;

    // --- validation → 422 (spec SP2) -----------------------------------
    if req.business_name.trim().is_empty() {
        return Err(unprocessable("Indica el nombre del negocio."));
    }
    let email = req.admin_email.trim().to_lowercase();
    if !email.contains('@') || !email.contains('.') || email.contains(' ') {
        return Err(unprocessable(
            "El correo no tiene un formato válido (ej: dueno@minegocio.cl).",
        ));
    }
    if req.admin_password.len() < MIN_PASSWORD_LEN {
        return Err(unprocessable(
            "La contraseña debe tener al menos 8 caracteres.",
        ));
    }
    let rut =
        domain::provisioning::validate_rut(&req.rut).map_err(|e| unprocessable(e.to_string()))?;
    let vertical = domain::provisioning::validate_vertical(&req.vertical)
        .map_err(|e| unprocessable(e.to_string()))?;

    let hash = auth::password::hash(&req.admin_password).map_err(|e| {
        tracing::error!(error = %e, "provisioning: password hash failed");
        ApiError::service_unavailable()
    })?;

    let input = domain::provisioning::ProvisionInput {
        slug: req.slug.clone(),
        business_name: req.business_name.clone(),
        rut: rut.clone(),
        vertical,
        admin_email: email,
        admin_password_hash: hash,
    };
    let created = domain::provisioning::provision_tenant(db, input)
        .await
        .map_err(|e| match e {
            domain::DomainError::Conflict(msg) => {
                ApiError::new(StatusCode::CONFLICT, "TENANT_EXISTS", msg)
            }
            domain::DomainError::Invalid(msg) => unprocessable(msg),
            other => ApiError::from(other),
        })?;

    // Audit line: slug + rut, NEVER the password (spec SP2).
    tracing::info!(
        tenant_id = %created.tenant_id,
        slug = %created.slug,
        %rut,
        "tenant provisioned via /admin/v1/tenants"
    );

    Ok((
        StatusCode::CREATED,
        Json(CreateTenantResponse {
            tenant_id: created.tenant_id,
            slug: created.slug,
        }),
    ))
}

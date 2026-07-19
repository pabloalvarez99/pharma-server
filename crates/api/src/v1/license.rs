//! License admin endpoints — hot-reload + status (read-only).
//!
//! - `POST /api/v1/admin/license/reload` re-reads `<data_dir>/license.json`,
//!   verifies offline, swaps the active license atomically (lock-free via
//!   `ArcSwap`). Admin/owner only. Returns the freshly-loaded summary.
//! - `GET  /api/v1/admin/license/status` returns the active license summary
//!   without touching disk. Same role.
//!
//! Invariants:
//! - Reload never blocks core ERP: parse/verify errors fall back to
//!   `License::free_default` (ADR-0005). Caller still gets the new summary so
//!   the UI can warn.
//! - Reload is idempotent: calling twice with the same file is a no-op
//!   semantically (same License contents → same `ArcSwap` pointee).
//! - Tenant binding (BUG-006): a *validly signed* license is only accepted if
//!   its `tenant_id` belongs to the reloading operator's tenant (JWT
//!   `tenant_id` claim). A foreign-tenant license is rejected (403) and the
//!   active license is left untouched. This check applies ONLY to a
//!   signature-verified license — a missing/invalid file still falls back to
//!   Free (ADR-0005), with no tenant binding to enforce.

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

const ADMIN_ROLES: &[&str] = &["admin", "owner"];

#[derive(Serialize)]
pub struct LicenseSummary {
    pub tier: String,
    pub status: &'static str,
    pub license_id: String,
    pub features: Vec<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub key_id: String,
    pub seat_count: u32,
}

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/license/reload", post(reload_license))
        .route("/api/v1/admin/license/status", get(license_status))
        .route("/api/v1/admin/license/crl/status", get(crl_status))
}

fn require_admin(claims: &auth::Claims) -> Result<(), ApiError> {
    if claims
        .roles
        .iter()
        .any(|r| ADMIN_ROLES.contains(&r.as_str()))
    {
        Ok(())
    } else {
        Err(ApiError::forbidden())
    }
}

/// BUG-006: a validly-signed license must belong to the operator's own tenant.
///
/// The JWT carries the caller's tenant as a SurrealDB record-id string
/// (`tenant:<key>`); the license carries `tenant_id` as a `Uuid`. The binding
/// holds when the record-id key equals the license `tenant_id` (i.e. the
/// tenant record id IS the license tenant UUID — the on-prem activation
/// contract). The full claim string is also accepted as a fallback so a plain
/// UUID claim (no `tenant:` table prefix) still matches.
///
/// Returns `Err(forbidden)` on mismatch; the caller must NOT swap the license.
fn require_license_tenant(
    license_tenant: uuid::Uuid,
    caller_tenant_claim: &str,
) -> Result<(), ApiError> {
    let want = license_tenant.to_string();
    // `tenant:abc` → `abc`; a bare id stays as-is.
    let caller_key = caller_tenant_claim
        .split_once(':')
        .map(|(_, key)| key)
        .unwrap_or(caller_tenant_claim);
    if caller_key.eq_ignore_ascii_case(&want) || caller_tenant_claim.eq_ignore_ascii_case(&want) {
        Ok(())
    } else {
        tracing::warn!(
            license_tenant = %want,
            caller_tenant = %caller_tenant_claim,
            "license reload rejected: license tenant_id does not match caller tenant"
        );
        Err(ApiError::forbidden())
    }
}

fn summarize(lic: &license::License) -> LicenseSummary {
    let now = chrono::Utc::now();
    let grace = chrono::Duration::days(30);
    let status = if license::is_expired(lic, now, grace) {
        "expired"
    } else if license::is_in_grace(lic, now, grace) {
        "grace"
    } else {
        "active"
    };
    LicenseSummary {
        tier: lic.tier.as_str().to_string(),
        status,
        license_id: lic.license_id.clone(),
        features: lic.features.clone(),
        expires_at: lic.expires_at,
        key_id: lic.key_id.clone(),
        seat_count: lic.seat_count,
    }
}

/// Re-lee `<data_dir>/license.json`, verifica offline y swappea la licencia
/// activa (lock-free vía `ArcSwap`). Requiere admin+. Errores de parse/verify
/// caen a `License::free_default` (ADR-0005).
#[utoipa::path(
    post, path = "/api/v1/admin/license/reload", tag = "License",
    responses(
        (status = 200, description = "Licencia recargada (puede ser fallback Free)", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Rol insuficiente (requiere admin+)", body = crate::error::ErrorEnvelope),
        (status = 503, description = "license_path no configurado", body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = []))
)]
pub async fn reload_license(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<LicenseSummary>, ApiError> {
    require_admin(&claims)?;
    let path = state
        .license_path
        .as_ref()
        .ok_or_else(ApiError::service_unavailable)?;

    // ADR-0005: a missing/invalid file must still fall back to Free so core
    // ERP keeps working — there is no tenant binding to enforce on a fallback.
    // A *successfully verified* license, however, must belong to the operator's
    // own tenant (BUG-006); otherwise reject without swapping the active one.
    let fresh = if path.exists() {
        match license::load_from_disk(path) {
            Ok(lic) => {
                require_license_tenant(lic.tenant_id, &claims.tenant_id)?;
                tracing::info!(
                    tier = lic.tier.as_str(),
                    license_id = %lic.license_id,
                    "license verified + tenant-bound on reload"
                );
                lic
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(),
                    "license file present but invalid; falling back to Free");
                license::License::free_default(uuid::Uuid::nil())
            }
        }
    } else {
        tracing::info!(path = %path.display(), "no license file; running Free tier");
        license::License::free_default(uuid::Uuid::nil())
    };

    let summary = summarize(&fresh);
    state.license.store(std::sync::Arc::new(fresh));
    tracing::info!(
        tier = %summary.tier,
        license_id = %summary.license_id,
        "license reloaded via admin endpoint"
    );
    Ok(Json(summary))
}

/// Devuelve el resumen de la licencia activa sin tocar disco. Requiere admin+.
/// (`tier`, `status`, `license_id`, `features`, `expires_at`, `key_id`, `seat_count`).
#[utoipa::path(
    get, path = "/api/v1/admin/license/status", tag = "License",
    responses(
        (status = 200, description = "Resumen de licencia activa", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Rol insuficiente (requiere admin+)", body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = []))
)]
pub async fn license_status(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<LicenseSummary>, ApiError> {
    require_admin(&claims)?;
    let lic = state.license.load();
    Ok(Json(summarize(&lic)))
}

/// Estado del cache local de revocación (CRL, ADR-0006). Read-only.
#[derive(Serialize)]
pub struct CrlStatus {
    /// Ruta del cache (`<data_dir>/crl_state.json`).
    pub crl_path: String,
    /// `false` si el archivo existe pero no se pudo parsear (se reporta como
    /// estado vacío; la revocación es best-effort y nunca bloquea el core).
    pub readable: bool,
    /// Última versión CRL aplicada (`0` = ninguna).
    pub last_seen_version: u64,
    /// Cuándo se aplicó la última versión (`null` si nunca).
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Cantidad de `license_id` revocados en el cache.
    pub revoked_count: usize,
    /// IDs revocados (ordenados; el set es chico por diseño — sólo licenses
    /// canceladas/reembolsadas, no el universo de licenses).
    pub revoked: Vec<String>,
    /// `license_id` de la licencia activa.
    pub active_license_id: String,
    /// `true` si la licencia activa figura en el set revocado. Cuando es
    /// `true`, el server ya degradó (o degradará al próximo reload/restart) a
    /// Free — el core gratis sigue operativo (ADR-0005 §6, nunca kill-switch).
    pub active_license_revoked: bool,
}

/// Devuelve el estado del cache de revocación (CRL) local sin tocar la red.
/// Requiere admin+. Permite al cliente/monitoreo ver si la licencia activa
/// quedó revocada y cuándo se sincronizó el CRL por última vez. El refresh
/// automático lo hace el job del scheduler (config `[crl]`, ADR-0006); la
/// importación manual, `pharma license crl-import`.
#[utoipa::path(
    get, path = "/api/v1/admin/license/crl/status", tag = "License",
    responses(
        (status = 200, description = "Estado del cache CRL local", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Rol insuficiente (requiere admin+)", body = crate::error::ErrorEnvelope),
        (status = 503, description = "license_path no configurado", body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = []))
)]
pub async fn crl_status(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<CrlStatus>, ApiError> {
    require_admin(&claims)?;
    let path = state
        .license_path
        .as_ref()
        .ok_or_else(ApiError::service_unavailable)?;
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let crl_path = license::default_crl_state_path(dir);

    // Cache ilegible ⇒ se reporta como vacío + `readable=false` (offline-first:
    // la revocación es best-effort, nunca un error que tumbe el endpoint).
    let (crl, readable) = match license::load_crl_state(&crl_path) {
        Ok(s) => (s, true),
        Err(e) => {
            tracing::warn!(error = %e, path = %crl_path.display(),
                "crl_state.json ilegible en status; se reporta vacío");
            (license::CrlState::default(), false)
        }
    };

    let active = state.license.load();
    let active_license_revoked = crl.is_revoked(&active.license_id);
    Ok(Json(CrlStatus {
        crl_path: crl_path.display().to_string(),
        readable,
        last_seen_version: crl.last_seen_version,
        updated_at: crl.updated_at,
        revoked_count: crl.revoked.len(),
        revoked: crl.revoked.iter().cloned().collect(),
        active_license_id: active.license_id.clone(),
        active_license_revoked,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use uuid::Uuid;

    #[test]
    fn tenant_bind_rejects_foreign_tenant_license() {
        // A validly-signed license issued for T2 must NOT be accepted by a T1
        // operator (BUG-006). Mismatch → 403, caller must not swap.
        let license_tenant = Uuid::new_v4(); // belongs to "T2"
        let caller_t1 = format!("tenant:{}", Uuid::new_v4()); // a different tenant
        let err = require_license_tenant(license_tenant, &caller_t1)
            .expect_err("foreign-tenant license must be rejected");
        assert_eq!(err.status, StatusCode::FORBIDDEN);
        assert_eq!(err.code, "FORBIDDEN");
    }

    #[test]
    fn tenant_bind_accepts_matching_record_id_claim() {
        // On-prem activation contract: the tenant record id key IS the license
        // tenant UUID. `tenant:<uuid>` claim must match.
        let t = Uuid::new_v4();
        let claim = format!("tenant:{t}");
        require_license_tenant(t, &claim).expect("matching tenant must be accepted");
    }

    #[test]
    fn tenant_bind_accepts_bare_uuid_claim() {
        // A claim with no `tenant:` table prefix (bare UUID) still matches.
        let t = Uuid::new_v4();
        require_license_tenant(t, &t.to_string()).expect("bare uuid claim must match");
    }

    #[test]
    fn tenant_bind_rejects_when_only_prefix_differs_but_uuid_differs() {
        // Same shape, different UUID → still a foreign tenant.
        let license_tenant = Uuid::new_v4();
        let other = format!("tenant:{}", Uuid::new_v4());
        assert!(require_license_tenant(license_tenant, &other).is_err());
    }
}

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

async fn reload_license(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<LicenseSummary>, ApiError> {
    require_admin(&claims)?;
    let path = state
        .license_path
        .as_ref()
        .ok_or_else(ApiError::service_unavailable)?;
    let fresh = crate::load_license_from(path);
    let summary = summarize(&fresh);
    state.license.store(std::sync::Arc::new(fresh));
    tracing::info!(
        tier = %summary.tier,
        license_id = %summary.license_id,
        "license reloaded via admin endpoint"
    );
    Ok(Json(summary))
}

async fn license_status(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<LicenseSummary>, ApiError> {
    require_admin(&claims)?;
    let lic = state.license.load();
    Ok(Json(summarize(&lic)))
}

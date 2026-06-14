//! Demo-seeding endpoint — `POST /api/v1/admin/seed-demo`.
//!
//! Admin/owner only, tenant-scoped (tenant ALWAYS from the JWT claim, never the
//! body). Delegates to [`domain::seed::seed_demo`] — the same service the CLI
//! (`pharma seed-demo`) uses, so the in-app "llenar con datos demo" button and
//! the terminal produce identical data.
//!
//! NEVER auto-runs: a human (admin in the app, or the operator in a terminal)
//! triggers it. All rows are DEMO-tagged and reversible via `force` (which wipes
//! the prior demo pack before re-seeding).

use std::sync::Arc;

use axum::{extract::State, routing::post, Json, Router};
use serde::Deserialize;
use surrealdb::sql::Thing;
use utoipa::ToSchema;

use domain::seed::{seed_demo, SeedSummary};

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

const ADMIN_ROLES: &[&str] = &["admin", "owner"];

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new().route("/api/v1/admin/seed-demo", post(seed_demo_handler))
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

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SeedDemoBody {
    /// `pharmacy` (default) o `minimarket`. Acepta sinónimos en español.
    #[serde(default)]
    pub vertical: Option<String>,
    /// Si ya existe data demo: `true` la regenera (wipe + reseed), `false`
    /// rechaza con 409 para no duplicar.
    #[serde(default)]
    pub force: bool,
}

/// POST /api/v1/admin/seed-demo — siembra data demo para el tenant del JWT.
#[utoipa::path(
    post,
    path = "/api/v1/admin/seed-demo",
    tag = "admin",
    request_body = SeedDemoBody,
    responses(
        (status = 200, description = "Resumen del seeding", body = SeedSummary),
        (status = 403, description = "Requiere rol admin/owner"),
        (status = 409, description = "Ya existe data demo (use force)"),
    ),
    security(("bearer" = []))
)]
async fn seed_demo_handler(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<SeedDemoBody>,
) -> Result<Json<SeedSummary>, ApiError> {
    require_admin(&claims)?;
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let vertical = body.vertical.as_deref().unwrap_or("pharmacy");
    let summary = seed_demo(&db, &tenant, vertical, body.force).await?;
    Ok(Json(summary))
}

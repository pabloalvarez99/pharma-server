//! Rubro pack endpoint — serves the declarative vertical pack
//! (`domain::rubro::RubroPack`) for the tenant's stored `business.vertical`.
//!
//! `GET /api/v1/rubro-pack` — any authenticated role (every view needs the
//! pack to gate its modules). The pack itself is static per rubro; only the
//! rubro selection is tenant data. Unset/unknown values fall back to the
//! generic `otro` pack (pivot rule: never default to farmacia).

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

const VERTICAL_KEY: &str = "business.vertical";

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new().route("/api/v1/rubro-pack", get(rubro_pack))
}

async fn rubro_pack(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<domain::rubro::RubroPack>, ApiError> {
    let db: Arc<db::Db> = s.db.clone().ok_or_else(ApiError::service_unavailable)?;
    let tenant = surrealdb::sql::thing(&claims.tenant_id)
        .map_err(|_| ApiError::unauthorized_invalid_token())?;

    let stored = domain::sales::service::get_setting(db.as_ref(), &tenant, VERTICAL_KEY)
        .await?
        .map(|s| s.value)
        .unwrap_or_default();

    Ok(Json(domain::rubro::pack_for(&stored).clone()))
}

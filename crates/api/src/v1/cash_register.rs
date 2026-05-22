//! Cash register (caja) — apertura, cierre, arqueo, movimientos.
//!
//! All routes JWT/tenant-scoped. Cashiers can open + close + move money;
//! reads are bearer.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::cash_register::{model::*, service};

use crate::role::cashier_plus;

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn user_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.sub).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    let reads = Router::new()
        .route("/api/v1/cash-sessions", get(list_sessions))
        .route("/api/v1/cash-sessions/{id}", get(get_session))
        .route("/api/v1/cash-sessions/{id}/arqueo", get(arqueo))
        .route("/api/v1/cash-sessions/{id}/movements", get(list_movements));

    let writes = Router::new()
        .route("/api/v1/cash-sessions", post(open_session))
        .route("/api/v1/cash-sessions/{id}/close", post(close_session))
        .route("/api/v1/cash-sessions/{id}/movements", post(add_movement))
        .route_layer(crate::role::layer(state, cashier_plus()));

    reads.merge(writes)
}

/// Apertura de caja. Requiere cashier+.
#[utoipa::path(post, path = "/api/v1/cash-sessions", tag = "CashRegister",
    request_body = serde_json::Value,
    responses((status = 201, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 409, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn open_session(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<OpenSessionInput>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let user = user_of(&claims)?;
    let s = service::open_session(db.as_ref(), &tenant, &user, body).await?;
    Ok((StatusCode::CREATED, Json(s)).into_response())
}

/// Lista cash sessions del tenant.
#[utoipa::path(get, path = "/api/v1/cash-sessions", tag = "CashRegister",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_sessions(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<SessionFilters>,
) -> Result<Json<Vec<CashSessionDto>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(
        service::list_sessions(db.as_ref(), &tenant, filters).await?,
    ))
}

/// Detalle de una cash session.
#[utoipa::path(get, path = "/api/v1/cash-sessions/{id}", tag = "CashRegister",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn get_session(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<CashSessionDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(service::get_session(db.as_ref(), &tenant, &id).await?))
}

/// Arqueo preview de una caja (estado, totales).
#[utoipa::path(get, path = "/api/v1/cash-sessions/{id}/arqueo", tag = "CashRegister",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn arqueo(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<CloseSummary>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(service::arqueo(db.as_ref(), &tenant, &id).await?))
}

/// Cierre de caja con conteo real (calcula diferencia). Requiere cashier+.
#[utoipa::path(post, path = "/api/v1/cash-sessions/{id}/close", tag = "CashRegister",
    params(("id" = String, Path)), request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope),
        (status = 409, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn close_session(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<CloseSessionInput>,
) -> Result<Json<CloseSummary>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(
        service::close_session(db.as_ref(), &tenant, &id, body).await?,
    ))
}

/// Crea movimiento de caja (in/out manual). Requiere cashier+.
#[utoipa::path(post, path = "/api/v1/cash-sessions/{id}/movements", tag = "CashRegister",
    params(("id" = String, Path)), request_body = serde_json::Value,
    responses((status = 201, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn add_movement(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<CashMovementInput>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let user = user_of(&claims).ok();
    let m = service::add_movement(db.as_ref(), &tenant, user.as_ref(), &id, body).await?;
    Ok((StatusCode::CREATED, Json(m)).into_response())
}

/// Lista movimientos de una caja.
#[utoipa::path(get, path = "/api/v1/cash-sessions/{id}/movements", tag = "CashRegister",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_movements(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Vec<CashMovementDto>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(
        service::list_movements(db.as_ref(), &tenant, &id).await?,
    ))
}

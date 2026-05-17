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

const CAJA_ROLES: &[&str] = &["admin", "owner", "cashier"];

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
        .route_layer(crate::role::layer(state, CAJA_ROLES));

    reads.merge(writes)
}

async fn open_session(
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

async fn list_sessions(
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

async fn get_session(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<CashSessionDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(service::get_session(db.as_ref(), &tenant, &id).await?))
}

async fn arqueo(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<CloseSummary>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(service::arqueo(db.as_ref(), &tenant, &id).await?))
}

async fn close_session(
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

async fn add_movement(
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

async fn list_movements(
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

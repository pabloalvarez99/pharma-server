//! Sucursales (branch) + cajas (register) HTTP handlers — config center.
//!
//! Tenant-scoped CRUD so the UI runs multi-sucursal / multi-caja from
//! Configuración. Reads require a valid bearer token (any role — the POS needs
//! to list cajas); mutations require `admin`/`owner` (configuring the business
//! topology is a back-office action, not a counter one). See migration 0032.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::role::admin_plus;
use crate::AppState;

use domain::branches::{model::*, service};

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    let reads = Router::new()
        .route("/api/v1/sucursales", get(list_branches))
        .route("/api/v1/sucursales/{id}", get(get_branch))
        .route("/api/v1/cajas", get(list_registers))
        .route("/api/v1/cajas/{id}", get(get_register));

    let writes = Router::new()
        .route("/api/v1/sucursales", post(create_branch))
        .route(
            "/api/v1/sucursales/{id}",
            axum::routing::patch(update_branch).delete(delete_branch),
        )
        .route("/api/v1/cajas", post(create_register))
        .route(
            "/api/v1/cajas/{id}",
            axum::routing::patch(update_register).delete(delete_register),
        )
        .route_layer(crate::role::layer(state, admin_plus()));

    reads.merge(writes)
}

// --- sucursales (branch) ---------------------------------------------------

/// Lista sucursales del tenant. `?active=false` para ver inactivas.
#[utoipa::path(get, path = "/api/v1/sucursales", tag = "ConfigCenter",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_branches(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<BranchFilters>,
) -> Result<Json<Vec<BranchDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_branches(&db, &t, filters).await?))
}

/// Detalle de una sucursal.
#[utoipa::path(get, path = "/api/v1/sucursales/{id}", tag = "ConfigCenter",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn get_branch(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<BranchDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_branch(&db, &t, &id).await?))
}

/// Crea una sucursal. Requiere admin+.
#[utoipa::path(post, path = "/api/v1/sucursales", tag = "ConfigCenter",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope), (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn create_branch(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewBranch>,
) -> Result<Json<BranchDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_branch(&db, &t, input).await?))
}

/// Actualiza una sucursal. Requiere admin+.
#[utoipa::path(patch, path = "/api/v1/sucursales/{id}", tag = "ConfigCenter",
    params(("id" = String, Path)), request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn update_branch(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<UpdateBranch>,
) -> Result<Json<BranchDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::update_branch(&db, &t, &id, patch).await?))
}

/// Desactiva (soft delete) una sucursal. Requiere admin+.
#[utoipa::path(delete, path = "/api/v1/sucursales/{id}", tag = "ConfigCenter",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn delete_branch(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    service::delete_branch(&db, &t, &id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

// --- cajas (register) ------------------------------------------------------

/// Lista cajas del tenant. `?active=false` para ver inactivas; `?branch=...`
/// filtra por sucursal.
#[utoipa::path(get, path = "/api/v1/cajas", tag = "ConfigCenter",
    responses((status = 200, body = serde_json::Value), (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_registers(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<RegisterFilters>,
) -> Result<Json<Vec<RegisterDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_registers(&db, &t, filters).await?))
}

/// Detalle de una caja.
#[utoipa::path(get, path = "/api/v1/cajas/{id}", tag = "ConfigCenter",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn get_register(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<RegisterDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_register(&db, &t, &id).await?))
}

/// Crea una caja. Requiere admin+. `branch` opcional; debe ser del tenant.
#[utoipa::path(post, path = "/api/v1/cajas", tag = "ConfigCenter",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope), (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, description = "Sucursal `branch` no encontrada", body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn create_register(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewRegister>,
) -> Result<Json<RegisterDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_register(&db, &t, input).await?))
}

/// Actualiza una caja. Requiere admin+. `branch` (si viene) debe ser del tenant.
#[utoipa::path(patch, path = "/api/v1/cajas/{id}", tag = "ConfigCenter",
    params(("id" = String, Path)), request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn update_register(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<UpdateRegister>,
) -> Result<Json<RegisterDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::update_register(&db, &t, &id, patch).await?))
}

/// Desactiva (soft delete) una caja. Requiere admin+.
#[utoipa::path(delete, path = "/api/v1/cajas/{id}", tag = "ConfigCenter",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn delete_register(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    service::delete_register(&db, &t, &id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

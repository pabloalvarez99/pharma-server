//! Supplier-operator HTTP surface for inbound federated orders.
//!
//! `agent_order` rows are created by the federated `/agent/inbox` handler
//! (signature-authenticated, no JWT). These endpoints are the *human* side:
//! the operator of the supplier tenant lists inbound orders and accepts or
//! rejects them. All routes are JWT/tenant-scoped (role `admin`/`owner`) so
//! one tenant never sees another's inbound orders. The remote buyer learns
//! the decision via the federated `po.status` topic.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::agent_orders::{model::*, service};

use crate::role::{admin_plus, cashier_plus};

fn tenant_of(claims: &auth::Claims) -> Result<surrealdb::sql::Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    let reads = Router::new()
        .route("/api/v1/agent-orders", get(list))
        .route("/api/v1/agent-orders/{id}", get(get_one))
        .route_layer(crate::role::layer(state.clone(), cashier_plus()));

    let writes = Router::new()
        .route("/api/v1/agent-orders/{id}/accept", post(accept))
        .route("/api/v1/agent-orders/{id}/reject", post(reject))
        .route("/api/v1/agent-orders/{id}/fulfill", post(fulfill))
        .route_layer(crate::role::layer(state, admin_plus()));

    reads.merge(writes)
}

/// Lista órdenes federadas entrantes (rol supplier) del tenant. Requiere cashier+.
#[utoipa::path(get, path = "/api/v1/agent-orders", tag = "AgentOrders",
    responses(
        (status = 200, description = "Lista de órdenes federadas", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Rol insuficiente (requiere cashier+)", body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn list(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<AgentOrderFilters>,
) -> Result<Json<Vec<AgentOrderDto>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(service::list(db.as_ref(), &tenant, filters).await?))
}

/// Detalle de una orden federada. Requiere cashier+.
#[utoipa::path(get, path = "/api/v1/agent-orders/{id}", tag = "AgentOrders",
    params(("id" = String, Path, description = "agent_order:xxx")),
    responses(
        (status = 200, description = "Detalle de la orden", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn get_one(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<AgentOrderDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(service::get(db.as_ref(), &tenant, &id).await?))
}

/// Aceptar una orden federada (status → accepted). Requiere admin+.
#[utoipa::path(post, path = "/api/v1/agent-orders/{id}/accept", tag = "AgentOrders",
    params(("id" = String, Path)),
    responses(
        (status = 200, description = "Orden aceptada", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Rol insuficiente (requiere admin+)", body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 409, description = "Transición de estado inválida", body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn accept(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<AgentOrderDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(
        service::decide(db.as_ref(), &tenant, &id, "accepted").await?,
    ))
}

/// Rechazar una orden federada (status → rejected). Requiere admin+.
#[utoipa::path(post, path = "/api/v1/agent-orders/{id}/reject", tag = "AgentOrders",
    params(("id" = String, Path)),
    responses(
        (status = 200, description = "Orden rechazada", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 409, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn reject(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<AgentOrderDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(
        service::decide(db.as_ref(), &tenant, &id, "rejected").await?,
    ))
}

/// Cerrar (fulfill) una orden ya aceptada. Requiere admin+.
#[utoipa::path(post, path = "/api/v1/agent-orders/{id}/fulfill", tag = "AgentOrders",
    params(("id" = String, Path)),
    responses(
        (status = 200, description = "Orden cumplida", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 409, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn fulfill(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<AgentOrderDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(service::fulfill(db.as_ref(), &tenant, &id).await?))
}

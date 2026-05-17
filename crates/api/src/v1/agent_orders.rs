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

const OP_ROLES: &[&str] = &["admin", "owner"];

fn tenant_of(claims: &auth::Claims) -> Result<surrealdb::sql::Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/agent-orders", get(list))
        .route("/api/v1/agent-orders/{id}", get(get_one))
        .route("/api/v1/agent-orders/{id}/accept", post(accept))
        .route("/api/v1/agent-orders/{id}/reject", post(reject))
        .route_layer(crate::role::layer(state, OP_ROLES))
}

async fn list(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<AgentOrderFilters>,
) -> Result<Json<Vec<AgentOrderDto>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(service::list(db.as_ref(), &tenant, filters).await?))
}

async fn get_one(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<AgentOrderDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(service::get(db.as_ref(), &tenant, &id).await?))
}

async fn accept(
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

async fn reject(
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

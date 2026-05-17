//! Sales HTTP handlers (Fase 4). POS sale, orders read, admin_setting CRUD.
//! Reads = bearer; mutations = role `admin`/`owner`. POS sale = role
//! `admin`/`owner`/`cashier` (cashier rol introduced here for counter staff).
//!
//! Idempotency: `POST /pos/sale` honors the `Idempotency-Key` header. The
//! domain layer signals a cache hit via `DomainError::Conflict("IDEMPOTENCY_CACHED:<json>")`;
//! we intercept that and reply with the cached payload + 200 instead of 409.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::sales::{model::*, service};

const WRITE_ROLES: &[&str] = &["admin", "owner"];
const POS_ROLES: &[&str] = &["admin", "owner", "cashier"];

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn user_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.sub).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

fn idempotency_key(h: &HeaderMap) -> Option<String> {
    h.get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

pub fn router(state: AppState) -> Router<AppState> {
    let reads = Router::new()
        .route("/api/v1/orders", get(list_orders))
        .route("/api/v1/orders/{id}", get(get_order))
        .route("/api/v1/returns", get(list_refunds))
        .route("/api/v1/settings/{key}", get(get_setting));

    let pos = Router::new()
        .route("/api/v1/pos/sale", post(post_sale))
        .route("/api/v1/pos/returns", post(create_refund))
        .route_layer(crate::role::layer(state.clone(), POS_ROLES));

    let writes = Router::new()
        .route("/api/v1/settings/{key}", axum::routing::put(set_setting))
        .route_layer(crate::role::layer(state, WRITE_ROLES));

    reads.merge(pos).merge(writes)
}

// --- POST /pos/sale --------------------------------------------------------

async fn post_sale(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    headers: HeaderMap,
    Json(body): Json<PosSaleRequest>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let user = user_of(&claims).ok();
    let key = idempotency_key(&headers);
    let sold_by_name = Some(claims.sub.as_str());

    match service::post_sale(
        db.as_ref(),
        &tenant,
        user.as_ref(),
        sold_by_name,
        key.as_deref(),
        body,
    )
    .await
    {
        Ok(resp) => Ok((StatusCode::CREATED, Json(resp)).into_response()),
        Err(domain::DomainError::Conflict(msg)) if msg.starts_with("IDEMPOTENCY_CACHED:") => {
            // Replay cached payload verbatim with 200 OK.
            let json = msg.trim_start_matches("IDEMPOTENCY_CACHED:");
            let value: serde_json::Value = serde_json::from_str(json)
                .map_err(|e| ApiError::internal(format!("cache decode: {e}")))?;
            Ok((StatusCode::OK, Json(value)).into_response())
        }
        Err(e) => Err(e.into()),
    }
}

// --- GET /orders -----------------------------------------------------------

async fn list_orders(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<OrderFilters>,
) -> Result<Json<Vec<OrderDto>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let rows = service::list_orders(db.as_ref(), &tenant, filters).await?;
    Ok(Json(rows))
}

#[derive(serde::Serialize)]
struct OrderDetail {
    order: OrderDto,
    items: Vec<OrderItemDto>,
}

async fn get_order(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<OrderDetail>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let (order, items) = service::get_order(db.as_ref(), &tenant, &id).await?;
    Ok(Json(OrderDetail { order, items }))
}

// --- returns / devoluciones ------------------------------------------------

async fn create_refund(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<NewDevolucion>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let user = user_of(&claims).ok();
    let resp = service::create_refund(db.as_ref(), &tenant, user.as_ref(), body).await?;
    Ok((StatusCode::CREATED, Json(resp)).into_response())
}

async fn list_refunds(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<DevolucionFilters>,
) -> Result<Json<Vec<DevolucionDto>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let rows = service::list_refunds(db.as_ref(), &tenant, filters).await?;
    Ok(Json(rows))
}

// --- admin_setting CRUD ---------------------------------------------------

async fn get_setting(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(key): Path<String>,
) -> Result<Json<AdminSettingDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    service::get_setting(db.as_ref(), &tenant, &key)
        .await?
        .map(Json)
        .ok_or_else(ApiError::not_found)
}

#[derive(serde::Deserialize)]
struct SettingValue {
    value: String,
}

async fn set_setting(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(key): Path<String>,
    Json(body): Json<SettingValue>,
) -> Result<Json<AdminSettingDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let out = service::set_setting(db.as_ref(), &tenant, &key, &body.value).await?;
    Ok(Json(out))
}

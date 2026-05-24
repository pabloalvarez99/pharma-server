//! Customers + loyalty HTTP handlers (Fase 7 subset · erp-parity).
//! Reads require a valid bearer token; mutations require role `admin`/`owner`.
//! Loyalty endpoints are read-only here — sales (Fase 4) writes the ledger.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::customers::{model::*, service};

const WRITE_ROLES: &[&str] = &["admin", "owner"];

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    let reads = Router::new()
        .route("/api/v1/clientes", get(list_customers))
        .route("/api/v1/clientes/{id}", get(get_customer))
        .route("/api/v1/loyalty", get(list_loyalty))
        .route("/api/v1/loyalty/stats", get(loyalty_stats));

    let writes = Router::new()
        .route("/api/v1/clientes", post(create_customer))
        .route(
            "/api/v1/clientes/{id}",
            axum::routing::patch(update_customer).delete(delete_customer),
        )
        .route(
            "/api/v1/admin/import-customers",
            post(import_customers_handler),
        )
        .route_layer(crate::role::layer(state, WRITE_ROLES));

    reads.merge(writes)
}

// --- customers -------------------------------------------------------------

async fn list_customers(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<CustomerFilters>,
) -> Result<Json<Vec<CustomerDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_customers(&db, &t, filters).await?))
}

async fn get_customer(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<CustomerDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_customer(&db, &t, &id).await?))
}

async fn create_customer(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewCustomer>,
) -> Result<Json<CustomerDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_customer(&db, &t, input).await?))
}

async fn update_customer(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<UpdateCustomer>,
) -> Result<Json<CustomerDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::update_customer(&db, &t, &id, patch).await?))
}

async fn delete_customer(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    service::delete_customer(&db, &t, &id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

// --- loyalty (read-only; sales Fase 4 writes the ledger) -------------------

async fn list_loyalty(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<LoyaltyFilters>,
) -> Result<Json<Vec<LoyaltyTxDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_loyalty(&db, &t, filters).await?))
}

async fn loyalty_stats(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<LoyaltyStats>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::loyalty_stats(&db, &t).await?))
}

// --- bulk import -----------------------------------------------------------
//
// `POST /api/v1/admin/import-customers` — admin/owner only (gated by the
// `WRITE_ROLES` layer). Idempotent upsert by `(tenant, email)`. The response
// is always 200 with `{ created, updated, errors: [{ idx, message }] }`
// even on partial failure — each invalid row is reported individually so a
// migration script can resume without losing the good rows.
async fn import_customers_handler(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<ImportCustomersRequest>,
) -> Result<Json<ImportCustomersResponse>, ApiError> {
    if req.customers.len() > IMPORT_CUSTOMERS_MAX_BATCH {
        return Err(ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            "BATCH_TOO_LARGE",
            format!(
                "máximo {} clientes por lote (recibidos {})",
                IMPORT_CUSTOMERS_MAX_BATCH,
                req.customers.len()
            ),
        ));
    }
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::import_customers(&db, &t, req).await?))
}

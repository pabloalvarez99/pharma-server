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

use crate::role::{admin_plus, cashier_plus};

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
        // English customer-lookup surface (POS counter): detail + history +
        // fuzzy search. Any authenticated role can read — cashiers need it.
        // `search` is a static segment so it never collides with `{id}`.
        .route("/api/v1/customers/search", get(search_customers))
        .route("/api/v1/customers/{id}", get(get_customer_detail))
        .route("/api/v1/customers/{id}/history", get(customer_history))
        .route("/api/v1/loyalty", get(list_loyalty))
        .route("/api/v1/loyalty/stats", get(loyalty_stats));

    // Customer CRUD at POS counter: granular RBAC opens this to cashier+
    // (counter staff register/update customers during a sale).
    let writes = Router::new()
        .route("/api/v1/clientes", post(create_customer))
        .route(
            "/api/v1/clientes/{id}",
            axum::routing::patch(update_customer).delete(delete_customer),
        )
        .route_layer(crate::role::layer(state.clone(), cashier_plus()));

    // Bulk admin import stays admin/owner-only — bulk customer upsert is a
    // back-office operation, NOT a counter action. Keeping the stricter gate
    // here is deliberate: do not let cashier+ mass-import via this endpoint.
    let admin_writes = Router::new()
        .route(
            "/api/v1/admin/import-customers",
            post(import_customers_handler),
        )
        .route_layer(crate::role::layer(state, admin_plus()));

    reads.merge(writes).merge(admin_writes)
}

// --- customers -------------------------------------------------------------

/// Lista clientes del tenant.
#[utoipa::path(get, path = "/api/v1/clientes", tag = "Customers",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_customers(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<CustomerFilters>,
) -> Result<Json<Vec<CustomerDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_customers(&db, &t, filters).await?))
}

/// Detalle de cliente.
#[utoipa::path(get, path = "/api/v1/clientes/{id}", tag = "Customers",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn get_customer(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<CustomerDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_customer(&db, &t, &id).await?))
}

async fn get_customer_detail(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<CustomerDetailDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_customer_detail(&db, &t, &id).await?))
}

async fn customer_history(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(filters): Query<CustomerHistoryFilters>,
) -> Result<Json<Vec<CustomerOrderDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(
        service::customer_history(&db, &t, &id, filters).await?,
    ))
}

async fn search_customers(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(query): Query<CustomerSearchQuery>,
) -> Result<Json<Vec<CustomerDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::search_customers(&db, &t, query).await?))
}

/// Crea cliente. Requiere cashier+.
#[utoipa::path(post, path = "/api/v1/clientes", tag = "Customers",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn create_customer(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewCustomer>,
) -> Result<Json<CustomerDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_customer(&db, &t, input).await?))
}

/// Actualiza cliente. Requiere cashier+.
#[utoipa::path(patch, path = "/api/v1/clientes/{id}", tag = "Customers",
    params(("id" = String, Path)), request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn update_customer(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<UpdateCustomer>,
) -> Result<Json<CustomerDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::update_customer(&db, &t, &id, patch).await?))
}

/// Elimina (soft) cliente. Requiere cashier+.
#[utoipa::path(delete, path = "/api/v1/clientes/{id}", tag = "Customers",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn delete_customer(
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

/// Lista del ledger de loyalty (sólo lectura).
#[utoipa::path(get, path = "/api/v1/loyalty", tag = "Customers",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_loyalty(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<LoyaltyFilters>,
) -> Result<Json<Vec<LoyaltyTxDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_loyalty(&db, &t, filters).await?))
}

/// Estadísticas de loyalty.
#[utoipa::path(get, path = "/api/v1/loyalty/stats", tag = "Customers",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn loyalty_stats(
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

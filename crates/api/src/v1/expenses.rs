//! Expenses + reports (sales-daily, margins-daily, top-products,
//! stock-rotation, near-expiry).
//!
//! Roles:
//! * Expenses list/create — `cashier+` (counter staff records petty cash).
//! * Reports — JWT only (no extra role). Some are license-gated:
//!   - `margins-daily` requires `reports.margins_daily` feature (402 if not
//!     entitled; Pro+ or microtx). Free tier sees `sales-daily` only.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::expenses::{model::*, service};

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
    // Reports = JWT only (no extra role); `margins-daily` adds a license gate.
    let reports = Router::new()
        .route("/api/v1/reports/sales-daily", get(sales_daily))
        .route("/api/v1/reports/margins-daily", get(margins_daily))
        .route("/api/v1/reports/top-products", get(top_products))
        .route("/api/v1/reports/stock-rotation", get(stock_rotation))
        .route("/api/v1/reports/near-expiry", get(near_expiry));

    // Expenses list + create both require cashier+ (counter staff records
    // petty cash; nobody outside ladder should read/write it).
    let expenses = Router::new()
        .route("/api/v1/expenses", get(list_expenses).post(create_expense))
        .route_layer(crate::role::layer(state, cashier_plus()));

    reports.merge(expenses)
}

// --- expenses --------------------------------------------------------------

/// Crea un gasto (egreso de caja, no relacionado a una venta). Requiere cashier+.
#[utoipa::path(post, path = "/api/v1/expenses", tag = "Expenses",
    request_body = serde_json::Value,
    responses(
        (status = 201, description = "Gasto creado", body = serde_json::Value),
        (status = 400, description = "Datos inválidos", body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Rol insuficiente (requiere cashier+)", body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn create_expense(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<NewExpense>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let by = user_of(&claims).ok();
    let e = service::create_expense(db.as_ref(), &tenant, by.as_ref(), body).await?;
    Ok((StatusCode::CREATED, Json(e)).into_response())
}

/// Lista gastos del tenant. Filtrable por rango de fecha + categoría.
#[utoipa::path(get, path = "/api/v1/expenses", tag = "Expenses",
    responses(
        (status = 200, description = "Lista de gastos", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn list_expenses(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<ExpenseFilters>,
) -> Result<Json<Vec<ExpenseDto>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(
        service::list_expenses(db.as_ref(), &tenant, filters).await?,
    ))
}

// --- reports ---------------------------------------------------------------

/// Reporte de ventas diarias (Free tier OK). Filtrable por rango de fecha.
#[utoipa::path(get, path = "/api/v1/reports/sales-daily", tag = "Expenses",
    responses(
        (status = 200, description = "Ventas agrupadas por día", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn sales_daily(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<SalesReportFilters>,
) -> Result<Json<Vec<DailySalesRow>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(
        service::sales_daily(db.as_ref(), &tenant, filters).await?,
    ))
}

/// Reporte de márgenes diarios. **Requiere Pro+** o microtx
/// `reports.margins_daily` — Free retorna 402 `FEATURE_REQUIRES_UPGRADE`.
#[utoipa::path(get, path = "/api/v1/reports/margins-daily", tag = "Expenses",
    responses(
        (status = 200, description = "Márgenes agrupados por día", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 402, description = "Tier license insuficiente (FEATURE_REQUIRES_UPGRADE)", body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn margins_daily(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<SalesReportFilters>,
) -> Result<Json<Vec<DailyMarginRow>>, ApiError> {
    // License gate (Fase 10d POC): requires Pro+ or microtx that grants
    // `reports.margins_daily`. Free tier → 402 FEATURE_REQUIRES_UPGRADE.
    let lic = state.license.load();
    license::require(&lic, "reports.margins_daily")?;
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(
        service::margins_daily(db.as_ref(), &tenant, filters).await?,
    ))
}

/// Top productos por ventas (qty/revenue).
#[utoipa::path(get, path = "/api/v1/reports/top-products", tag = "Expenses",
    responses(
        (status = 200, description = "Lista de productos top por ventas", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn top_products(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<TopProductsFilters>,
) -> Result<Json<Vec<TopProductRow>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(
        service::top_products(db.as_ref(), &tenant, filters).await?,
    ))
}

/// Reporte de rotación de stock (ventas / inventario promedio).
#[utoipa::path(get, path = "/api/v1/reports/stock-rotation", tag = "Expenses",
    responses(
        (status = 200, description = "Rotación de stock por producto", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn stock_rotation(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<SalesReportFilters>,
) -> Result<Json<Vec<StockRotationRow>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(
        service::stock_rotation(db.as_ref(), &tenant, filters).await?,
    ))
}

/// Lotes próximos a vencer (ventana configurable en días), opcionalmente
/// acotados a una sucursal.
#[utoipa::path(get, path = "/api/v1/reports/near-expiry", tag = "Expenses",
    params(
        ("days"   = Option<i64>,    Query, description = "Ventana en días (default 30)"),
        ("branch" = Option<String>, Query, description = "Sucursal del lote: ausente = todos los locales, `none` = casa matriz, `branch:<key>` = ese local"),
    ),
    responses(
        (status = 200, description = "Lotes próximos a vencer", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn near_expiry(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<NearExpiryFilters>,
) -> Result<Json<Vec<NearExpiryRow>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(
        service::near_expiry(db.as_ref(), &tenant, filters).await?,
    ))
}

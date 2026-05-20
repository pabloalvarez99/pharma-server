//! Expenses + daily-sales report.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::expenses::{model::*, service};

const WRITE_ROLES: &[&str] = &["admin", "owner"];

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
        .route("/api/v1/expenses", get(list_expenses))
        .route("/api/v1/reports/sales-daily", get(sales_daily))
        .route("/api/v1/reports/margins-daily", get(margins_daily))
        .route("/api/v1/reports/top-products", get(top_products))
        .route("/api/v1/reports/stock-rotation", get(stock_rotation))
        .route("/api/v1/reports/near-expiry", get(near_expiry));

    let writes = Router::new()
        .route("/api/v1/expenses", post(create_expense))
        .route_layer(crate::role::layer(state, WRITE_ROLES));

    reads.merge(writes)
}

async fn create_expense(
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

async fn list_expenses(
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

async fn sales_daily(
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

async fn margins_daily(
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

async fn top_products(
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

async fn stock_rotation(
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

async fn near_expiry(
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

//! Paginated stock-movement audit-trail query.
//!
//! `GET /api/v1/stock-movements` — cualquier rol cajero o superior puede
//! consultar el historial de movimientos de stock del tenant.  La respuesta
//! incluye metadatos de paginación (`total`, `page`, `limit`) para que los
//! clientes naveguen registros sin cargar todo el historial.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::role::cashier_plus;
use crate::AppState;

use domain::inventory::model::StockMovementDto;
use domain::inventory::service;

// ---------------------------------------------------------------------------
// Helpers (mirrors inventory.rs pattern)
// ---------------------------------------------------------------------------

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

/// Parámetros de consulta para `GET /api/v1/stock-movements`.
#[derive(Debug, Deserialize)]
pub struct StockMovementsQuery {
    /// Filtrar por product id (`product:xxx`).
    #[serde(default)]
    pub product_id: Option<String>,
    /// Fecha inicio (inclusive), formato `YYYY-MM-DD`.
    #[serde(default)]
    pub from: Option<NaiveDate>,
    /// Fecha fin (inclusive), formato `YYYY-MM-DD`.
    #[serde(default)]
    pub to: Option<NaiveDate>,
    /// Número de página (1-based, default 1).
    #[serde(default = "default_page")]
    pub page: u64,
    /// Registros por página (default 50, máx 200).
    #[serde(default = "default_limit")]
    pub limit: u64,
}

fn default_page() -> u64 {
    1
}

fn default_limit() -> u64 {
    50
}

// ---------------------------------------------------------------------------
// Response shape
// ---------------------------------------------------------------------------

/// Respuesta paginada de movimientos de stock.
#[derive(Debug, Serialize)]
pub struct StockMovementsPage {
    pub data: Vec<StockMovementDto>,
    pub total: u64,
    pub page: u64,
    pub limit: u64,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/stock-movements", get(list_movements_paginated))
        .route_layer(crate::role::layer(state, cashier_plus()))
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Lista movimientos de stock paginados del tenant.
///
/// Accesible por cualquier rol cajero o superior.  Los parámetros opcionales
/// `product_id`, `from`, y `to` filtran la consulta; `page` y `limit`
/// controlan la paginación.
#[utoipa::path(
    get,
    path = "/api/v1/stock-movements",
    tag = "Inventory",
    params(
        ("product_id" = Option<String>, Query, description = "Filtrar por product id"),
        ("from" = Option<String>, Query, description = "Fecha inicio YYYY-MM-DD"),
        ("to"   = Option<String>, Query, description = "Fecha fin YYYY-MM-DD"),
        ("page"  = Option<u64>,   Query, description = "Página (1-based, default 1)"),
        ("limit" = Option<u64>,   Query, description = "Registros por página (default 50, máx 200)"),
    ),
    responses(
        (status = 200, body = serde_json::Value,
         description = "{ data: [...], total, page, limit }"),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = []))
)]
pub async fn list_movements_paginated(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<StockMovementsQuery>,
) -> Result<Json<StockMovementsPage>, ApiError> {
    let db = db_of(&s)?;
    let tenant = tenant_of(&claims)?;

    // Clamp limit to [1, 200].
    let limit = q.limit.clamp(1, 200);
    let page = q.page.max(1);
    let offset = (page - 1).saturating_mul(limit);

    // Convert NaiveDate → DateTime<Utc> (start / end of day).
    let from_dt: Option<DateTime<Utc>> = q.from.map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
    let to_dt: Option<DateTime<Utc>> = q.to.map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());

    let filters = domain::inventory::model::MovementFilters {
        product: q.product_id.clone(),
        reason: None,
        from: from_dt,
        to: to_dt,
        limit: Some(u32::try_from(limit).unwrap_or(u32::MAX)),
        offset: Some(u32::try_from(offset).unwrap_or(u32::MAX)),
    };

    // Page data.
    let data = service::list_movements(&db, &tenant, filters.clone()).await?;

    // Count query for total (same filters, no pagination).
    let count_filters = domain::inventory::model::MovementFilters {
        limit: Some(u32::MAX),
        offset: Some(0),
        ..filters
    };
    let all = service::list_movements(&db, &tenant, count_filters).await?;
    let total = all.len() as u64;

    Ok(Json(StockMovementsPage {
        data,
        total,
        page,
        limit,
    }))
}

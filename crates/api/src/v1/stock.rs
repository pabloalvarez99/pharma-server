//! Stock por sucursal + transferencias — business-depth V2 (HTTP).
//!
//! `GET  /api/v1/stock/sucursales`        — on-hand por (producto, sucursal).
//!   Rol cajero+ : el POS y el dashboard leen el stock del local para saber si
//!   pueden vender.
//! `GET  /api/v1/stock/sucursales/reporte` — una fila por producto con el
//!   desglose por local + total (el total es `product.stock`, invariante V2).
//! `POST /api/v1/stock/transferencias`     — mueve stock entre dos locales,
//!   atómico. Requiere admin+ : mover inventario entre sucursales es
//!   back-office, no una acción de mostrador.
//!
//! Ver `domain::stock` y la migración 0041.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::role::{admin_plus, cashier_plus};
use crate::AppState;

use domain::stock::{model::*, service};

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    let reads = Router::new()
        .route("/api/v1/stock/sucursales", get(list_branch_stock))
        .route("/api/v1/stock/sucursales/reporte", get(branch_stock_report))
        .route_layer(crate::role::layer(state.clone(), cashier_plus()));

    let writes = Router::new()
        .route("/api/v1/stock/transferencias", post(transfer))
        .route_layer(crate::role::layer(state, admin_plus()));

    reads.merge(writes)
}

/// Stock on-hand por (producto, sucursal). `?product=product:xxx` filtra un
/// producto; `?branch=branch:xxx` una sucursal (`?branch=none` = casa matriz);
/// `?non_zero=true` esconde las filas en cero.
#[utoipa::path(get, path = "/api/v1/stock/sucursales", tag = "Inventory",
    params(
        ("product"  = Option<String>, Query, description = "Filtrar por producto"),
        ("branch"   = Option<String>, Query, description = "Filtrar por sucursal (`none` = casa matriz)"),
        ("non_zero" = Option<bool>,   Query, description = "Sólo filas con stock distinto de cero"),
    ),
    responses(
        (status = 200, body = serde_json::Value, description = "Stock on-hand por (producto, sucursal)"),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_branch_stock(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<BranchStockFilters>,
) -> Result<Json<Vec<BranchStockDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_branch_stock(&db, &t, filters).await?))
}

/// Reporte de existencias por sucursal: una fila por producto con el desglose
/// por local y el total. Mismos filtros que el listado.
#[utoipa::path(get, path = "/api/v1/stock/sucursales/reporte", tag = "Inventory",
    params(
        ("product"  = Option<String>, Query, description = "Filtrar por producto"),
        ("branch"   = Option<String>, Query, description = "Filtrar por sucursal (`none` = casa matriz)"),
        ("non_zero" = Option<bool>,   Query, description = "Sólo filas con stock distinto de cero"),
    ),
    responses(
        (status = 200, body = serde_json::Value, description = "Producto → desglose por sucursal + total"),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn branch_stock_report(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<BranchStockFilters>,
) -> Result<Json<Vec<BranchStockReportRow>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::branch_stock_report(&db, &t, filters).await?))
}

/// Transfiere stock de un producto entre dos sucursales del tenant. Atómico:
/// emite los dos movimientos de auditoría (salida + entrada) en una sola
/// transacción y no cambia el stock total del negocio, sólo su distribución.
/// Rechaza si el origen no tiene suficiente. Requiere admin+.
#[utoipa::path(post, path = "/api/v1/stock/transferencias", tag = "Inventory",
    request_body = serde_json::Value,
    responses(
        (status = 200, body = serde_json::Value, description = "Transferencia aplicada (saldos resultantes + ids de movimiento)"),
        (status = 400, body = crate::error::ErrorEnvelope, description = "Input inválido / sucursal no encontrada / servicio sin stock"),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope, description = "Producto no encontrado"),
        (status = 422, body = crate::error::ErrorEnvelope, description = "Stock insuficiente en el origen")),
    security(("bearer_jwt" = [])))]
pub async fn transfer(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<TransferInput>,
) -> Result<Json<TransferResult>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(
        service::transfer(&db, &t, Some(&claims.sub), input).await?,
    ))
}

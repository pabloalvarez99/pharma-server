//! Compliance CL (V3): libro de compras + resumen IVA (F29) + captura de la
//! factura del proveedor sobre una OC.
//!
//! Lecturas admin+ (son cifras tributarias del negocio); capturar la factura es
//! admin+ también (afecta el libro).

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{get, patch},
    Json, Router,
};
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::role::admin_plus;
use crate::AppState;

use domain::compliance::{model::*, repo};

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

/// `?period=YYYY-MM`. Sin él se asume el mes en curso (lo que el dueño quiere
/// ver al abrir la vista).
#[derive(Debug, Deserialize)]
pub struct PeriodQuery {
    #[serde(default)]
    period: Option<String>,
}

fn period_or_current(q: &PeriodQuery) -> String {
    q.period
        .clone()
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m").to_string())
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/reports/libro-compras", get(libro_compras))
        .route("/api/v1/reports/iva", get(iva))
        .route("/api/v1/purchase-orders/{id}/factura", patch(set_factura))
        .route_layer(crate::role::layer(state, admin_plus()))
}

/// GET `/api/v1/reports/libro-compras?period=YYYY-MM` (admin+) — libro de
/// compras del período: una fila por documento de proveedor + totales.
async fn libro_compras(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<PeriodQuery>,
) -> Result<Json<PurchaseBook>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(
        repo::purchase_book(&db, &t, &period_or_current(&q)).await?,
    ))
}

/// GET `/api/v1/reports/iva?period=YYYY-MM` (admin+) — débito (ventas) −
/// crédito (compras) = IVA a pagar del período.
async fn iva(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<PeriodQuery>,
) -> Result<Json<IvaSummary>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(
        repo::iva_summary(&db, &t, &period_or_current(&q)).await?,
    ))
}

/// PATCH `/api/v1/purchase-orders/{id}/factura` (admin+) — capturar folio,
/// fecha, tipo y montos de la factura del proveedor. Sólo se escribe lo enviado.
async fn set_factura(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<InvoiceInput>,
) -> Result<Json<PurchaseBook>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let po = surrealdb::sql::thing(&id)
        .map_err(|_| ApiError::invalid("id de orden de compra inválido"))?;
    if po.tb != "purchase_order" {
        return Err(ApiError::invalid(
            "el id no corresponde a una orden de compra",
        ));
    }
    repo::set_invoice(&db, &t, &po, &body).await?;
    // Devolvemos el libro del período de la factura (o el actual) para que la
    // vista refresque sin una segunda llamada.
    let period = body
        .date
        .map(|d| d.format("%Y-%m").to_string())
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m").to_string());
    Ok(Json(repo::purchase_book(&db, &t, &period).await?))
}

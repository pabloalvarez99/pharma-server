//! Cuenta corriente / fiado (V1) — estado de cuenta + abonos.
//!
//! El CARGO de una venta fiada lo genera el sale path (`payment_method =
//! pos_fiado`). Acá exponemos leer la cuenta y registrar abonos. cashier+ (el
//! cajero cobra el fiado en el mostrador).

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::role::cashier_plus;
use crate::AppState;

use domain::credit::{model::*, service};

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn user_of(claims: &auth::Claims) -> Option<Thing> {
    surrealdb::sql::thing(&claims.sub).ok()
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

/// Parse a `customer:xxx` path id, rejecting anything that isn't a customer.
fn customer_thing(id: &str) -> Result<Thing, ApiError> {
    let t = surrealdb::sql::thing(id).map_err(|_| ApiError::invalid("id de cliente inválido"))?;
    if t.tb != "customer" {
        return Err(ApiError::invalid("el id no corresponde a un cliente"));
    }
    Ok(t)
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/customers/{id}/cuenta", get(get_account))
        .route("/api/v1/customers/{id}/abono", post(post_abono))
        .route_layer(crate::role::layer(state, cashier_plus()))
}

/// GET `/api/v1/customers/{id}/cuenta` (cashier+) — estado de cuenta corriente:
/// saldo, total fiado, total abonado + movimientos.
async fn get_account(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<CustomerAccountDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let c = customer_thing(&id)?;
    Ok(Json(service::account(&db, &t, &c).await?))
}

/// POST `/api/v1/customers/{id}/abono` (cashier+) — registrar un pago del cliente
/// contra su deuda. Valida monto > 0 y que no supere la deuda. `cash_session`
/// opcional (pago en efectivo con caja abierta → entra al arqueo).
async fn post_abono(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<NewAbono>,
) -> Result<Json<LedgerEntryDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let c = customer_thing(&id)?;
    let by = user_of(&claims);

    let cash_session = match body.cash_session.as_deref() {
        Some(cs) if !cs.is_empty() => {
            Some(surrealdb::sql::thing(cs).map_err(|_| ApiError::invalid("id de caja inválido"))?)
        }
        _ => None,
    };

    let entry = service::record_abono(
        &db,
        &t,
        &c,
        body.amount,
        cash_session.as_ref(),
        body.note.as_deref(),
        by.as_ref(),
    )
    .await?;
    Ok(Json(entry))
}

//! Prescriptions + pharmacist-shift HTTP handlers (Fase 7 subset · erp-parity).
//!
//! Prescription rows are immutable (Ley 20.000): only `POST` / `GET` are
//! exposed; there is no `PATCH` or `DELETE` route by design. Controlled
//! records are listed (and CSV-exported) under `/libro-recetas`.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::prescriptions::{model::*, service};

/// Prescriptions creation requires a pharmacist (or higher). Shifts are
/// administrative.
use crate::role::{admin_plus, pharmacist_plus};

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    let reads = Router::new()
        .route("/api/v1/prescriptions", get(list_prescriptions))
        .route("/api/v1/prescriptions/{id}", get(get_prescription))
        .route("/api/v1/libro-recetas", get(list_libro))
        .route("/api/v1/libro-recetas/export", get(export_libro))
        .route("/api/v1/turnos-farmaceutico", get(list_shifts));

    let prescription_writes = Router::new()
        .route("/api/v1/prescriptions", post(create_prescription))
        .route_layer(crate::role::layer(state.clone(), pharmacist_plus()));

    let shift_writes = Router::new()
        .route("/api/v1/turnos-farmaceutico", post(create_shift))
        .route(
            "/api/v1/turnos-farmaceutico/{id}",
            axum::routing::patch(update_shift),
        )
        .route_layer(crate::role::layer(state, admin_plus()));

    reads.merge(prescription_writes).merge(shift_writes)
}

// --- prescriptions ---------------------------------------------------------

/// Lista recetas (todas, no sólo controladas).
#[utoipa::path(get, path = "/api/v1/prescriptions", tag = "Prescriptions",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_prescriptions(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<PrescriptionFilters>,
) -> Result<Json<Vec<PrescriptionDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_prescriptions(&db, &t, filters).await?))
}

/// Detalle de receta.
#[utoipa::path(get, path = "/api/v1/prescriptions/{id}", tag = "Prescriptions",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn get_prescription(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<PrescriptionDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_prescription(&db, &t, &id).await?))
}

/// Crea una receta (Ley 20.000, immutable). Requiere pharmacist+.
#[utoipa::path(post, path = "/api/v1/prescriptions", tag = "Prescriptions",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn create_prescription(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewPrescription>,
) -> Result<Json<PrescriptionDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_prescription(&db, &t, input).await?))
}

/// Libro de controlados (sólo Ley 20.000).
#[utoipa::path(get, path = "/api/v1/libro-recetas", tag = "Prescriptions",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_libro(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<PrescriptionFilters>,
) -> Result<Json<Vec<PrescriptionDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_controlled(&db, &t, filters).await?))
}

/// CSV del libro de controlados.
#[utoipa::path(get, path = "/api/v1/libro-recetas/export", tag = "Prescriptions",
    responses(
        (status = 200, description = "CSV libro recetas", content_type = "text/csv"),
        (status = 401, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn export_libro(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<PrescriptionFilters>,
) -> Result<Response, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let mut f = filters;
    f.limit = Some(f.limit.unwrap_or(500).min(500));
    let rows = service::list_controlled(&db, &t, f).await?;
    let mut w = csv::Writer::from_writer(vec![]);
    w.write_record([
        "id",
        "dispensed_at",
        "patient_name",
        "patient_rut",
        "doctor_name",
        "doctor_rut",
        "folio",
        "product",
        "customer",
    ])
    .map_err(|e| ApiError::internal(e.to_string()))?;
    for p in rows {
        w.write_record([
            p.id,
            p.dispensed_at.to_rfc3339(),
            p.patient_name,
            p.patient_rut,
            p.doctor_name.unwrap_or_default(),
            p.doctor_rut.unwrap_or_default(),
            p.folio.unwrap_or_default(),
            p.product.unwrap_or_default(),
            p.customer.unwrap_or_default(),
        ])
        .map_err(|e| ApiError::internal(e.to_string()))?;
    }
    let body = w
        .into_inner()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"libro-recetas.csv\"",
            ),
        ],
        body,
    )
        .into_response())
}

// --- pharmacist shifts -----------------------------------------------------

/// Lista turnos del químico farmacéutico.
#[utoipa::path(get, path = "/api/v1/turnos-farmaceutico", tag = "Prescriptions",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_shifts(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<ShiftFilters>,
) -> Result<Json<Vec<PharmacistShiftDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_shifts(&db, &t, filters).await?))
}

/// Crea turno del químico. Requiere admin+.
#[utoipa::path(post, path = "/api/v1/turnos-farmaceutico", tag = "Prescriptions",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn create_shift(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewPharmacistShift>,
) -> Result<Json<PharmacistShiftDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_shift(&db, &t, input).await?))
}

/// Actualiza turno del químico. Requiere admin+.
#[utoipa::path(patch, path = "/api/v1/turnos-farmaceutico/{id}", tag = "Prescriptions",
    params(("id" = String, Path)), request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn update_shift(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<UpdatePharmacistShift>,
) -> Result<Json<PharmacistShiftDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::update_shift(&db, &t, &id, patch).await?))
}

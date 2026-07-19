//! Inventory HTTP handlers (Fase 3). Reads require a valid bearer token;
//! mutations additionally require role `admin`/`owner` (RequireRole layer).
//!
//! Mirrors the `catalog` orchestration shape: thin wrappers over
//! `domain::inventory::service`, tenant pulled from the JWT.

use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, Query, State},
    routing::{get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::inventory::{model::*, service};

use crate::role::admin_plus;

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    let reads = Router::new()
        // GET /api/v1/stock-movements (paginated, cashier_plus gate) is owned
        // by v1::stock_movements; OpenAPI documents it there too.
        .route("/api/v1/batches", get(list_batches))
        .route("/api/v1/batches/{id}", get(get_batch))
        .route("/api/v1/faltas", get(list_faltas))
        .route("/api/v1/inventory", get(summary))
        .route("/api/v1/inventory/abc", get(abc))
        .route(
            "/api/v1/inventory/reorder-suggestions",
            get(reorder_suggestions),
        );

    let writes = Router::new()
        .route("/api/v1/stock-movements", post(create_movement))
        .route("/api/v1/stock-movements/adjust", post(adjust_movement))
        .route("/api/v1/stock-movements/import", post(import_movements))
        .route("/api/v1/batches", post(create_batch))
        .route(
            "/api/v1/batches/{id}",
            patch(update_batch_h).delete(delete_batch_h),
        )
        .route("/api/v1/faltas", post(create_falta_h))
        .route("/api/v1/faltas/{id}", patch(update_falta_h))
        .route_layer(crate::role::layer(state, admin_plus()));

    reads.merge(writes)
}

// --- stock movements -------------------------------------------------------

#[derive(Serialize)]
pub(crate) struct MovementResponse {
    movement: StockMovementDto,
    product: domain::catalog::model::ProductDto,
}

/// Crea un movimiento de stock (entrada/salida/ajuste). Requiere admin+.
#[utoipa::path(post, path = "/api/v1/stock-movements", tag = "Inventory",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 422, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn create_movement(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewMovement>,
) -> Result<Json<MovementResponse>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let (movement, product) = service::add_movement(
        &db,
        &t,
        &input.product,
        input.delta,
        &input.reason,
        Some(&claims.sub),
        input.r#ref,
    )
    .await?;
    Ok(Json(MovementResponse { movement, product }))
}

/// Ajuste manual de stock con auditoría. Requiere admin+.
#[utoipa::path(post, path = "/api/v1/stock-movements/adjust", tag = "Inventory",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn adjust_movement(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(adj): Json<AdjustMovement>,
) -> Result<Json<MovementResponse>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let (movement, product) = service::adjust(&db, &t, adj, Some(&claims.sub)).await?;
    // ERP→web stock push (ADR-0013 trigger `manual.adjust`): fire-and-forget.
    crate::stock_webhook::notify_products(&s, t.clone(), vec![movement.product.clone()]);
    Ok(Json(MovementResponse { movement, product }))
}

#[derive(Serialize)]
pub(crate) struct ImportSummary {
    created: usize,
    failed: usize,
    errors: Vec<ImportError>,
}

#[derive(Serialize)]
pub(crate) struct ImportError {
    line: usize,
    message: String,
}

/// CSV columns (case-insensitive): `product` (id), `delta`, `reason`,
/// optional `ref`. Each row is its own movement (and tx).
/// Import CSV de movimientos de stock (multipart). Requiere admin+.
#[utoipa::path(post, path = "/api/v1/stock-movements/import", tag = "Inventory",
    request_body(content = String, content_type = "multipart/form-data"),
    responses((status = 200, body = serde_json::Value), (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope), (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn import_movements(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    mut mp: Multipart,
) -> Result<Json<ImportSummary>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let admin = claims.sub.clone();

    let field = mp
        .next_field()
        .await
        .map_err(|e| ApiError::invalid(format!("multipart inválido: {e}")))?
        .ok_or_else(|| ApiError::invalid("falta el archivo CSV"))?;
    let bytes = field
        .bytes()
        .await
        .map_err(|e| ApiError::invalid(format!("lectura de archivo falló: {e}")))?
        .to_vec();

    let mut rdr = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(bytes.as_slice());
    let headers = rdr
        .headers()
        .map_err(|e| ApiError::invalid(format!("CSV sin cabecera válida: {e}")))?
        .clone();
    let col = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));
    let (Some(i_product), Some(i_delta), Some(i_reason)) =
        (col("product"), col("delta"), col("reason"))
    else {
        return Err(ApiError::invalid(
            "CSV debe incluir columnas `product`, `delta`, `reason`",
        ));
    };
    let i_ref = col("ref");
    let get = |rec: &csv::StringRecord, idx: Option<usize>| {
        idx.and_then(|i| rec.get(i))
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };

    let mut summary = ImportSummary {
        created: 0,
        failed: 0,
        errors: Vec::new(),
    };
    for (n, rec) in rdr.records().enumerate() {
        let line = n + 2;
        let rec = match rec {
            Ok(r) => r,
            Err(e) => {
                summary.failed += 1;
                summary.errors.push(ImportError {
                    line,
                    message: e.to_string(),
                });
                continue;
            }
        };
        let product = rec.get(i_product).unwrap_or("").trim().to_string();
        let delta: i64 = match rec.get(i_delta).unwrap_or("").trim().parse() {
            Ok(d) => d,
            Err(_) => {
                summary.failed += 1;
                summary.errors.push(ImportError {
                    line,
                    message: "delta inválido".into(),
                });
                continue;
            }
        };
        let reason = rec.get(i_reason).unwrap_or("").trim().to_string();
        let r#ref = get(&rec, i_ref);
        match service::add_movement(&db, &t, &product, delta, &reason, Some(&admin), r#ref).await {
            Ok(_) => summary.created += 1,
            Err(e) => {
                summary.failed += 1;
                summary.errors.push(ImportError {
                    line,
                    message: e.to_string(),
                });
            }
        }
    }
    Ok(Json(summary))
}

// --- batches ---------------------------------------------------------------

#[derive(Deserialize, Default)]
pub(crate) struct BatchQuery {
    #[serde(default)]
    product: Option<String>,
    #[serde(default)]
    only_available: Option<bool>,
    #[serde(default)]
    expiring_within_days: Option<i64>,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    offset: Option<u32>,
}

impl From<BatchQuery> for BatchFilters {
    fn from(q: BatchQuery) -> Self {
        Self {
            product: q.product,
            only_available: q.only_available,
            expiring_within_days: q.expiring_within_days,
            limit: q.limit,
            offset: q.offset,
        }
    }
}

/// Lista lotes (batches).
#[utoipa::path(get, path = "/api/v1/batches", tag = "Inventory",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_batches(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<BatchQuery>,
) -> Result<Json<Vec<BatchDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_batches(&db, &t, q.into()).await?))
}

/// Detalle de un lote.
#[utoipa::path(get, path = "/api/v1/batches/{id}", tag = "Inventory",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn get_batch(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<BatchDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_batch(&db, &t, &id).await?))
}

/// Crea un lote. Requiere admin+.
#[utoipa::path(post, path = "/api/v1/batches", tag = "Inventory",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn create_batch(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewBatch>,
) -> Result<Json<BatchDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(
        service::create_batch(&db, &t, input, Some(&claims.sub)).await?,
    ))
}

/// Actualiza un lote (patch). Requiere admin+.
#[utoipa::path(patch, path = "/api/v1/batches/{id}", tag = "Inventory",
    params(("id" = String, Path)), request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn update_batch_h(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<UpdateBatch>,
) -> Result<Json<BatchDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::update_batch(&db, &t, &id, patch).await?))
}

/// Elimina (soft) un lote. Requiere admin+.
#[utoipa::path(delete, path = "/api/v1/batches/{id}", tag = "Inventory",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn delete_batch_h(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    service::delete_batch(&db, &t, &id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

// --- faltas ----------------------------------------------------------------

#[derive(Deserialize, Default)]
pub(crate) struct FaltaQuery {
    #[serde(default)]
    resolved: Option<bool>,
    #[serde(default)]
    product: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    offset: Option<u32>,
}

impl From<FaltaQuery> for FaltaFilters {
    fn from(q: FaltaQuery) -> Self {
        Self {
            resolved: q.resolved,
            product: q.product,
            limit: q.limit,
            offset: q.offset,
        }
    }
}

/// Lista faltas (out-of-stock requests).
#[utoipa::path(get, path = "/api/v1/faltas", tag = "Inventory",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_faltas(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<FaltaQuery>,
) -> Result<Json<Vec<FaltaDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_faltas(&db, &t, q.into()).await?))
}

/// Crea una falta. Requiere admin+.
#[utoipa::path(post, path = "/api/v1/faltas", tag = "Inventory",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn create_falta_h(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewFalta>,
) -> Result<Json<FaltaDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_falta(&db, &t, input).await?))
}

/// Actualiza una falta. Requiere admin+.
#[utoipa::path(patch, path = "/api/v1/faltas/{id}", tag = "Inventory",
    params(("id" = String, Path)), request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn update_falta_h(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<UpdateFalta>,
) -> Result<Json<FaltaDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::update_falta(&db, &t, &id, patch).await?))
}

// --- summary / abc / reorder ----------------------------------------------

/// Resumen de inventario (totales, valor, etc.).
#[utoipa::path(get, path = "/api/v1/inventory", tag = "Inventory",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn summary(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<InventorySummary>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::summary(&db, &t).await?))
}

/// Reporte ABC (clasificación rotación).
#[utoipa::path(get, path = "/api/v1/inventory/abc", tag = "Inventory",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn abc(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<AbcReport>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::abc(&db, &t).await?))
}

/// Sugerencias de reorden (stock < mínimo).
#[utoipa::path(get, path = "/api/v1/inventory/reorder-suggestions", tag = "Inventory",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn reorder_suggestions(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<ReorderReport>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::reorder_suggestions(&db, &t).await?))
}

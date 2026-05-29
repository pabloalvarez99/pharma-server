//! Sales HTTP handlers (Fase 4). POS sale, orders read, admin_setting CRUD.
//! Reads = bearer; mutations = role `admin`/`owner`. POS sale = role
//! `admin`/`owner`/`cashier` (cashier rol introduced here for counter staff).
//!
//! Idempotency: `POST /pos/sale` honors the `Idempotency-Key` header. The
//! domain layer signals a cache hit via `DomainError::Conflict("IDEMPOTENCY_CACHED:<json>")`;
//! we intercept that and reply with the cached payload + 200 instead of 409.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::sales::{historic, model::*, service};

use crate::role::{admin_plus, cashier_plus};

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn user_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.sub).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

fn idempotency_key(h: &HeaderMap) -> Option<String> {
    h.get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

pub fn router(state: AppState) -> Router<AppState> {
    let reads = Router::new()
        .route("/api/v1/orders", get(list_orders))
        .route("/api/v1/orders/{id}", get(get_order))
        .route("/api/v1/orders/{id}/receipt", get(get_receipt))
        .route("/api/v1/returns", get(list_refunds))
        .route("/api/v1/interactions/check", post(check_interactions))
        .route("/api/v1/settings/{key}", get(get_setting));

    let pos = Router::new()
        .route("/api/v1/pos/sale", post(post_sale))
        .route("/api/v1/pos/returns", post(create_refund))
        .route_layer(crate::role::layer(state.clone(), cashier_plus()));

    // admin_plus() == &["admin","owner"] (the old WRITE_ROLES). Settings
    // mutation and historic-order bulk import are back-office only.
    let writes = Router::new()
        .route("/api/v1/settings/{key}", axum::routing::put(set_setting))
        .route(
            "/api/v1/admin/import-historic-orders",
            post(import_historic_orders),
        )
        .route_layer(crate::role::layer(state, admin_plus()));

    reads.merge(pos).merge(writes)
}

// --- POST /pos/sale --------------------------------------------------------

/// Vender en POS. Crea pedido + movimientos + descuento de stock + asiento
/// de caja en una sola tx. Honra `Idempotency-Key`.
#[utoipa::path(
    post,
    path = "/api/v1/pos/sale",
    tag = "Sales",
    request_body = serde_json::Value,
    responses(
        (status = 201, description = "Venta creada", body = serde_json::Value),
        (status = 200, description = "Idempotency-Key replay; payload cacheado", body = serde_json::Value),
        (status = 400, description = "Datos inválidos", body = crate::error::ErrorEnvelope),
        (status = 401, description = "Sin token o inválido", body = crate::error::ErrorEnvelope),
        (status = 403, description = "Rol insuficiente (requiere cashier+)", body = crate::error::ErrorEnvelope),
        (status = 422, description = "Stock insuficiente", body = crate::error::ErrorEnvelope),
        (status = 500, description = "Error interno", body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = []))
)]
pub async fn post_sale(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    headers: HeaderMap,
    Json(body): Json<PosSaleRequest>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let user = user_of(&claims).ok();
    let key = idempotency_key(&headers);
    let sold_by_name = Some(claims.sub.as_str());

    // Capture sold lines (product id + qty) before `body` is moved, so the
    // stock-webhook dispatcher can report post-sale stock without re-parsing.
    let sold_lines: Vec<(String, i64)> = body
        .items
        .iter()
        .map(|i| (i.product.clone(), i.quantity))
        .collect();

    match service::post_sale(
        db.as_ref(),
        &tenant,
        user.as_ref(),
        sold_by_name,
        key.as_deref(),
        body,
    )
    .await
    {
        Ok(resp) => {
            // ERP→web stock push (ADR-0013): fire-and-forget, never blocks the
            // POS hot path. No-op when disabled/unconfigured.
            crate::stock_webhook::notify_sale(&state, tenant.clone(), sold_lines);
            Ok((StatusCode::CREATED, Json(resp)).into_response())
        }
        Err(domain::DomainError::Conflict(msg)) if msg.starts_with("IDEMPOTENCY_CACHED:") => {
            // Replay cached payload verbatim with 200 OK.
            let json = msg.trim_start_matches("IDEMPOTENCY_CACHED:");
            let value: serde_json::Value = serde_json::from_str(json)
                .map_err(|e| ApiError::internal(format!("cache decode: {e}")))?;
            Ok((StatusCode::OK, Json(value)).into_response())
        }
        Err(e) => Err(e.into()),
    }
}

// --- GET /orders -----------------------------------------------------------

/// Lista de pedidos (orders) del tenant. Filtros por fecha, cliente, etc.
#[utoipa::path(
    get,
    path = "/api/v1/orders",
    tag = "Sales",
    responses(
        (status = 200, description = "Lista de pedidos", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 500, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = []))
)]
pub async fn list_orders(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<OrderFilters>,
) -> Result<Json<Vec<OrderDto>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let rows = service::list_orders(db.as_ref(), &tenant, filters).await?;
    Ok(Json(rows))
}

#[derive(serde::Serialize)]
pub(crate) struct OrderDetail {
    order: OrderDto,
    items: Vec<OrderItemDto>,
}

/// Detalle de un pedido (cabecera + items).
#[utoipa::path(
    get,
    path = "/api/v1/orders/{id}",
    tag = "Sales",
    params(("id" = String, Path, description = "Order record id (order:xxx)")),
    responses(
        (status = 200, description = "Pedido + items", body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = []))
)]
pub async fn get_order(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<OrderDetail>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let (order, items) = service::get_order(db.as_ref(), &tenant, &id).await?;
    Ok(Json(OrderDetail { order, items }))
}

/// `GET /api/v1/orders/{id}/receipt` — printable boleta data for a sale.
/// Read-only, tenant-scoped. 404 if the order is not in this tenant.
async fn get_receipt(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ReceiptDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let receipt = service::get_receipt(db.as_ref(), &tenant, &id).await?;
    Ok(Json(receipt))
}

// --- returns / devoluciones ------------------------------------------------

/// Crear devolución (refund) parcial o total de un pedido.
#[utoipa::path(
    post,
    path = "/api/v1/pos/returns",
    tag = "Sales",
    request_body = serde_json::Value,
    responses(
        (status = 201, description = "Devolución creada", body = serde_json::Value),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, description = "Rol insuficiente (requiere cashier+)", body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = []))
)]
pub async fn create_refund(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<NewDevolucion>,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let user = user_of(&claims).ok();
    let resp = service::create_refund(db.as_ref(), &tenant, user.as_ref(), body).await?;
    Ok((StatusCode::CREATED, Json(resp)).into_response())
}

/// Lista de devoluciones del tenant.
#[utoipa::path(
    get, path = "/api/v1/returns", tag = "Sales",
    responses(
        (status = 200, body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = []))
)]
pub async fn list_refunds(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<DevolucionFilters>,
) -> Result<Json<Vec<DevolucionDto>>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let rows = service::list_refunds(db.as_ref(), &tenant, filters).await?;
    Ok(Json(rows))
}

// --- interactions pre-check ------------------------------------------------

#[derive(serde::Deserialize)]
pub(crate) struct CheckRequest {
    /// Product record ids (`product:xxx`). Tenant-scoped lookup; missing
    /// or other-tenant ids are silently dropped.
    #[serde(default)]
    products: Vec<String>,
    /// Free-text ingredients (e.g. from a draft cart line) — useful when
    /// the POS holds items not yet linked to a product row.
    #[serde(default)]
    extra_ingredients: Vec<String>,
}

#[derive(serde::Serialize)]
pub(crate) struct CheckResponse {
    interaction_warnings: Vec<domain::sales::interactions::InteractionDetail>,
}

/// Preview interactions without committing a sale. The POS calls this when
/// the cart changes to surface warnings live (badge in UI, etc.).
/// Pre-check de interacciones farmacológicas sin commit.
#[utoipa::path(
    post, path = "/api/v1/interactions/check", tag = "Sales",
    request_body = serde_json::Value,
    responses(
        (status = 200, body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = []))
)]
pub async fn check_interactions(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<CheckRequest>,
) -> Result<Json<CheckResponse>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let mut things: Vec<Thing> = Vec::new();
    for s in &body.products {
        if let Ok(t) = surrealdb::sql::thing(s) {
            if t.tb == "product" {
                things.push(t);
            }
        }
    }
    let mut ingredients = service::load_active_ingredients(db.as_ref(), &tenant, &things)
        .await
        .unwrap_or_default();
    ingredients.extend(body.extra_ingredients);
    Ok(Json(CheckResponse {
        interaction_warnings: domain::sales::interactions::check(&ingredients),
    }))
}

// --- admin_setting CRUD ---------------------------------------------------

/// Lee un admin_setting por key.
#[utoipa::path(
    get, path = "/api/v1/settings/{key}", tag = "Sales",
    params(("key" = String, Path, description = "Setting key")),
    responses(
        (status = 200, body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = []))
)]
pub async fn get_setting(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(key): Path<String>,
) -> Result<Json<AdminSettingDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    service::get_setting(db.as_ref(), &tenant, &key)
        .await?
        .map(Json)
        .ok_or_else(ApiError::not_found)
}

#[derive(serde::Deserialize)]
pub(crate) struct SettingValue {
    value: String,
}

/// Crea o actualiza un admin_setting. Requiere admin+.
#[utoipa::path(
    put, path = "/api/v1/settings/{key}", tag = "Sales",
    params(("key" = String, Path)),
    request_body = serde_json::Value,
    responses(
        (status = 200, body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = []))
)]
pub async fn set_setting(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(key): Path<String>,
    Json(body): Json<SettingValue>,
) -> Result<Json<AdminSettingDto>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let out = service::set_setting(db.as_ref(), &tenant, &key, &body.value).await?;
    Ok(Json(out))
}

// --- POST /admin/import-historic-orders ------------------------------------

/// Bulk-load PAST sales (data migration tool). Tenant-scoped via JWT, gated
/// to admin/owner roles by the router layer.
///
/// Differs from `/pos/sale` in three deliberate ways: `created_at` is taken
/// from the input, `product.stock` stays untouched (current stock is already
/// correct), and per-order failures don't abort the batch — they surface in
/// `errors` with the offending index. 200 OK even on partial failure; clients
/// reconcile via the `created` count + `errors` list.
async fn import_historic_orders(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<historic::HistoricImportRequest>,
) -> Result<Json<historic::ImportReport>, ApiError> {
    let db = db_of(&state)?;
    let tenant = tenant_of(&claims)?;
    let user = user_of(&claims).ok();
    let sold_by_name = Some(claims.sub.as_str());
    let report =
        historic::import_historic_orders(db.as_ref(), &tenant, user.as_ref(), sold_by_name, body)
            .await?;
    Ok(Json(report))
}

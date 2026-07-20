//! Free Web public storefront — read-only catalog (PR1, ADR-0020).
//!
//! ```text
//! GET /api/v1/public/{slug}/store                          → PublicStoreDto
//! GET /api/v1/public/{slug}/catalog?q&category&limit&offset → PublicCatalogPage
//! GET /api/v1/public/{slug}/catalog/{product_slug}          → PublicProductDto
//! ```
//!
//! **No JWT** — `{slug}` is the tenant slug. Every handler goes through
//! `resolve_published_tenant` first: unless the tenant exists AND set
//! `web.published = "true"` (via the existing `PUT /api/v1/settings/{key}`),
//! all three routes answer a uniform 404 envelope — probes cannot tell
//! "unknown tenant" from "not published" (404 darkness, ADR-0005/0020).
//!
//! Distinct from [`super::public_catalog`] (Tu Farmacia sync seam,
//! `?tenant=` + config gate + optional API key): this surface serves the
//! built-in free storefront and is gated per-tenant by `admin_setting`.
//!
//! Safety: the projection never includes cost/margin fields or integer stock
//! (`availability` buckets only); prices serialize as decimal strings.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};

use crate::error::ApiError;
use crate::AppState;

use domain::catalog::{
    model::{PublicCatalogFilters, PublicCatalogPage, PublicProductDto, PublicStoreDto},
    service,
};

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

/// Public router — mounted WITHOUT auth extractors; per-IP rate limited like
/// the other unauthenticated surfaces (falls through when `rate_limit` is
/// `None` in tests).
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/public/{slug}/store", get(get_store))
        .route("/api/v1/public/{slug}/catalog", get(list_catalog))
        .route(
            "/api/v1/public/{slug}/catalog/{product_slug}",
            get(get_product),
        )
        .route_layer(crate::rate_limit::ip_layer(state))
}

/// Ficha pública de la tienda (nombre, WhatsApp, horario, retiro). 404 si el
/// tenant no existe o no publicó su web.
#[utoipa::path(get, path = "/api/v1/public/{slug}/store", tag = "PublicWeb",
    params(("slug" = String, Path, description = "Tenant slug")),
    responses((status = 200, body = PublicStoreDto), (status = 404, body = crate::error::ErrorEnvelope)))]
pub async fn get_store(
    State(s): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<PublicStoreDto>, ApiError> {
    let db = db_of(&s)?;
    let tenant = service::resolve_published_tenant(&db, &slug).await?;
    Ok(Json(service::public_store(&db, &tenant, &slug).await?))
}

/// Catálogo público paginado (solo productos activos, visibles online y de
/// venta directa). 404-oscuridad si la web no está publicada.
#[utoipa::path(get, path = "/api/v1/public/{slug}/catalog", tag = "PublicWeb",
    params(("slug" = String, Path, description = "Tenant slug")),
    responses((status = 200, body = PublicCatalogPage), (status = 404, body = crate::error::ErrorEnvelope)))]
pub async fn list_catalog(
    State(s): State<AppState>,
    Path(slug): Path<String>,
    Query(filters): Query<PublicCatalogFilters>,
) -> Result<Json<PublicCatalogPage>, ApiError> {
    let db = db_of(&s)?;
    let tenant = service::resolve_published_tenant(&db, &slug).await?;
    Ok(Json(
        service::list_public_catalog(&db, &tenant, &slug, filters).await?,
    ))
}

/// Detalle público de producto por slug. 404 si está oculto, inactivo o
/// requiere receta — indistinguible de "no existe".
#[utoipa::path(get, path = "/api/v1/public/{slug}/catalog/{product_slug}", tag = "PublicWeb",
    params(
        ("slug" = String, Path, description = "Tenant slug"),
        ("product_slug" = String, Path, description = "Product slug")
    ),
    responses((status = 200, body = PublicProductDto), (status = 404, body = crate::error::ErrorEnvelope)))]
pub async fn get_product(
    State(s): State<AppState>,
    Path((slug, product_slug)): Path<(String, String)>,
) -> Result<Json<PublicProductDto>, ApiError> {
    let db = db_of(&s)?;
    let tenant = service::resolve_published_tenant(&db, &slug).await?;
    Ok(Json(
        service::get_public_product(&db, &tenant, &product_slug).await?,
    ))
}

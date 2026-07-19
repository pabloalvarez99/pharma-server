//! Public read-only catalog endpoint (Fase 12 sync online opt-in).
//!
//! `GET /api/v1/public/catalog?tenant=<slug>&q=<term>&limit=<N>&offset=<N>`
//!
//! Designed for the external Tu Farmacia website to read a pharmacy's catalog
//! over LAN/VPS without exposing internal fields. **No JWT** — the caller is
//! identified by an opaque `tenant` slug. Rate limited per-IP (reuses the
//! existing IP layer; defaults to 60 rpm).
//!
//! ## Safety invariants
//!
//! 1. **Opt-in only.** `AppConfig.public_catalog.enabled = false` (default)
//!    makes the route return 404 from inside the handler — the route
//!    effectively doesn't exist. ADR-0005 "no surprise public exposure".
//! 2. **No tenant enumeration.** Unknown slug returns the *same* 404 as
//!    "endpoint disabled" — callers cannot discover which tenants exist.
//! 3. **Scrubbed projection.** Response includes only `external_id`, `name`,
//!    `price`, `in_stock` (bool — never the stock count), `active_ingredient`.
//!    Never `id`, `cost_price`, `stock`, `discount_percent`, internal slugs.
//! 4. **Active-only.** `active = false` rows are filtered out.
//! 5. **Rate limited.** Per-IP token bucket (60 rpm default, configurable via
//!    `[rate_limit]` table). Production CI wires `state.rate_limit = Some(..)`.
//! 6. **CORS.** Only origins in `AppConfig.public_catalog.cors_origins` (default
//!    `["https://tu-farmacia.cl"]`) get `Access-Control-Allow-Origin`. Browsers
//!    block other origins. Server-to-server callers without an `Origin` header
//!    pass through (CORS is only a browser-enforced protection layer).

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::error::ApiError;
use crate::AppState;

/// Sanitised product view. Mirror of `domain::catalog::model::ProductDto`
/// **stripped** to only the fields safe for unauth public consumption.
/// Any change here is a contract change — bump documentation accordingly.
#[derive(Debug, Clone, Serialize)]
pub struct PublicProductDto {
    /// Tenant-facing SKU code (commercial). Internal record id is never leaked.
    pub external_id: Option<String>,
    pub name: String,
    /// Monetary string; same serialisation as `ProductDto.price`.
    pub price: String,
    /// Availability bool. Never the integer stock count — that would expose
    /// purchasing patterns.
    pub in_stock: bool,
    pub active_ingredient: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PublicQuery {
    /// Tenant slug. Required. Unknown → 404 (no enumeration).
    pub tenant: String,
    /// Free-text search; case-insensitive substring of `name` or
    /// `active_ingredient`. Optional.
    #[serde(default)]
    pub q: Option<String>,
    /// Page size, clamped to `[1, 200]`. Default 50.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Number of rows to skip. Default 0.
    #[serde(default)]
    pub offset: Option<u32>,
}

/// Build the public-catalog sub-router.
///
/// Layers (innermost → outermost):
/// 1. handler — emits 404 if disabled or unknown tenant, 200 otherwise.
/// 2. CORS — strict allow-list from config.
/// 3. Per-IP rate limit — reuses `crate::rate_limit::ip_layer`. Falls through
///    when `state.rate_limit` is `None` (tests).
pub fn router(state: AppState) -> Router<AppState> {
    let cors = build_cors(&state.public_catalog.cors_origins);
    Router::new()
        .route("/api/v1/public/catalog", get(list_public_catalog))
        .layer(cors)
        .route_layer(crate::rate_limit::ip_layer(state))
}

fn build_cors(origins: &[String]) -> CorsLayer {
    let parsed: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|o| HeaderValue::from_str(o).ok())
        .collect();
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(parsed))
        .allow_methods([axum::http::Method::GET])
        .allow_headers([axum::http::header::CONTENT_TYPE])
}

#[derive(Debug, serde::Deserialize)]
struct TenantRow {
    id: Thing,
}

#[derive(Debug, serde::Deserialize)]
struct ProductRow {
    name: String,
    /// SurrealQL `decimal` round-trips to `Number::Decimal`. Avoid adding
    /// `rust_decimal` to this crate's deps just for the type — keep it as the
    /// native Surreal number and emit `to_string()` (same serialisation as
    /// `rust_decimal::Decimal`).
    price: surrealdb::sql::Number,
    stock: i64,
    external_id: Option<String>,
    active_ingredient: Option<String>,
}

async fn list_public_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PublicQuery>,
) -> Response {
    // Opt-in check. Disabled → uniform 404 (do NOT leak that the feature exists).
    if !state.public_catalog.enabled {
        return ApiError::not_found().into_response();
    }
    let Some(db) = state.db.clone() else {
        return ApiError::service_unavailable().into_response();
    };

    // Resolve tenant slug → record id. Unknown slug → same 404 as "disabled".
    let tenant = match resolve_tenant(&db, &q.tenant).await {
        Ok(Some(t)) => t,
        _ => return ApiError::not_found().into_response(),
    };

    // Optional scoped API-key gate (ADR-0014 L1). Fail-closed: when required,
    // a request without a valid `catalog:read` key for this tenant is rejected.
    if state.public_catalog.require_api_key {
        if let Some(deny) =
            crate::api_key::guard(&db, &tenant, &headers, crate::api_key::SCOPE_CATALOG_READ).await
        {
            return deny;
        }
    }

    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let offset = q.offset.unwrap_or(0);
    let search = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let rows = match query_products(&db, &tenant, search, limit, offset).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "public catalog: query failed");
            return ApiError::service_unavailable().into_response();
        }
    };

    let body: Vec<PublicProductDto> = rows
        .into_iter()
        .map(|r| PublicProductDto {
            external_id: r.external_id,
            name: r.name,
            price: r.price.to_string(),
            in_stock: r.stock > 0,
            active_ingredient: r.active_ingredient,
        })
        .collect();
    (StatusCode::OK, Json(body)).into_response()
}

async fn resolve_tenant(db: &Arc<db::Db>, slug: &str) -> Result<Option<Thing>, surrealdb::Error> {
    let mut r = db
        .query("SELECT id FROM tenant WHERE slug = $slug LIMIT 1")
        .bind(("slug", slug.to_string()))
        .await?;
    let row: Option<TenantRow> = r.take(0)?;
    Ok(row.map(|r| r.id))
}

async fn query_products(
    db: &Arc<db::Db>,
    tenant: &Thing,
    search: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<ProductRow>, surrealdb::Error> {
    // Filter: tenant-scoped + active. Optional case-insensitive substring over
    // name / active_ingredient.
    let q = if search.is_some() {
        format!(
            "SELECT name, price, stock, external_id, active_ingredient \
             FROM product \
             WHERE tenant = $t AND active = true \
               AND (string::lowercase(name) CONTAINS $q \
                 OR string::lowercase(active_ingredient ?? '') CONTAINS $q) \
             ORDER BY name LIMIT {limit} START {offset}"
        )
    } else {
        format!(
            "SELECT name, price, stock, external_id, active_ingredient \
             FROM product \
             WHERE tenant = $t AND active = true \
             ORDER BY name LIMIT {limit} START {offset}"
        )
    };
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("q", search.map(|s| s.to_lowercase()).unwrap_or_default()))
        .await?;
    let rows: Vec<ProductRow> = r.take(0)?;
    Ok(rows)
}

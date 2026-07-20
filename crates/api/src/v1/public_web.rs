//! Free Web public storefront — read-only catalog (PR1) + pickup order
//! creation (PR3), ADR-0020.
//!
//! ```text
//! GET  /api/v1/public/{slug}/store                          → PublicStoreDto
//! GET  /api/v1/public/{slug}/catalog?q&category&limit&offset → PublicCatalogPage
//! GET  /api/v1/public/{slug}/catalog/{product_slug}          → PublicProductDto
//! POST /api/v1/public/{slug}/orders/web                      → WebPickupOrderResponse
//! ```
//!
//! The order route stacks four gates (in order): `RequireApiKey` bearer
//! (401), `orders:write` scope + tenant binding (403), `web.published` 404
//! darkness, and a per-key HMAC-SHA256 request signature over
//! `"{ts}.{METHOD}.{path}.{sha256_hex(body)}"` with ±300s timestamp skew
//! (401 `SIGNATURE_INVALID` / `TIMESTAMP_SKEW`). The signature is verified
//! INSIDE the handler because it needs the raw body bytes.
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
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::public_auth::{ensure_key_matches_tenant, require_scope, RequireApiKey};
use crate::AppState;

use domain::catalog::{
    model::{PublicCatalogFilters, PublicCatalogPage, PublicProductDto, PublicStoreDto},
    service,
};
use domain::sales::{
    model::{
        WebPickupOrderRequest, WebPickupOrderResponse, WEB_ORDER_MAX_ITEMS, WEB_ORDER_MAX_QTY,
    },
    service as sales_service,
};

type HmacSha256 = Hmac<Sha256>;

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
        .route("/api/v1/public/{slug}/orders/web", post(create_web_order))
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

// --- POST /public/{slug}/orders/web (PR3) -----------------------------------

const RB_TS_HEADER: &str = "x-rb-timestamp";
const RB_SIG_HEADER: &str = "x-rb-signature";
/// Accept timestamps within ±5 min of now.
const RB_MAX_SKEW_SECS: i64 = 300;

fn signature_invalid() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "SIGNATURE_INVALID",
        "Firma inválida.",
    )
}

fn timestamp_skew() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "TIMESTAMP_SKEW",
        "Marca de tiempo fuera de rango.",
    )
}

fn validation_error() -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "VALIDATION_ERROR",
        "Datos de cliente inválidos.",
    )
}

/// Verify the per-key HMAC request signature:
/// `hex(HMAC_SHA256(hmac_secret, "{ts}.{METHOD}.{path}.{sha256_hex(body)}"))`.
/// Skew is checked first (its own 401 code); everything else — missing
/// headers, empty per-key secret (0026-era keys), non-hex, wrong MAC — is a
/// uniform `SIGNATURE_INVALID`. Comparison is constant-time
/// (`hmac::Mac::verify_slice`).
fn verify_web_signature(
    secret: &str,
    headers: &HeaderMap,
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<(), ApiError> {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let sig = header(RB_SIG_HEADER).ok_or_else(signature_invalid)?;
    let ts_raw = header(RB_TS_HEADER).ok_or_else(timestamp_skew)?;
    let ts: i64 = ts_raw.parse().map_err(|_| timestamp_skew())?;
    if (chrono::Utc::now().timestamp() - ts).abs() > RB_MAX_SKEW_SECS {
        return Err(timestamp_skew());
    }
    if secret.is_empty() {
        // Legacy CLI-minted key without hmac_secret: fail closed.
        return Err(signature_invalid());
    }
    let body_hash = hex::encode(Sha256::digest(body));
    let canonical = format!("{ts}.{method}.{path}.{body_hash}");
    let sig_bytes = hex::decode(sig).map_err(|_| signature_invalid())?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| signature_invalid())?;
    mac.update(canonical.as_bytes());
    mac.verify_slice(&sig_bytes)
        .map_err(|_| signature_invalid())
}

/// Request-shape validation the pack pins to 400 `VALIDATION_ERROR` (the
/// domain re-validates defensively with generic `INVALID_INPUT`).
fn validate_web_order(req: &WebPickupOrderRequest) -> Result<(), ApiError> {
    if req.customer.name.trim().is_empty() || req.customer.phone.trim().is_empty() {
        return Err(validation_error());
    }
    if req.items.is_empty() || req.items.len() > WEB_ORDER_MAX_ITEMS {
        return Err(validation_error());
    }
    if req
        .items
        .iter()
        .any(|i| i.qty < 1 || i.qty > WEB_ORDER_MAX_QTY)
    {
        return Err(validation_error());
    }
    Ok(())
}

/// Crear pedido de retiro (pickup) desde el storefront. Bearer `rb_live_…` +
/// firma HMAC por clave + `Idempotency-Key` opcional (reintento seguro).
#[utoipa::path(post, path = "/api/v1/public/{slug}/orders/web", tag = "PublicWeb",
    params(("slug" = String, Path, description = "Tenant slug")),
    request_body = WebPickupOrderRequest,
    responses(
        (status = 201, body = WebPickupOrderResponse),
        (status = 200, description = "Idempotency-Key replay; payload cacheado", body = WebPickupOrderResponse),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 422, body = crate::error::ErrorEnvelope),
    ),
    security(("api_key" = [])))]
pub async fn create_web_order(
    State(s): State<AppState>,
    Path(slug): Path<String>,
    RequireApiKey(ctx): RequireApiKey,
    headers: HeaderMap,
    body: Bytes,
) -> Result<axum::response::Response, ApiError> {
    let db = db_of(&s)?;
    // Scope first (key-only, leaks nothing about the tenant), then 404
    // darkness for unpublished/unknown slugs (Law 4: even a valid key sees
    // 404), then the cross-tenant binding.
    require_scope(&ctx, "orders:write")?;
    let tenant = service::resolve_published_tenant(&db, &slug).await?;
    ensure_key_matches_tenant(&ctx, &tenant)?;

    let path = format!("/api/v1/public/{slug}/orders/web");
    verify_web_signature(&ctx.hmac_secret, &headers, "POST", &path, &body)?;

    let req: WebPickupOrderRequest =
        serde_json::from_slice(&body).map_err(|_| validation_error())?;
    validate_web_order(&req)?;

    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty());

    match sales_service::create_web_order(&db, &tenant, idempotency_key, req).await {
        Ok(resp) => Ok((StatusCode::CREATED, Json(resp)).into_response()),
        Err(domain::DomainError::Conflict(msg)) if msg.starts_with("IDEMPOTENCY_CACHED:") => {
            let json = msg.trim_start_matches("IDEMPOTENCY_CACHED:");
            let value: serde_json::Value = serde_json::from_str(json)
                .map_err(|e| ApiError::internal(format!("cache decode: {e}")))?;
            Ok((StatusCode::OK, Json(value)).into_response())
        }
        Err(domain::DomainError::Invalid(msg))
            if msg.starts_with(sales_service::PRODUCT_NOT_AVAILABLE_MARKER) =>
        {
            Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PRODUCT_NOT_AVAILABLE",
                "Producto no disponible.",
            ))
        }
        Err(e) => Err(e.into()),
    }
}

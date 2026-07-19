//! Catalog HTTP handlers (Fase 2). Reads require a valid bearer token;
//! mutations additionally require role `admin`/`owner` (RequireRole layer).

use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, Query, State},
    http::header,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::catalog::{model::*, service};

use crate::role::admin_plus;

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    // Static path segments before `{id}` so `by-barcode` is not captured as an id.
    let reads = Router::new()
        .route("/api/v1/products", get(list_products))
        .route("/api/v1/products/stats", get(product_stats))
        .route("/api/v1/products/export", get(export_products))
        .route("/api/v1/products/by-barcode/{code}", get(get_by_barcode))
        .route("/api/v1/products/{id}/variants", get(list_variants))
        .route("/api/v1/products/{id}", get(get_product))
        .route("/api/v1/categories", get(list_categories))
        .route("/api/v1/categories/{id}", get(get_category))
        .route("/api/v1/etiquetas/search", get(etiquetas));

    let writes = Router::new()
        .route("/api/v1/products", post(create_product))
        .route("/api/v1/products/import", post(import_products))
        .route("/api/v1/products/bulk-price", post(bulk_price))
        .route("/api/v1/products/update-prices", post(update_prices))
        .route("/api/v1/products/{id}/variants", post(create_variant))
        .route(
            "/api/v1/products/{id}",
            axum::routing::patch(update_product).delete(delete_product),
        )
        .route("/api/v1/products/{id}/stock", post(adjust_stock))
        .route("/api/v1/categories", post(create_category))
        .route(
            "/api/v1/categories/{id}",
            axum::routing::patch(update_category).delete(delete_category),
        )
        .route_layer(crate::role::layer(state, admin_plus()));

    reads.merge(writes)
}

// --- products: reads -------------------------------------------------------

/// Query string for product list. Fields are flat (no `#[serde(flatten)]`)
/// because `serde_urlencoded` + flatten rejects `?limit=5` with 400.
#[derive(Debug, Default, Deserialize)]
pub struct ListProductsQuery {
    pub search: Option<String>,
    pub category: Option<String>,
    pub active: Option<bool>,
    pub low_stock: Option<i64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// `true` = include variant children (default: only top-level).
    #[serde(default)]
    pub include_variants: bool,
}

impl ListProductsQuery {
    fn into_parts(self) -> (ProductFilters, bool) {
        (
            ProductFilters {
                search: self.search,
                category: self.category,
                active: self.active,
                low_stock: self.low_stock,
                limit: self.limit,
                offset: self.offset,
            },
            self.include_variants,
        )
    }
}

/// Lista productos del tenant. Por defecto oculta variantes hijas (`parent_id`);
/// pasar `?include_variants=true` para verlas, o usar `GET .../variants`.
#[utoipa::path(get, path = "/api/v1/products", tag = "Catalog",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_products(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<ListProductsQuery>,
) -> Result<Json<Vec<ProductDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let (filters, include_variants) = q.into_parts();
    Ok(Json(
        service::list_products_with_variants(&db, &t, filters, include_variants).await?,
    ))
}

/// Detalle de un producto.
#[utoipa::path(get, path = "/api/v1/products/{id}", tag = "Catalog",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn get_product(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ProductDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_product(&db, &t, &id).await?))
}

/// Resuelve un código de barras del tenant al producto vendible (variante o SKU plano).
#[utoipa::path(get, path = "/api/v1/products/by-barcode/{code}", tag = "Catalog",
    params(("code" = String, Path, description = "EAN/UPC u otro barcode del local")),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn get_by_barcode(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(code): Path<String>,
) -> Result<Json<ProductDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::find_by_barcode(&db, &t, &code).await?))
}

/// Lista variantes (hijos multi-SKU) de un producto padre.
#[utoipa::path(get, path = "/api/v1/products/{id}/variants", tag = "Catalog",
    params(("id" = String, Path, description = "Id del producto padre")),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_variants(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<Vec<ProductDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_variants(&db, &t, &id).await?))
}

/// Crea una variante vendible bajo el producto padre (stock + barcode propios).
#[utoipa::path(post, path = "/api/v1/products/{id}/variants", tag = "Catalog",
    params(("id" = String, Path, description = "Id del producto padre")),
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 409, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn create_variant(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(input): Json<NewVariant>,
) -> Result<Json<ProductDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_variant(&db, &t, &id, input).await?))
}

/// Estadísticas de catálogo (totales, sin stock, etc.).
#[utoipa::path(get, path = "/api/v1/products/stats", tag = "Catalog",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn product_stats(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<ProductStats>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::stats(&db, &t).await?))
}

#[derive(Deserialize)]
pub(crate) struct EtiquetaQuery {
    #[serde(default)]
    q: String,
}

/// Autocomplete de etiquetas (laboratorio, ingrediente, acción).
#[utoipa::path(get, path = "/api/v1/etiquetas/search", tag = "Catalog",
    params(("q" = String, Query, description = "Substring de búsqueda")),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn etiquetas(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<EtiquetaQuery>,
) -> Result<Json<EtiquetaResults>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::etiquetas(&db, &t, &q.q).await?))
}

/// Exporta catálogo como CSV.
#[utoipa::path(get, path = "/api/v1/products/export", tag = "Catalog",
    responses(
        (status = 200, description = "CSV de productos", content_type = "text/csv"),
        (status = 401, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn export_products(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Response, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let products = service::list_products(&db, &t, ProductFilters::default()).await?;
    let mut w = csv::Writer::from_writer(vec![]);
    w.write_record([
        "id",
        "name",
        "slug",
        "price",
        "cost_price",
        "stock",
        "category",
        "active",
        "external_id",
        "laboratory",
        "active_ingredient",
        "therapeutic_action",
        "prescription_type",
        "presentation",
        "discount_percent",
    ])
    .map_err(|e| ApiError::internal(e.to_string()))?;
    for p in products {
        w.write_record([
            p.id,
            p.name,
            p.slug,
            p.price.to_string(),
            p.cost_price.map(|d| d.to_string()).unwrap_or_default(),
            p.stock.to_string(),
            p.category.unwrap_or_default(),
            p.active.to_string(),
            p.external_id.unwrap_or_default(),
            p.laboratory.unwrap_or_default(),
            p.active_ingredient.unwrap_or_default(),
            p.therapeutic_action.unwrap_or_default(),
            p.prescription_type,
            p.presentation.unwrap_or_default(),
            p.discount_percent
                .map(|n| n.to_string())
                .unwrap_or_default(),
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
                "attachment; filename=\"products.csv\"",
            ),
        ],
        body,
    )
        .into_response())
}

// --- products: writes ------------------------------------------------------

/// Crea un producto. Requiere admin+.
#[utoipa::path(post, path = "/api/v1/products", tag = "Catalog",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn create_product(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewProduct>,
) -> Result<Json<ProductDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_product(&db, &t, input).await?))
}

/// Actualiza un producto (patch). Requiere admin+.
#[utoipa::path(patch, path = "/api/v1/products/{id}", tag = "Catalog",
    params(("id" = String, Path)), request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn update_product(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<UpdateProduct>,
) -> Result<Json<ProductDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::update_product(&db, &t, &id, patch).await?))
}

/// Elimina (soft) un producto. Requiere admin+.
#[utoipa::path(delete, path = "/api/v1/products/{id}", tag = "Catalog",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn delete_product(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    service::delete_product(&db, &t, &id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

/// Ajuste rápido de stock por producto (atajo de stock-movements). Requiere admin+.
#[utoipa::path(post, path = "/api/v1/products/{id}/stock", tag = "Catalog",
    params(("id" = String, Path)), request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn adjust_stock(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(adj): Json<StockAdjust>,
) -> Result<Json<ProductDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let product = service::adjust_stock(&db, &t, &id, adj, Some(&claims.sub)).await?;
    // ERP→web stock push (ADR-0013 trigger `manual.adjust`): fire-and-forget.
    crate::stock_webhook::notify_products(&s, t.clone(), vec![id.clone()]);
    Ok(Json(product))
}

/// Bulk update de precios por % o monto. Requiere admin+.
#[utoipa::path(post, path = "/api/v1/products/bulk-price", tag = "Catalog",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn bulk_price(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<BulkPrice>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let n = service::bulk_price(&db, &t, req).await?;
    Ok(Json(serde_json::json!({ "repriced": n })))
}

/// Reprice from `supplier_price_list`. That table arrives in Fase 5
/// (purchasing); endpoint shape is reserved now, returns 501.
#[utoipa::path(post, path = "/api/v1/products/update-prices", tag = "Catalog",
    responses(
        (status = 501, description = "No implementado todavía", body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn update_prices() -> ApiError {
    ApiError::not_implemented().with_details(serde_json::json!({
        "reason": "supplier_price_list llega en Fase 5 (compras)"
    }))
}

#[derive(Serialize)]
pub(crate) struct ImportSummary {
    /// Rows that resulted in a new `product` row.
    created: usize,
    /// Rows that matched an existing product by tenant + external_id and were
    /// updated in place (price/stock/ingredient/laboratory refreshed). Lets the
    /// bulk catalog migration be idempotent — re-running never duplicates.
    updated: usize,
    /// Rows that could not be created or updated. See `errors` for detail.
    failed: usize,
    errors: Vec<ImportError>,
    /// Echo of the `dry_run` flag. When true NOTHING was written — these counts
    /// are a *preview* the operator confirms before committing the catalog.
    dry_run: bool,
}

#[derive(Serialize)]
pub(crate) struct ImportError {
    line: usize,
    message: String,
}

#[derive(Deserialize, Default)]
pub(crate) struct ImportParams {
    /// When true, validate + count rows WITHOUT writing anything. Powers the
    /// "preview antes de confirmar" step in the importar view.
    #[serde(default)]
    dry_run: bool,
}

/// Strips a leading UTF-8 BOM (`EF BB BF`). Excel always prepends one when it
/// saves "CSV UTF-8", which otherwise glues itself to the first header (`name`
/// becomes `\u{feff}name`) and breaks column detection for real migrant files.
fn strip_bom(bytes: Vec<u8>) -> Vec<u8> {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        bytes[3..].to_vec()
    } else {
        bytes
    }
}

/// Sniffs the field delimiter from the first line. Excel in Chile/LATAM exports
/// CSV with `;` (because the locale uses `,` as the decimal mark); plain exports
/// use `,`; some tools use TAB. Picks whichever appears most in the header row,
/// defaulting to `,`.
fn sniff_delimiter(bytes: &[u8]) -> u8 {
    let header_line = bytes.split(|&b| b == b'\n').next().unwrap_or(bytes);
    let count = |d: u8| header_line.iter().filter(|&&b| b == d).count();
    [
        (b';', count(b';')),
        (b'\t', count(b'\t')),
        (b',', count(b',')),
    ]
    .into_iter()
    .filter(|&(_, c)| c > 0)
    .max_by_key(|&(_, c)| c)
    .map(|(d, _)| d)
    .unwrap_or(b',')
}

/// Folds a raw CSV header to a lookup key: lowercased, accents removed, spaces
/// and dashes collapsed to `_`. So `Código de Barras`, `codigo_barras` and
/// `CODIGO BARRAS` all land on `codigo_barras`.
fn norm_key(h: &str) -> String {
    let mut out = String::with_capacity(h.len());
    for ch in h.trim().chars() {
        let c = match ch {
            'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => 'a',
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
            'ñ' | 'Ñ' => 'n',
            ' ' | '-' | '.' => '_',
            '\u{feff}' => continue,
            other => other.to_ascii_lowercase(),
        };
        out.push(c);
    }
    out
}

/// Maps a (normalized) header to the canonical column the importer understands,
/// accepting common Spanish names a migrant's previous system would emit.
fn canon_header(raw: &str) -> Option<&'static str> {
    match norm_key(raw).as_str() {
        "name" | "nombre" | "producto" | "descripcion_producto" => Some("name"),
        "price" | "precio" | "precio_venta" | "pvp" | "valor" | "valor_venta" => Some("price"),
        "sale_price" | "precioventa" => Some("sale_price"),
        "cost_price" | "costo" | "precio_costo" | "costo_unitario" | "precio_compra" => {
            Some("cost_price")
        }
        "stock" | "existencia" | "existencias" | "cantidad" | "inventario" => Some("stock"),
        "external_id" | "sku" | "codigo" | "codigo_interno" | "cod" | "id_externo"
        | "codigo_producto" => Some("external_id"),
        "category" | "categoria" | "rubro" | "familia" => Some("category"),
        "laboratory" | "laboratorio" | "lab" | "marca" => Some("laboratory"),
        "active_ingredient" | "principio_activo" | "ingrediente_activo" => {
            Some("active_ingredient")
        }
        "therapeutic_action" | "accion_terapeutica" | "accion" => Some("therapeutic_action"),
        "prescription_type" | "tipo_receta" | "receta" => Some("prescription_type"),
        "presentation" | "presentacion" | "formato" => Some("presentation"),
        "discount_percent" | "descuento" | "descuento_porcentaje" | "dcto" => {
            Some("discount_percent")
        }
        "image_url" | "imagen" | "url_imagen" | "foto" => Some("image_url"),
        "description" | "descripcion" | "detalle" | "glosa" => Some("description"),
        "slug" => Some("slug"),
        "barcode" | "codigo_barra" | "codigo_barras" | "codigo_de_barras" | "codigobarra"
        | "ean" | "ean13" | "cod_barra" => Some("barcode"),
        _ => None,
    }
}

/// Builds canonical-column → index from the header row. First occurrence wins.
fn resolve_columns(headers: &csv::StringRecord) -> std::collections::HashMap<&'static str, usize> {
    let mut map = std::collections::HashMap::new();
    for (i, h) in headers.iter().enumerate() {
        if let Some(canon) = canon_header(h) {
            map.entry(canon).or_insert(i);
        }
    }
    map
}

/// `true` for dot-grouped thousands like `1.990`, `12.500`, `1.234.567` — i.e.
/// `^\d{1,3}(\.\d{3})+$`. Used to disambiguate a lone `.` as a thousands mark
/// (CLP is integer pesos) versus a real decimal point (`12.50`).
fn is_dot_thousands(s: &str) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    let mut parts = body.split('.');
    let first = parts.next().unwrap_or("");
    if first.is_empty() || first.len() > 3 || !first.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let mut any = false;
    for p in parts {
        any = true;
        if p.len() != 3 || !p.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    any
}

/// Parses a price/cost cell tolerant of Chilean/Excel formatting: strips a
/// leading `$`, all whitespace, and resolves thousands vs decimal separators.
/// CLP prices are integer pesos, so `1.990` / `12.500` MUST become 1990 / 12500
/// (not 1.99 / 12.5). Heuristics:
///   - both `.` and `,` present → the LAST one is the decimal point, the other
///     is a thousands separator (handles `1.990,50` and `1,990.50`).
///   - only `.` and it looks dot-grouped (`1.234.567`) → strip the dots.
///   - only `,` → comma is the decimal separator (`1990,5` → `1990.5`).
fn normalize_decimal(raw: &str) -> Option<rust_decimal::Decimal> {
    let mut s: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(rest) = s.strip_prefix('$') {
        s = rest.to_string();
    }
    if s.is_empty() {
        return None;
    }
    let has_dot = s.contains('.');
    let has_comma = s.contains(',');
    let cleaned = if has_dot && has_comma {
        if s.rfind(',') > s.rfind('.') {
            s.replace('.', "").replace(',', ".")
        } else {
            s.replace(',', "")
        }
    } else if has_comma {
        s.replace(',', ".")
    } else if has_dot && is_dot_thousands(&s) {
        s.replace('.', "")
    } else {
        s
    };
    cleaned.parse::<rust_decimal::Decimal>().ok()
}

/// Parses an integer cell (stock, discount) tolerant of `1.000`/`1 000`/`$`.
fn normalize_int(raw: &str) -> Option<i64> {
    let digits: String = raw
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    if digits.is_empty() || digits == "-" {
        return None;
    }
    digits.parse().ok()
}

/// Import CSV de productos (multipart). Requiere admin+. Con `?dry_run=true`
/// valida y cuenta SIN escribir nada (preview antes de confirmar).
#[utoipa::path(post, path = "/api/v1/products/import", tag = "Catalog",
    params(("dry_run" = Option<bool>, Query, description = "Valida sin escribir (preview)")),
    request_body(content = String, content_type = "multipart/form-data"),
    responses((status = 200, body = serde_json::Value), (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope), (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn import_products(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(params): Query<ImportParams>,
    mut mp: Multipart,
) -> Result<Json<ImportSummary>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let dry_run = params.dry_run;

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
    let bytes = strip_bom(bytes);
    let delimiter = sniff_delimiter(&bytes);

    // `flexible` so rows with extra/missing columns don't abort the whole parse;
    // a row that's short on a required cell is reported per-row instead.
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(bytes.as_slice());
    let headers = rdr
        .headers()
        .map_err(|e| ApiError::invalid(format!("CSV sin cabecera válida: {e}")))?
        .clone();
    let cols = resolve_columns(&headers);
    let i_name = cols.get("name").copied();
    let i_price = cols
        .get("price")
        .or_else(|| cols.get("sale_price"))
        .copied();
    let (Some(i_name), Some(i_price)) = (i_name, i_price) else {
        return Err(ApiError::invalid(
            "CSV debe incluir al menos columnas `name` y `price` (acepta nombres en \
             español: `nombre`, `precio`).",
        ));
    };
    let cell = |rec: &csv::StringRecord, key: &str| {
        cols.get(key)
            .and_then(|&i| rec.get(i))
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };

    let mut summary = ImportSummary {
        created: 0,
        updated: 0,
        failed: 0,
        errors: Vec::new(),
        dry_run,
    };
    // In dry-run we can't read back rows we haven't written, so track external_ids
    // seen within this file to count within-file repeats as updates (matches the
    // committed behaviour where the 2nd occurrence updates the 1st).
    let mut seen_xids: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Category column carries free-text names in a migrant's CSV (`Analgésicos`),
    // not `category:xxx` record ids. Resolve name → id, creating the category on
    // first sight (find-or-create), so the operator's taxonomy comes across. Seed
    // the cache with existing categories so re-imports reuse them, never duplicate.
    let mut cat_cache: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if !dry_run {
        for c in domain::catalog::repo::list_categories(&db, &t)
            .await
            .unwrap_or_default()
        {
            cat_cache.insert(c.name.to_lowercase(), c.id);
        }
    }
    for (n, rec) in rdr.records().enumerate() {
        let line = n + 2; // +1 header, +1 to 1-based
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
        let name = rec.get(i_name).unwrap_or("").trim().to_string();
        if name.is_empty() {
            // Skip fully-empty rows (trailing blank lines from Excel) silently;
            // a row with data but no name is a real error.
            if rec.iter().all(|c| c.trim().is_empty()) {
                continue;
            }
            summary.failed += 1;
            summary.errors.push(ImportError {
                line,
                message: "falta el nombre del producto".into(),
            });
            continue;
        }
        let price_raw = rec.get(i_price).unwrap_or("").trim();
        let price = match normalize_decimal(price_raw) {
            Some(p) => p,
            None => {
                summary.failed += 1;
                summary.errors.push(ImportError {
                    line,
                    message: format!("precio inválido: «{price_raw}»"),
                });
                continue;
            }
        };
        let external_id = cell(&rec, "external_id");
        let cost_price = cell(&rec, "cost_price").and_then(|v| normalize_decimal(&v));
        let stock: i64 = cell(&rec, "stock")
            .and_then(|v| normalize_int(&v))
            .unwrap_or(0);
        let laboratory = cell(&rec, "laboratory");
        let therapeutic_action = cell(&rec, "therapeutic_action");
        let active_ingredient = cell(&rec, "active_ingredient");
        let prescription_type = cell(&rec, "prescription_type");
        let presentation = cell(&rec, "presentation");
        let discount_percent: Option<i64> =
            cell(&rec, "discount_percent").and_then(|v| normalize_int(&v));
        let category_raw = cell(&rec, "category");
        let image_url = cell(&rec, "image_url");
        let description = cell(&rec, "description");
        let slug = cell(&rec, "slug");
        let barcode = cell(&rec, "barcode");

        // Resolve the category name → record id (commit path only — dry-run never
        // writes, so it skips category resolution entirely).
        let category = if dry_run {
            None
        } else if let Some(craw) = category_raw.as_deref() {
            if craw.starts_with("category:") {
                Some(craw.to_string())
            } else {
                let key = craw.to_lowercase();
                match cat_cache.get(&key) {
                    Some(id) => Some(id.clone()),
                    None => match service::create_category(
                        &db,
                        &t,
                        domain::catalog::model::NewCategory {
                            name: craw.to_string(),
                            slug: None,
                            description: None,
                            image_url: None,
                        },
                    )
                    .await
                    {
                        Ok(dto) => {
                            cat_cache.insert(key, dto.id.clone());
                            Some(dto.id)
                        }
                        Err(e) => {
                            summary.failed += 1;
                            summary.errors.push(ImportError {
                                line,
                                message: format!("categoría: {e}"),
                            });
                            continue;
                        }
                    },
                }
            }
        } else {
            None
        };

        // Upsert-by-external_id: re-running the same CSV updates existing
        // rows instead of duplicating them with `-2` slug suffixes.
        if let Some(xid) = external_id.as_deref() {
            // Within-file repeat (only meaningful in dry-run; committed runs see
            // the row already persisted via find_id_by_external_id below).
            if dry_run && !seen_xids.insert(xid.to_string()) {
                summary.updated += 1;
                continue;
            }
            match domain::catalog::repo::find_id_by_external_id(&db, &t, xid).await {
                Ok(Some(existing)) => {
                    if dry_run {
                        summary.updated += 1;
                        continue;
                    }
                    let patch = domain::catalog::model::UpdateProduct {
                        name: Some(name.clone()),
                        description: description.clone(),
                        price: Some(price),
                        cost_price,
                        category: category.clone(),
                        image_url: image_url.clone(),
                        active: None,
                        external_id: Some(xid.to_string()),
                        laboratory: laboratory.clone(),
                        therapeutic_action: therapeutic_action.clone(),
                        active_ingredient: active_ingredient.clone(),
                        prescription_type: prescription_type.clone(),
                        presentation: presentation.clone(),
                        discount_percent,
                    };
                    match service::update_product(&db, &t, &existing.to_string(), patch).await {
                        Ok(_) => {
                            summary.updated += 1;
                            if let Some(bc) = barcode.as_deref() {
                                if let Err(e) =
                                    domain::catalog::repo::upsert_barcode(&db, &t, &existing, bc)
                                        .await
                                {
                                    summary.errors.push(ImportError {
                                        line,
                                        message: format!("barcode: {e}"),
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            summary.failed += 1;
                            summary.errors.push(ImportError {
                                line,
                                message: e.to_string(),
                            });
                        }
                    }
                    continue;
                }
                Ok(None) => {} // fall through to create
                Err(e) => {
                    summary.failed += 1;
                    summary.errors.push(ImportError {
                        line,
                        message: e.to_string(),
                    });
                    continue;
                }
            }
        }

        if dry_run {
            summary.created += 1;
            continue;
        }

        let input = NewProduct {
            name,
            slug,
            description,
            price,
            cost_price,
            stock,
            category,
            image_url,
            external_id,
            laboratory,
            therapeutic_action,
            active_ingredient,
            prescription_type,
            presentation,
            discount_percent,
        };
        match service::create_product(&db, &t, input).await {
            Ok(dto) => {
                summary.created += 1;
                if let Some(bc) = barcode.as_deref() {
                    if let Ok(pid) = surrealdb::sql::thing(&dto.id) {
                        if let Err(e) =
                            domain::catalog::repo::upsert_barcode(&db, &t, &pid, bc).await
                        {
                            summary.errors.push(ImportError {
                                line,
                                message: format!("barcode: {e}"),
                            });
                        }
                    }
                }
            }
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

// --- categories ------------------------------------------------------------

/// Lista categorías del catálogo.
#[utoipa::path(get, path = "/api/v1/categories", tag = "Catalog",
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_categories(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<CategoryDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(domain::catalog::repo::list_categories(&db, &t).await?))
}

/// Detalle de una categoría.
#[utoipa::path(get, path = "/api/v1/categories/{id}", tag = "Catalog",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn get_category(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<CategoryDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_category(&db, &t, &id).await?))
}

/// Crea categoría. Requiere admin+.
#[utoipa::path(post, path = "/api/v1/categories", tag = "Catalog",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn create_category(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewCategory>,
) -> Result<Json<CategoryDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_category(&db, &t, input).await?))
}

/// Actualiza categoría. Requiere admin+.
#[utoipa::path(patch, path = "/api/v1/categories/{id}", tag = "Catalog",
    params(("id" = String, Path)), request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn update_category(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<UpdateCategory>,
) -> Result<Json<CategoryDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::update_category(&db, &t, &id, patch).await?))
}

/// Elimina (soft) categoría. Requiere admin+.
#[utoipa::path(delete, path = "/api/v1/categories/{id}", tag = "Catalog",
    params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value), (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope), (status = 404, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn delete_category(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    service::delete_category(&db, &t, &id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[cfg(test)]
mod import_parsing_tests {
    use super::*;
    use rust_decimal::Decimal;

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[test]
    fn bom_is_stripped() {
        assert_eq!(strip_bom(vec![0xEF, 0xBB, 0xBF, b'n']), vec![b'n']);
        assert_eq!(strip_bom(vec![b'n']), vec![b'n']);
    }

    #[test]
    fn delimiter_sniff_picks_semicolon_for_excel_cl() {
        assert_eq!(sniff_delimiter(b"name;price;stock\n"), b';');
        assert_eq!(sniff_delimiter(b"name,price,stock\n"), b',');
        assert_eq!(sniff_delimiter(b"name\tprice\tstock\n"), b'\t');
        assert_eq!(sniff_delimiter(b"name\n"), b',');
    }

    #[test]
    fn spanish_headers_resolve() {
        assert_eq!(canon_header("Nombre"), Some("name"));
        assert_eq!(canon_header("Precio"), Some("price"));
        assert_eq!(canon_header("Código"), Some("external_id"));
        assert_eq!(canon_header("Código de Barras"), Some("barcode"));
        assert_eq!(canon_header("categoría"), Some("category"));
        assert_eq!(canon_header("Principio Activo"), Some("active_ingredient"));
        assert_eq!(canon_header("desconocido"), None);
    }

    #[test]
    fn clp_thousands_separator_is_integer_pesos() {
        assert_eq!(normalize_decimal("1.990"), Some(dec("1990")));
        assert_eq!(normalize_decimal("12.500"), Some(dec("12500")));
        assert_eq!(normalize_decimal("1.234.567"), Some(dec("1234567")));
        assert_eq!(normalize_decimal("$ 2.490"), Some(dec("2490")));
    }

    #[test]
    fn real_decimals_survive() {
        assert_eq!(normalize_decimal("12.50"), Some(dec("12.50")));
        assert_eq!(normalize_decimal("1990,50"), Some(dec("1990.50")));
        assert_eq!(normalize_decimal("1.990,50"), Some(dec("1990.50")));
        assert_eq!(normalize_decimal("1,990.50"), Some(dec("1990.50")));
        assert_eq!(normalize_decimal("1990"), Some(dec("1990")));
    }

    #[test]
    fn garbage_price_rejected() {
        assert_eq!(normalize_decimal("abc"), None);
        assert_eq!(normalize_decimal(""), None);
        assert_eq!(normalize_decimal("$"), None);
    }

    #[test]
    fn int_cells_tolerate_formatting() {
        assert_eq!(normalize_int("1.000"), Some(1000));
        assert_eq!(normalize_int("40"), Some(40));
        assert_eq!(normalize_int(""), None);
        assert_eq!(normalize_int("abc"), None);
    }
}

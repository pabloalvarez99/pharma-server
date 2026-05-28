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

const WRITE_ROLES: &[&str] = &["admin", "owner"];

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    let reads = Router::new()
        .route("/api/v1/products", get(list_products))
        .route("/api/v1/products/stats", get(product_stats))
        .route("/api/v1/products/export", get(export_products))
        .route("/api/v1/products/{id}", get(get_product))
        .route("/api/v1/categories", get(list_categories))
        .route("/api/v1/categories/{id}", get(get_category))
        .route("/api/v1/etiquetas/search", get(etiquetas));

    let writes = Router::new()
        .route("/api/v1/products", post(create_product))
        .route("/api/v1/products/import", post(import_products))
        .route("/api/v1/products/bulk-price", post(bulk_price))
        .route("/api/v1/products/update-prices", post(update_prices))
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
        .route_layer(crate::role::layer(state, WRITE_ROLES));

    reads.merge(writes)
}

// --- products: reads -------------------------------------------------------

async fn list_products(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<ProductFilters>,
) -> Result<Json<Vec<ProductDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_products(&db, &t, filters).await?))
}

async fn get_product(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<ProductDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_product(&db, &t, &id).await?))
}

async fn product_stats(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<ProductStats>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::stats(&db, &t).await?))
}

#[derive(Deserialize)]
struct EtiquetaQuery {
    #[serde(default)]
    q: String,
}

async fn etiquetas(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(q): Query<EtiquetaQuery>,
) -> Result<Json<EtiquetaResults>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::etiquetas(&db, &t, &q.q).await?))
}

async fn export_products(
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

async fn create_product(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewProduct>,
) -> Result<Json<ProductDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_product(&db, &t, input).await?))
}

async fn update_product(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<UpdateProduct>,
) -> Result<Json<ProductDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::update_product(&db, &t, &id, patch).await?))
}

async fn delete_product(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    service::delete_product(&db, &t, &id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn adjust_stock(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(adj): Json<StockAdjust>,
) -> Result<Json<ProductDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(
        service::adjust_stock(&db, &t, &id, adj, Some(&claims.sub)).await?,
    ))
}

async fn bulk_price(
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
async fn update_prices() -> ApiError {
    ApiError::not_implemented().with_details(serde_json::json!({
        "reason": "supplier_price_list llega en Fase 5 (compras)"
    }))
}

#[derive(Serialize)]
struct ImportSummary {
    /// Rows that resulted in a new `product` row.
    created: usize,
    /// Rows that matched an existing product by tenant + external_id and were
    /// updated in place (price/stock/ingredient/laboratory refreshed). Lets the
    /// bulk catalog migration be idempotent — re-running never duplicates.
    updated: usize,
    /// Rows that could not be created or updated. See `errors` for detail.
    failed: usize,
    errors: Vec<ImportError>,
}

#[derive(Serialize)]
struct ImportError {
    line: usize,
    message: String,
}

async fn import_products(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    mut mp: Multipart,
) -> Result<Json<ImportSummary>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;

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
    // Accept either `price` or `sale_price` for the price column to match the
    // catalogo CSV format emitted by `scripts/extract_tufarmacia_full.py`.
    let i_price = col("price").or_else(|| col("sale_price"));
    let (Some(i_name), Some(i_price)) = (col("name"), i_price) else {
        return Err(ApiError::invalid(
            "CSV debe incluir al menos columnas `name` y `price` (o `sale_price`)",
        ));
    };
    let get = |rec: &csv::StringRecord, idx: Option<usize>| {
        idx.and_then(|i| rec.get(i))
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };

    let mut summary = ImportSummary {
        created: 0,
        updated: 0,
        failed: 0,
        errors: Vec::new(),
    };
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
        let price = match rec.get(i_price).unwrap_or("").trim().parse() {
            Ok(p) => p,
            Err(_) => {
                summary.failed += 1;
                summary.errors.push(ImportError {
                    line,
                    message: "precio inválido".into(),
                });
                continue;
            }
        };
        let external_id = get(&rec, col("external_id"));
        let cost_price = get(&rec, col("cost_price")).and_then(|v| v.parse().ok());
        let stock: i64 = get(&rec, col("stock"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let laboratory = get(&rec, col("laboratory"));
        let therapeutic_action = get(&rec, col("therapeutic_action"));
        let active_ingredient = get(&rec, col("active_ingredient"));
        let prescription_type = get(&rec, col("prescription_type"));
        let presentation = get(&rec, col("presentation"));
        let discount_percent: Option<i64> =
            get(&rec, col("discount_percent")).and_then(|v| v.parse().ok());
        let category = get(&rec, col("category"));
        let image_url = get(&rec, col("image_url"));
        let description = get(&rec, col("description"));
        let slug = get(&rec, col("slug"));

        // Upsert-by-external_id: re-running the same CSV updates existing
        // rows instead of duplicating them with `-2` slug suffixes.
        if let Some(xid) = external_id.as_deref() {
            match domain::catalog::repo::find_id_by_external_id(&db, &t, xid).await {
                Ok(Some(existing)) => {
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
                        Ok(_) => summary.updated += 1,
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

// --- categories ------------------------------------------------------------

async fn list_categories(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<CategoryDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(domain::catalog::repo::list_categories(&db, &t).await?))
}

async fn get_category(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<CategoryDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_category(&db, &t, &id).await?))
}

async fn create_category(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewCategory>,
) -> Result<Json<CategoryDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_category(&db, &t, input).await?))
}

async fn update_category(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<UpdateCategory>,
) -> Result<Json<CategoryDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::update_category(&db, &t, &id, patch).await?))
}

async fn delete_category(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    service::delete_category(&db, &t, &id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

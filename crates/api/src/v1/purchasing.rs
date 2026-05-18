//! Purchasing HTTP handlers (Fase 5-subset). Reads require a valid bearer
//! token; mutations additionally require role `admin`/`owner`.

use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

use domain::purchasing::{model::*, service};

const WRITE_ROLES: &[&str] = &["admin", "owner"];

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    let reads = Router::new()
        .route("/api/v1/suppliers", get(list_suppliers))
        .route("/api/v1/suppliers/{id}", get(get_supplier))
        .route("/api/v1/supplier-prices", get(list_prices))
        .route("/api/v1/purchase-orders", get(list_purchase_orders))
        .route("/api/v1/purchase-orders/{id}", get(get_purchase_order))
        .route(
            "/api/v1/purchase-orders/{id}/payments",
            get(get_po_payments),
        );

    let writes = Router::new()
        .route("/api/v1/suppliers", post(create_supplier))
        .route("/api/v1/purchase-orders", post(create_purchase_order))
        .route(
            "/api/v1/purchase-orders/{id}/receive",
            post(receive_purchase_order),
        )
        .route(
            "/api/v1/purchase-orders/{id}/payments",
            post(create_po_payment),
        )
        .route(
            "/api/v1/suppliers/{id}",
            axum::routing::patch(update_supplier).delete(delete_supplier),
        )
        .route("/api/v1/suppliers/{id}/map-product", post(map_product))
        .route("/api/v1/supplier-prices", post(create_price))
        .route("/api/v1/supplier-prices/compare", post(compare_prices))
        .route("/api/v1/supplier-prices/import", post(import_prices))
        .route_layer(crate::role::layer(state, WRITE_ROLES));

    reads.merge(writes)
}

// --- suppliers: reads ------------------------------------------------------

async fn list_suppliers(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<SupplierFilters>,
) -> Result<Json<Vec<SupplierDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_suppliers(&db, &t, filters).await?))
}

async fn get_supplier(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<SupplierDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_supplier(&db, &t, &id).await?))
}

// --- suppliers: writes -----------------------------------------------------

async fn create_supplier(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewSupplier>,
) -> Result<Json<SupplierDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_supplier(&db, &t, input).await?))
}

async fn update_supplier(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(patch): Json<UpdateSupplier>,
) -> Result<Json<SupplierDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::update_supplier(&db, &t, &id, patch).await?))
}

async fn delete_supplier(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    service::delete_supplier(&db, &t, &id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn map_product(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(input): Json<MapSupplierProduct>,
) -> Result<Json<SupplierProductMappingDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::map_product(&db, &t, &id, input).await?))
}

// --- prices ----------------------------------------------------------------

async fn list_prices(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<SupplierPriceFilters>,
) -> Result<Json<Vec<SupplierPriceDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_prices(&db, &t, filters).await?))
}

async fn create_price(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewSupplierPrice>,
) -> Result<Json<SupplierPriceDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_price(&db, &t, input).await?))
}

async fn compare_prices(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<CompareRequest>,
) -> Result<Json<CompareResponse>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::compare(&db, &t, req).await?))
}

// --- purchase orders -------------------------------------------------------

async fn list_purchase_orders(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(filters): Query<PurchaseOrderFilters>,
) -> Result<Json<Vec<PurchaseOrderDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::list_purchase_orders(&db, &t, filters).await?))
}

async fn get_purchase_order(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<PurchaseOrderDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::get_purchase_order(&db, &t, &id).await?))
}

async fn create_purchase_order(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(input): Json<NewPurchaseOrder>,
) -> Result<Json<PurchaseOrderDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(service::create_purchase_order(&db, &t, input).await?))
}

async fn receive_purchase_order(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<PurchaseOrderDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(
        service::receive_purchase_order(&db, &t, &id, Some(&claims.sub)).await?,
    ))
}

async fn get_po_payments(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<PurchasePaymentSummary>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(
        service::get_purchase_payment_summary(&db, &t, &id).await?,
    ))
}

async fn create_po_payment(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(input): Json<NewPurchasePayment>,
) -> Result<Json<PurchasePaymentDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    Ok(Json(
        service::create_purchase_payment(&db, &t, &id, input, Some(&claims.sub)).await?,
    ))
}

// --- CSV import ------------------------------------------------------------

#[derive(Deserialize)]
struct ImportQuery {
    /// Default supplier for rows that omit `supplier`. `supplier:xxx`.
    supplier: Option<String>,
}

#[derive(Serialize)]
struct ImportSummary {
    created: usize,
    failed: usize,
    errors: Vec<ImportError>,
}

#[derive(Serialize)]
struct ImportError {
    line: usize,
    message: String,
}

/// CSV columns (header-based, case-insensitive):
/// `supplier` (optional if `?supplier=...`), `supplier_code`, `product`,
/// `description`, `unit_cost` (required), `currency`, `valid_from`.
async fn import_prices(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(qs): Query<ImportQuery>,
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
    let Some(i_cost) = col("unit_cost") else {
        return Err(ApiError::invalid("CSV debe incluir la columna `unit_cost`"));
    };
    let i_supplier = col("supplier");
    let i_code = col("supplier_code");
    let i_product = col("product");
    let i_desc = col("description");
    let i_currency = col("currency");
    if i_supplier.is_none() && qs.supplier.is_none() {
        return Err(ApiError::invalid(
            "indique columna `supplier` o el query `?supplier=...`",
        ));
    }
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
        let supplier = get(&rec, i_supplier)
            .or_else(|| qs.supplier.clone())
            .unwrap_or_default();
        let unit_cost = match rec.get(i_cost).unwrap_or("").trim().parse() {
            Ok(v) => v,
            Err(_) => {
                summary.failed += 1;
                summary.errors.push(ImportError {
                    line,
                    message: "unit_cost inválido".into(),
                });
                continue;
            }
        };
        let input = NewSupplierPrice {
            supplier,
            product: get(&rec, i_product),
            supplier_code: get(&rec, i_code),
            description: get(&rec, i_desc),
            unit_cost,
            currency: get(&rec, i_currency),
            valid_from: None,
        };
        match service::create_price(&db, &t, input).await {
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

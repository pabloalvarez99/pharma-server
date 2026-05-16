//! Purchasing persistence. Pure async fns over `&Db`. Every query is
//! tenant-scoped: callers pass the tenant `Thing` from the JWT claim.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{Map, Value};
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::*;

// --- DB rows ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SupplierRow {
    id: Thing,
    name: String,
    rut: Option<String>,
    contact_name: Option<String>,
    contact_email: Option<String>,
    contact_phone: Option<String>,
    default_invoice_format: Option<String>,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<SupplierRow> for SupplierDto {
    fn from(r: SupplierRow) -> Self {
        Self {
            id: r.id.to_string(),
            name: r.name,
            rut: r.rut,
            contact_name: r.contact_name,
            contact_email: r.contact_email,
            contact_phone: r.contact_phone,
            default_invoice_format: r.default_invoice_format,
            active: r.active,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct MappingRow {
    id: Thing,
    supplier: Thing,
    product: Thing,
    supplier_code: String,
    created_at: DateTime<Utc>,
}

impl From<MappingRow> for SupplierProductMappingDto {
    fn from(r: MappingRow) -> Self {
        Self {
            id: r.id.to_string(),
            supplier: r.supplier.to_string(),
            product: r.product.to_string(),
            supplier_code: r.supplier_code,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct PriceRow {
    id: Thing,
    supplier: Thing,
    product: Option<Thing>,
    supplier_code: Option<String>,
    description: Option<String>,
    unit_cost: Decimal,
    currency: String,
    valid_from: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl From<PriceRow> for SupplierPriceDto {
    fn from(r: PriceRow) -> Self {
        Self {
            id: r.id.to_string(),
            supplier: r.supplier.to_string(),
            product: r.product.map(|p| p.to_string()),
            supplier_code: r.supplier_code,
            description: r.description,
            unit_cost: r.unit_cost,
            currency: r.currency,
            valid_from: r.valid_from,
            created_at: r.created_at,
        }
    }
}

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// `rust_decimal` (`serde-with-str`) serializes as a JSON string; SurrealQL
/// `decimal` rejects that on bind. Convert to a native `Number::Decimal` so it
/// round-trips as a real decimal.
fn dec_val(d: Decimal) -> surrealdb::sql::Value {
    surrealdb::sql::Number::from(d).into()
}

// --- suppliers -------------------------------------------------------------

pub async fn supplier_belongs(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("SELECT id FROM supplier WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

pub async fn product_belongs(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("SELECT id FROM product WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

pub async fn create_supplier(
    db: &Db,
    tenant: &Thing,
    input: &NewSupplier,
) -> DomainResult<SupplierDto> {
    let mut r = db
        .query(
            "CREATE supplier SET tenant = $t, name = $name, rut = $rut, \
             contact_name = $contact_name, contact_email = $contact_email, \
             contact_phone = $contact_phone, \
             default_invoice_format = $default_invoice_format RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("name", input.name.clone()))
        .bind(("rut", input.rut.clone()))
        .bind(("contact_name", input.contact_name.clone()))
        .bind(("contact_email", input.contact_email.clone()))
        .bind(("contact_phone", input.contact_phone.clone()))
        .bind((
            "default_invoice_format",
            input.default_invoice_format.clone(),
        ))
        .await?;
    let row: Option<SupplierRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn list_suppliers(
    db: &Db,
    tenant: &Thing,
    f: &SupplierFilters,
) -> DomainResult<Vec<SupplierDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.search.is_some() {
        conds.push("(string::lowercase(name) CONTAINS $q OR rut = $raw_q)".to_string());
    }
    if f.active.is_some() {
        conds.push("active = $active".to_string());
    }
    let limit = f.limit.unwrap_or(100).min(500);
    let offset = f.offset.unwrap_or(0);
    let q = format!(
        "SELECT * FROM supplier WHERE {} ORDER BY name LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset
    );
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind((
            "q",
            f.search
                .as_deref()
                .map(|s| s.to_lowercase())
                .unwrap_or_default(),
        ))
        .bind(("raw_q", f.search.clone().unwrap_or_default()))
        .bind(("active", f.active.unwrap_or(true)))
        .await?;
    let rows: Vec<SupplierRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_supplier(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
) -> DomainResult<Option<SupplierDto>> {
    let mut r = db
        .query("SELECT * FROM supplier WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<SupplierRow> = r.take(0)?;
    Ok(row.map(Into::into))
}

pub async fn update_supplier(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
    patch: &UpdateSupplier,
) -> DomainResult<SupplierDto> {
    let mut m = Map::new();
    if let Some(v) = &patch.name {
        m.insert("name".into(), Value::String(v.clone()));
    }
    if let Some(v) = &patch.rut {
        m.insert("rut".into(), Value::String(v.clone()));
    }
    if let Some(v) = &patch.contact_name {
        m.insert("contact_name".into(), Value::String(v.clone()));
    }
    if let Some(v) = &patch.contact_email {
        m.insert("contact_email".into(), Value::String(v.clone()));
    }
    if let Some(v) = &patch.contact_phone {
        m.insert("contact_phone".into(), Value::String(v.clone()));
    }
    if let Some(v) = &patch.default_invoice_format {
        m.insert("default_invoice_format".into(), Value::String(v.clone()));
    }
    if let Some(v) = patch.active {
        m.insert("active".into(), Value::Bool(v));
    }
    let mut r = db
        .query("UPDATE supplier MERGE $p WHERE id = $id AND tenant = $t RETURN AFTER")
        .bind(("p", Value::Object(m)))
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<SupplierRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

/// Soft delete: `active = false`. Preserves referential integrity for
/// price-lists / mappings / future purchase orders.
pub async fn soft_delete_supplier(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("UPDATE supplier SET active = false WHERE id = $id AND tenant = $t RETURN AFTER")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

// --- mappings --------------------------------------------------------------

pub async fn create_mapping(
    db: &Db,
    tenant: &Thing,
    supplier: &Thing,
    product: &Thing,
    supplier_code: &str,
) -> DomainResult<SupplierProductMappingDto> {
    let mut r = db
        .query(
            "CREATE supplier_product_mapping SET tenant = $t, supplier = $s, \
             product = $p, supplier_code = $code RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("s", supplier.clone()))
        .bind(("p", product.clone()))
        .bind(("code", supplier_code.to_string()))
        .await?;
    let row: Option<MappingRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

/// Resolve `(supplier, supplier_code)` → product Thing via existing mapping.
pub async fn resolve_mapping_product(
    db: &Db,
    tenant: &Thing,
    supplier: &Thing,
    supplier_code: &str,
) -> DomainResult<Option<Thing>> {
    let mut r = db
        .query(
            "SELECT product FROM supplier_product_mapping \
             WHERE tenant = $t AND supplier = $s AND supplier_code = $code LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("s", supplier.clone()))
        .bind(("code", supplier_code.to_string()))
        .await?;
    let row: Option<Thing> = r.take((0, "product"))?;
    Ok(row)
}

// --- price lists -----------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub async fn create_price(
    db: &Db,
    tenant: &Thing,
    supplier: &Thing,
    product: Option<Thing>,
    supplier_code: Option<&str>,
    description: Option<&str>,
    unit_cost: Decimal,
    currency: &str,
    valid_from: Option<DateTime<Utc>>,
) -> DomainResult<SupplierPriceDto> {
    let mut r = db
        .query(
            "CREATE supplier_price_list SET tenant = $t, supplier = $s, \
             product = $p, supplier_code = $code, description = $description, \
             unit_cost = $unit_cost, currency = $currency, \
             valid_from = $valid_from RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("s", supplier.clone()))
        .bind(("p", product))
        .bind(("code", supplier_code.map(str::to_string)))
        .bind(("description", description.map(str::to_string)))
        .bind(("unit_cost", dec_val(unit_cost)))
        .bind(("currency", currency.to_string()))
        .bind((
            "valid_from",
            surrealdb::sql::Datetime::from(valid_from.unwrap_or_else(Utc::now)),
        ))
        .await?;
    let row: Option<PriceRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn list_prices(
    db: &Db,
    tenant: &Thing,
    f: &SupplierPriceFilters,
) -> DomainResult<Vec<SupplierPriceDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.supplier.is_some() {
        conds.push("supplier = $s".to_string());
    }
    if f.product.is_some() {
        conds.push("product = $p".to_string());
    }
    if f.supplier_code.is_some() {
        conds.push("supplier_code = $code".to_string());
    }
    let limit = f.limit.unwrap_or(100).min(500);
    let offset = f.offset.unwrap_or(0);
    let q = format!(
        "SELECT * FROM supplier_price_list WHERE {} ORDER BY created_at DESC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset
    );
    let supplier = f
        .supplier
        .as_deref()
        .and_then(|s| surrealdb::sql::thing(s).ok());
    let product = f
        .product
        .as_deref()
        .and_then(|s| surrealdb::sql::thing(s).ok());
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("s", supplier))
        .bind(("p", product))
        .bind(("code", f.supplier_code.clone().unwrap_or_default()))
        .await?;
    let rows: Vec<PriceRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

async fn supplier_name(db: &Db, supplier: &Thing) -> DomainResult<String> {
    let mut r = db
        .query("SELECT name FROM $s LIMIT 1")
        .bind(("s", supplier.clone()))
        .await?;
    let name: Option<String> = r.take((0, "name"))?;
    Ok(name.unwrap_or_default())
}

/// Best (lowest) `unit_cost` price-list row for a product in this tenant,
/// paired with supplier name.
pub async fn best_price_for_product(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
) -> DomainResult<Option<(SupplierPriceDto, String)>> {
    let mut r = db
        .query(
            "SELECT * FROM supplier_price_list \
             WHERE tenant = $t AND product = $p \
             ORDER BY unit_cost ASC LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", product.clone()))
        .await?;
    let row: Option<PriceRow> = r.take(0)?;
    match row {
        Some(p) => {
            let name = supplier_name(db, &p.supplier).await?;
            Ok(Some((p.into(), name)))
        }
        None => Ok(None),
    }
}

/// Best (lowest) `unit_cost` price-list row matching a free supplier_code
/// (any supplier) for this tenant, paired with supplier name.
pub async fn best_price_for_code(
    db: &Db,
    tenant: &Thing,
    supplier_code: &str,
) -> DomainResult<Option<(SupplierPriceDto, String)>> {
    let mut r = db
        .query(
            "SELECT * FROM supplier_price_list \
             WHERE tenant = $t AND supplier_code = $code \
             ORDER BY unit_cost ASC LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("code", supplier_code.to_string()))
        .await?;
    let row: Option<PriceRow> = r.take(0)?;
    match row {
        Some(p) => {
            let name = supplier_name(db, &p.supplier).await?;
            Ok(Some((p.into(), name)))
        }
        None => Ok(None),
    }
}

/// Fetch a product's current `cost_price` (if any) — used by compare to
/// compute savings vs the best offer.
pub async fn product_cost_price(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
) -> DomainResult<Option<Decimal>> {
    let mut r = db
        .query("SELECT cost_price FROM product WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", product.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Option<Decimal>> = r.take((0, "cost_price"))?;
    Ok(row.flatten())
}

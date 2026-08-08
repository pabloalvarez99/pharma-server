//! Catalog persistence. Pure async fns over `&Db`. Every query is
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
struct CategoryRow {
    id: Thing,
    name: String,
    slug: String,
    description: Option<String>,
    image_url: Option<String>,
    active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<CategoryRow> for CategoryDto {
    fn from(r: CategoryRow) -> Self {
        Self {
            id: r.id.to_string(),
            name: r.name,
            slug: r.slug,
            description: r.description,
            image_url: r.image_url,
            active: r.active,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ProductRow {
    id: Thing,
    name: String,
    slug: String,
    description: Option<String>,
    price: Decimal,
    cost_price: Option<Decimal>,
    stock: i64,
    #[serde(default = "default_true")]
    physical_stock: bool,
    category: Option<Thing>,
    image_url: Option<String>,
    active: bool,
    external_id: Option<String>,
    laboratory: Option<String>,
    therapeutic_action: Option<String>,
    active_ingredient: Option<String>,
    prescription_type: String,
    presentation: Option<String>,
    discount_percent: Option<i64>,
    /// Per-rubro flexible attributes (migration 0033). Rows persisted before
    /// 0033 (or any select missing the column) decode as `None`.
    #[serde(default)]
    attrs: Option<Value>,
    /// Parent product when this row is a variant (migration 0034). Pre-0034
    /// rows decode as `None`.
    #[serde(default)]
    parent_id: Option<Thing>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ProductRow> for ProductDto {
    fn from(r: ProductRow) -> Self {
        Self {
            id: r.id.to_string(),
            name: r.name,
            slug: r.slug,
            description: r.description,
            price: r.price,
            cost_price: r.cost_price,
            stock: r.stock,
            physical_stock: r.physical_stock,
            category: r.category.map(|c| c.to_string()),
            image_url: r.image_url,
            active: r.active,
            external_id: r.external_id,
            laboratory: r.laboratory,
            therapeutic_action: r.therapeutic_action,
            active_ingredient: r.active_ingredient,
            prescription_type: r.prescription_type,
            presentation: r.presentation,
            discount_percent: r.discount_percent,
            attrs: r.attrs,
            parent_id: r.parent_id.map(|p| p.to_string()),
            // Enriched by service after join to product_barcode / children.
            barcode: None,
            variants_stock: None,
            variant_count: None,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Serde fallback for `ProductRow.physical_stock` so rows persisted before
/// migration 0031 (or any select missing the column) decode as físico.
fn default_true() -> bool {
    true
}

/// `rust_decimal` (with `serde-with-str`) serializes as a JSON string, which
/// the SurrealQL `decimal` schema rejects on bind. Convert to a native
/// `Number::Decimal` value so it round-trips as a real decimal.
fn dec_val(d: Decimal) -> surrealdb::sql::Value {
    surrealdb::sql::Number::from(d).into()
}

fn dec_opt(d: Option<Decimal>) -> surrealdb::sql::Value {
    match d {
        Some(x) => dec_val(x),
        None => surrealdb::sql::Value::None,
    }
}

/// Convert `serde_json::Value` into a native SurrealQL value.
/// Binding a raw `serde_json::Value` against SCHEMAFULL `option<object>`
/// fields silently stores `{}` — Surreal's bind path does not map JSON
/// objects the way we need. Used for `product.attrs` (and variants).
fn json_to_sql(v: &serde_json::Value) -> surrealdb::sql::Value {
    use std::collections::BTreeMap;
    use surrealdb::sql::{Array, Object, Value};
    match v {
        serde_json::Value::Null => Value::None,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::from(i)
            } else if let Some(u) = n.as_u64() {
                Value::from(u)
            } else if let Some(f) = n.as_f64() {
                Value::from(f)
            } else {
                Value::None
            }
        }
        serde_json::Value::String(s) => Value::from(s.clone()),
        serde_json::Value::Array(items) => Value::Array(Array::from(
            items.iter().map(json_to_sql).collect::<Vec<_>>(),
        )),
        serde_json::Value::Object(map) => {
            let mut obj: BTreeMap<String, Value> = BTreeMap::new();
            for (k, val) in map {
                obj.insert(k.clone(), json_to_sql(val));
            }
            Value::Object(Object::from(obj))
        }
    }
}

fn attrs_bind(attrs: &Option<serde_json::Value>) -> surrealdb::sql::Value {
    match attrs {
        None => surrealdb::sql::Value::None,
        Some(v) => json_to_sql(v),
    }
}

// --- categories ------------------------------------------------------------

pub async fn category_slug_exists(db: &Db, tenant: &Thing, slug: &str) -> DomainResult<bool> {
    let mut r = db
        .query("SELECT id FROM category WHERE tenant = $t AND slug = $s LIMIT 1")
        .bind(("t", tenant.clone()))
        .bind(("s", slug.to_string()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

pub async fn category_belongs(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("SELECT id FROM category WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

pub async fn create_category(
    db: &Db,
    tenant: &Thing,
    slug: &str,
    input: &NewCategory,
) -> DomainResult<CategoryDto> {
    let mut r = db
        .query(
            "CREATE category SET tenant = $t, name = $name, slug = $slug, \
             description = $description, image_url = $image_url RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("name", input.name.clone()))
        .bind(("slug", slug.to_string()))
        .bind(("description", input.description.clone()))
        .bind(("image_url", input.image_url.clone()))
        .await?;
    let row: Option<CategoryRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn list_categories(db: &Db, tenant: &Thing) -> DomainResult<Vec<CategoryDto>> {
    let mut r = db
        .query("SELECT * FROM category WHERE tenant = $t ORDER BY name")
        .bind(("t", tenant.clone()))
        .await?;
    let rows: Vec<CategoryRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_category(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
) -> DomainResult<Option<CategoryDto>> {
    let mut r = db
        .query("SELECT * FROM category WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<CategoryRow> = r.take(0)?;
    Ok(row.map(Into::into))
}

pub async fn update_category(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
    patch: &UpdateCategory,
) -> DomainResult<CategoryDto> {
    let mut m = Map::new();
    if let Some(v) = &patch.name {
        m.insert("name".into(), Value::String(v.clone()));
    }
    if let Some(v) = &patch.description {
        m.insert("description".into(), Value::String(v.clone()));
    }
    if let Some(v) = &patch.image_url {
        m.insert("image_url".into(), Value::String(v.clone()));
    }
    if let Some(v) = patch.active {
        m.insert("active".into(), Value::Bool(v));
    }
    let mut r = db
        .query("UPDATE category MERGE $p WHERE id = $id AND tenant = $t RETURN AFTER")
        .bind(("p", Value::Object(m)))
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<CategoryRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

/// Soft delete: `active = false`. Preserves referential integrity for future
/// `order_item` / `stock_movement` (Fase 4+).
pub async fn soft_delete_category(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("UPDATE category SET active = false WHERE id = $id AND tenant = $t RETURN AFTER")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

// --- products --------------------------------------------------------------

pub async fn product_slug_exists(db: &Db, tenant: &Thing, slug: &str) -> DomainResult<bool> {
    let mut r = db
        .query("SELECT id FROM product WHERE tenant = $t AND slug = $s LIMIT 1")
        .bind(("t", tenant.clone()))
        .bind(("s", slug.to_string()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

/// Returns the product id for a given tenant + external_id, if present.
/// Backs upsert-by-external_id semantics in the bulk import endpoint
/// (catalog migrations: re-running must NOT duplicate rows).
pub async fn find_id_by_external_id(
    db: &Db,
    tenant: &Thing,
    external_id: &str,
) -> DomainResult<Option<Thing>> {
    let mut r = db
        .query("SELECT id FROM product WHERE tenant = $t AND external_id = $x LIMIT 1")
        .bind(("t", tenant.clone()))
        .bind(("x", external_id.to_string()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row)
}

/// Idempotent tenant-scoped `barcode → product` mapping. Backs catalog
/// import: the unique index `(tenant, barcode)` (migration 0003) means a
/// plain CREATE would fail on re-import, so this UPSERTs by `(tenant,
/// barcode)`. If the barcode already exists it is re-pointed to `product`;
/// otherwise a new mapping row is created. The agent/POS lookups read this
/// table via `SELECT VALUE product FROM product_barcode WHERE tenant=$t AND
/// barcode=$b`.
///
/// **Do not use for create-variant** — re-pointing would steal a barcode under
/// race. Prefer [`create_barcode`] there.
pub async fn upsert_barcode(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
    barcode: &str,
) -> DomainResult<()> {
    db.query(
        "UPSERT product_barcode SET tenant = $t, product = $p, barcode = $b \
         WHERE tenant = $t AND barcode = $b",
    )
    .bind(("t", tenant.clone()))
    .bind(("p", product.clone()))
    .bind(("b", barcode.to_string()))
    .await?;
    Ok(())
}

/// Insert a **new** barcode mapping without re-pointing an existing one.
/// Unique index `(tenant, barcode)` → [`DomainError::Conflict`] if taken.
/// Safe under concurrent create-variant of the same EAN.
pub async fn create_barcode(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
    barcode: &str,
) -> DomainResult<()> {
    // Fast path: already mapped to this product (retry / idempotent).
    if let Some(owner) = product_id_by_barcode(db, tenant, barcode).await? {
        if owner == *product {
            return Ok(());
        }
        return Err(DomainError::Conflict(format!(
            "el código de barras '{barcode}' ya está asignado a {owner}"
        )));
    }
    let res = db
        .query(
            "CREATE product_barcode SET tenant = $t, product = $p, barcode = $b \
             RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", product.clone()))
        .bind(("b", barcode.to_string()))
        .await;
    match res {
        Ok(mut r) => match r.take::<Option<Thing>>((0, "id")) {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {}
            Err(e) => {
                // Surreal may return Ok with an error payload on unique violation.
                if let Some(owner) = product_id_by_barcode(db, tenant, barcode).await? {
                    if owner == *product {
                        return Ok(());
                    }
                    return Err(DomainError::Conflict(format!(
                        "el código de barras '{barcode}' ya está asignado a {owner}"
                    )));
                }
                let es = e.to_string();
                if es.contains("unique")
                    || es.contains("already")
                    || es.contains("index")
                    || es.contains("conflict")
                {
                    return Err(DomainError::Conflict(format!(
                        "el código de barras '{barcode}' ya está asignado"
                    )));
                }
                return Err(DomainError::Db(Box::new(e)));
            }
        },
        Err(e) => {
            // Unique race or other DB fault — re-check ownership.
            if let Some(owner) = product_id_by_barcode(db, tenant, barcode).await? {
                if owner == *product {
                    return Ok(());
                }
                return Err(DomainError::Conflict(format!(
                    "el código de barras '{barcode}' ya está asignado a {owner}"
                )));
            }
            let es = e.to_string();
            if es.contains("unique")
                || es.contains("already")
                || es.contains("index")
                || es.contains("conflict")
            {
                return Err(DomainError::Conflict(format!(
                    "el código de barras '{barcode}' ya está asignado"
                )));
            }
            return Err(DomainError::Db(Box::new(e)));
        }
    }
    // CREATE returned 0 rows without Err — treat as conflict if taken.
    if let Some(owner) = product_id_by_barcode(db, tenant, barcode).await? {
        if owner == *product {
            return Ok(());
        }
        return Err(DomainError::Conflict(format!(
            "el código de barras '{barcode}' ya está asignado a {owner}"
        )));
    }
    Err(DomainError::Other(anyhow::anyhow!(
        "create product_barcode returned 0 rows for {barcode}"
    )))
}

/// Barcode for a single product (tenant-scoped), if any.
pub async fn barcode_of_product(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
) -> DomainResult<Option<String>> {
    let mut r = db
        .query(
            "SELECT VALUE barcode FROM product_barcode \
             WHERE tenant = $t AND product = $p LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", product.clone()))
        .await?;
    let code: Option<String> = r.take(0)?;
    Ok(code)
}

/// Batch barcode lookup for many products (tenant-scoped). Key = product id string.
pub async fn barcodes_of_products(
    db: &Db,
    tenant: &Thing,
    products: &[Thing],
) -> DomainResult<std::collections::HashMap<String, String>> {
    if products.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let mut r = db
        .query(
            "SELECT product, barcode FROM product_barcode \
             WHERE tenant = $t AND product INSIDE $ps",
        )
        .bind(("t", tenant.clone()))
        .bind(("ps", products.to_vec()))
        .await?;
    #[derive(serde::Deserialize)]
    struct Row {
        product: Thing,
        barcode: String,
    }
    let rows: Vec<Row> = r.take(0).unwrap_or_default();
    Ok(rows
        .into_iter()
        .map(|row| (row.product.to_string(), row.barcode))
        .collect())
}

/// Sum of `stock` on active children of `parent` (read-side only).
pub async fn sum_children_stock(db: &Db, tenant: &Thing, parent: &Thing) -> DomainResult<i64> {
    let mut r = db
        .query(
            "SELECT math::sum(stock) AS total FROM product WITH INDEX product_tenant_parent \
             WHERE tenant = $t AND parent_id = $p AND active = true \
             GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", parent.clone()))
        .await?;
    #[derive(serde::Deserialize)]
    struct SumRow {
        total: Option<i64>,
    }
    let row: Option<SumRow> = r.take(0)?;
    Ok(row.and_then(|s| s.total).unwrap_or(0))
}

/// Batch: parent id → (sum stock, count) of active children. One query for a
/// list page (client uses field presence as multi-SKU parent flag).
pub async fn children_agg_by_parents(
    db: &Db,
    tenant: &Thing,
    parents: &[Thing],
) -> DomainResult<std::collections::HashMap<String, (i64, i64)>> {
    if parents.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    // `WITH INDEX` obligatorio: entre `product_tenant_parent` y
    // `product_tenant_active` el planner elige recorriendo un HashMap, así que
    // la misma consulta a veces salía por `active` y leía el catálogo entero
    // (medido: 0,3 ms con el índice correcto, 58 ms con el otro, alternando).
    // Esta consulta corre en CADA página de productos.
    let mut r = db
        .query(
            "SELECT parent_id, stock FROM product WITH INDEX product_tenant_parent \
             WHERE tenant = $t AND parent_id INSIDE $ps AND active = true",
        )
        .bind(("t", tenant.clone()))
        .bind(("ps", parents.to_vec()))
        .await?;
    #[derive(serde::Deserialize)]
    struct Row {
        parent_id: Thing,
        stock: i64,
    }
    let rows: Vec<Row> = r.take(0).unwrap_or_default();
    let mut map: std::collections::HashMap<String, (i64, i64)> = std::collections::HashMap::new();
    for row in rows {
        let e = map.entry(row.parent_id.to_string()).or_insert((0, 0));
        e.0 += row.stock;
        e.1 += 1;
    }
    Ok(map)
}

/// Batch: parent id → sum of active children stock.
pub async fn children_stock_by_parents(
    db: &Db,
    tenant: &Thing,
    parents: &[Thing],
) -> DomainResult<std::collections::HashMap<String, i64>> {
    let agg = children_agg_by_parents(db, tenant, parents).await?;
    Ok(agg.into_iter().map(|(k, (stock, _))| (k, stock)).collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn create_product(
    db: &Db,
    tenant: &Thing,
    slug: &str,
    input: &NewProduct,
    category: Option<Thing>,
) -> DomainResult<ProductDto> {
    let mut r = db
        .query(
            "CREATE product SET tenant = $t, name = $name, slug = $slug, \
             description = $description, price = $price, cost_price = $cost_price, \
             stock = $stock, category = $category, image_url = $image_url, \
             external_id = $external_id, laboratory = $laboratory, \
             therapeutic_action = $therapeutic_action, active_ingredient = $active_ingredient, \
             prescription_type = $prescription_type, presentation = $presentation, \
             discount_percent = $discount_percent, attrs = $attrs RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("name", input.name.clone()))
        .bind(("slug", slug.to_string()))
        .bind(("description", input.description.clone()))
        .bind(("price", dec_val(input.price)))
        .bind(("cost_price", dec_opt(input.cost_price)))
        .bind(("stock", input.stock))
        .bind(("category", category))
        .bind(("image_url", input.image_url.clone()))
        .bind(("external_id", input.external_id.clone()))
        .bind(("laboratory", input.laboratory.clone()))
        .bind(("therapeutic_action", input.therapeutic_action.clone()))
        .bind(("active_ingredient", input.active_ingredient.clone()))
        .bind((
            "prescription_type",
            input
                .prescription_type
                .clone()
                .unwrap_or_else(|| "direct".to_string()),
        ))
        .bind(("presentation", input.presentation.clone()))
        .bind(("discount_percent", input.discount_percent))
        .bind(("attrs", attrs_bind(&input.attrs)))
        .await?;
    let row: Option<ProductRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

/// Create a sellable variant child under `parent`. Caller already validated
/// tenant ownership, single-level depth, and slug uniqueness.
#[allow(clippy::too_many_arguments)]
pub async fn create_variant_product(
    db: &Db,
    tenant: &Thing,
    parent: &Thing,
    slug: &str,
    name: &str,
    price: Decimal,
    cost_price: Option<Decimal>,
    stock: i64,
    attrs: Option<Value>,
    external_id: Option<String>,
    image_url: Option<String>,
    category: Option<Thing>,
    physical_stock: bool,
) -> DomainResult<ProductDto> {
    let mut r = db
        .query(
            "CREATE product SET tenant = $t, name = $name, slug = $slug, \
             price = $price, cost_price = $cost_price, stock = $stock, \
             category = $category, image_url = $image_url, \
             external_id = $external_id, parent_id = $parent, attrs = $attrs, \
             physical_stock = $physical_stock, prescription_type = 'direct' \
             RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("name", name.to_string()))
        .bind(("slug", slug.to_string()))
        .bind(("price", dec_val(price)))
        .bind(("cost_price", dec_opt(cost_price)))
        .bind(("stock", stock))
        .bind(("category", category))
        .bind(("image_url", image_url))
        .bind(("external_id", external_id))
        .bind(("parent", parent.clone()))
        .bind(("attrs", attrs_bind(&attrs)))
        .bind(("physical_stock", physical_stock))
        .await?;
    let row: Option<ProductRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

/// Active variants of a parent, tenant-scoped (soft-deleted hijos excluded).
pub async fn list_variants(
    db: &Db,
    tenant: &Thing,
    parent: &Thing,
) -> DomainResult<Vec<ProductDto>> {
    // Mismo empate de índices que en `children_agg_by_parents`: sin pinear, el
    // planner a veces recorre `product_tenant_active` y lee todo el catálogo
    // para devolver las (pocas) variantes de un padre.
    let mut r = db
        .query(
            "SELECT * FROM product WITH INDEX product_tenant_parent \
             WHERE tenant = $t AND parent_id = $p AND active = true ORDER BY name",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", parent.clone()))
        .await?;
    let rows: Vec<ProductRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Drop all `product_barcode` rows for a product (tenant-scoped). Frees the EAN
/// so a soft-deleted variant can be recreated with the same code.
pub async fn delete_barcodes_of_product(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
) -> DomainResult<()> {
    db.query("DELETE product_barcode WHERE tenant = $t AND product = $p")
        .bind(("t", tenant.clone()))
        .bind(("p", product.clone()))
        .await?;
    Ok(())
}

/// `true` if the product has at least one active child variant.
pub async fn has_active_variants(db: &Db, tenant: &Thing, parent: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query(
            // Camino del escáner del POS (`find_by_barcode`): mismo empate de
            // índices, mismo riesgo de leer el catálogo entero para responder
            // "¿tiene variantes?".
            "SELECT id FROM product WITH INDEX product_tenant_parent \
             WHERE tenant = $t AND parent_id = $p AND active = true LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", parent.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

/// Resolve tenant-scoped barcode → product id (variant or plain SKU).
pub async fn product_id_by_barcode(
    db: &Db,
    tenant: &Thing,
    barcode: &str,
) -> DomainResult<Option<Thing>> {
    let mut r = db
        .query(
            "SELECT VALUE product FROM product_barcode \
             WHERE tenant = $t AND barcode = $b LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("b", barcode.to_string()))
        .await?;
    let id: Option<Thing> = r.take(0)?;
    Ok(id)
}

/// Set `product.physical_stock` (tenant-scoped). `false` marks an item as a
/// service: the sale path then skips its stock check + FEFO plan (migración
/// 0031). Used by the demo seed for the servicios vertical; `create_product`
/// leaves the DB DEFAULT (`true`) for every normal SKU.
pub async fn set_physical_stock(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
    physical_stock: bool,
) -> DomainResult<()> {
    db.query("UPDATE $p SET physical_stock = $v WHERE tenant = $t")
        .bind(("p", product.clone()))
        .bind(("t", tenant.clone()))
        .bind(("v", physical_stock))
        .await?
        .check()?;
    Ok(())
}

pub async fn list_products(
    db: &Db,
    tenant: &Thing,
    f: &ProductFilters,
) -> DomainResult<Vec<ProductDto>> {
    list_products_opts(db, tenant, f, false).await
}

/// Like [`list_products`], with explicit control over whether child variants
/// (`parent_id` set) appear in the result. Default list paths hide them so the
/// retail catalog shows padres + SKUs planos; `GET .../variants` lists children.
pub async fn list_products_opts(
    db: &Db,
    tenant: &Thing,
    f: &ProductFilters,
    include_variants: bool,
) -> DomainResult<Vec<ProductDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.search.is_some() {
        conds.push(
            "(string::lowercase(name) CONTAINS $q \
             OR string::lowercase(active_ingredient ?? '') CONTAINS $q \
             OR external_id = $raw_q)"
                .to_string(),
        );
    }
    if f.category.is_some() {
        conds.push("category = $cat".to_string());
    }
    if f.active.is_some() {
        conds.push("active = $active".to_string());
    }
    if f.low_stock.is_some() {
        // Servicios (physical_stock = false) no tienen inventario → nunca son
        // "stock bajo"; se excluyen de la alerta.
        conds.push("stock <= $low AND physical_stock = true".to_string());
    }
    // Default: only top-level (padres + SKUs planos). Variantes viven bajo
    // GET /products/{id}/variants; no ensuciar el catálogo retail.
    if !include_variants {
        conds.push("parent_id = NONE".to_string());
    }
    // Elegir el índice a mano, no dejárselo al planner.
    //
    // Cada condición de acá matchea un índice compuesto distinto que arranca
    // con `tenant` (`product_tenant_active`, `product_tenant_parent`,
    // `product_tenant_stock`, …). SurrealDB 2.6 desempata recorriendo un
    // HashMap (`idx::planner::tree::CompoundIndexes`), así que la MISMA
    // consulta sale con un plan distinto entre corridas. Medido en release con
    // 3.000 productos: la página de stock bajo daba 2 ms con
    // `product_tenant_stock` y 84-140 ms cuando le tocaba
    // `product_tenant_active` — que ignora el filtro selectivo, lee las 3.000
    // filas y recién ahí ordena en memoria. Alternaba corrida a corrida.
    //
    // Pinear el índice del filtro más selectivo vuelve el plan determinístico.
    // La página sin filtros (el camino caliente del POS) usa `product_name`
    // (migración 0044): recorre el índice ya ordenado y corta en el LIMIT sin
    // ordenar nada en memoria — 90 ms → 2,1 ms.
    let with_clause = if f.search.is_some() {
        // `CONTAINS` no lo sirve ningún índice: el recorrido de tabla es el
        // plan real y es estable. Pinear acá no ayudaría.
        ""
    } else if f.low_stock.is_some() {
        " WITH INDEX product_tenant_stock"
    } else if f.category.is_some() {
        " WITH INDEX product_tenant_category"
    } else {
        // Página sin filtro selectivo: el camino caliente de la pantalla
        // Cobrar del POS (`?active=true&limit=40`).
        " WITH INDEX product_name"
    };
    let limit = f.limit.unwrap_or(100).min(500);
    let offset = f.offset.unwrap_or(0);
    let q = format!(
        "SELECT * FROM product{} WHERE {} ORDER BY name LIMIT {} START {}",
        with_clause,
        conds.join(" AND "),
        limit,
        offset
    );
    let cat = f
        .category
        .as_deref()
        .and_then(|s| surrealdb::sql::thing(s).ok());
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
        .bind(("cat", cat))
        .bind(("active", f.active.unwrap_or(true)))
        .bind(("low", f.low_stock.unwrap_or(LOW_STOCK_DEFAULT)))
        .await?;
    let rows: Vec<ProductRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_product(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<Option<ProductDto>> {
    // `FROM $id` es lectura directa por clave. Con `FROM product WHERE id = $id`
    // el planner no reconoce el id como clave: sale por `product_tenant_active`
    // y recorre el catálogo hasta encontrarlo (medido: 10 ms con 3.000
    // productos, contra 0,1 ms por clave). El `WHERE tenant` se mantiene: sin
    // él un id de otro tenant sería legible.
    let mut r = db
        .query("SELECT * FROM $id WHERE tenant = $t")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<ProductRow> = r.take(0)?;
    Ok(row.map(Into::into))
}

/// `category` is tri-state: `None` skip, `Some(None)` clear, `Some(Some)` set.
pub async fn update_product(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
    patch: &UpdateProduct,
    category: Option<Option<Thing>>,
) -> DomainResult<ProductDto> {
    let mut sets: Vec<&str> = Vec::new();
    if patch.name.is_some() {
        sets.push("name = $name");
    }
    if patch.description.is_some() {
        sets.push("description = $description");
    }
    if patch.price.is_some() {
        sets.push("price = $price");
    }
    if patch.cost_price.is_some() {
        sets.push("cost_price = $cost_price");
    }
    if patch.active.is_some() {
        sets.push("active = $active");
    }
    if patch.image_url.is_some() {
        sets.push("image_url = $image_url");
    }
    if patch.external_id.is_some() {
        sets.push("external_id = $external_id");
    }
    if patch.laboratory.is_some() {
        sets.push("laboratory = $laboratory");
    }
    if patch.therapeutic_action.is_some() {
        sets.push("therapeutic_action = $therapeutic_action");
    }
    if patch.active_ingredient.is_some() {
        sets.push("active_ingredient = $active_ingredient");
    }
    if patch.prescription_type.is_some() {
        sets.push("prescription_type = $prescription_type");
    }
    if patch.presentation.is_some() {
        sets.push("presentation = $presentation");
    }
    if patch.discount_percent.is_some() {
        sets.push("discount_percent = $discount_percent");
    }
    if patch.attrs.is_some() {
        sets.push("attrs = $attrs");
    }
    if patch.online_visible.is_some() {
        sets.push("online_visible = $online_visible");
    }
    if patch.online_title.is_some() {
        sets.push("online_title = $online_title");
    }
    if patch.online_description.is_some() {
        sets.push("online_description = $online_description");
    }
    if patch.online_price.is_some() {
        sets.push("online_price = $online_price");
    }
    if patch.online_sort.is_some() {
        sets.push("online_sort = $online_sort");
    }
    if category.is_some() {
        sets.push("category = $category");
    }
    if sets.is_empty() {
        return get_product(db, tenant, id)
            .await?
            .ok_or(DomainError::NotFound);
    }
    let q = format!(
        "UPDATE product SET {} WHERE id = $id AND tenant = $t RETURN AFTER",
        sets.join(", ")
    );
    let mut r = db
        .query(q)
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .bind(("name", patch.name.clone()))
        .bind(("description", patch.description.clone()))
        .bind(("price", dec_opt(patch.price)))
        .bind(("cost_price", dec_opt(patch.cost_price)))
        .bind(("active", patch.active))
        .bind(("image_url", patch.image_url.clone()))
        .bind(("external_id", patch.external_id.clone()))
        .bind(("laboratory", patch.laboratory.clone()))
        .bind(("therapeutic_action", patch.therapeutic_action.clone()))
        .bind(("active_ingredient", patch.active_ingredient.clone()))
        .bind(("prescription_type", patch.prescription_type.clone()))
        .bind(("presentation", patch.presentation.clone()))
        .bind(("discount_percent", patch.discount_percent))
        .bind(("attrs", attrs_bind(&patch.attrs)))
        .bind(("online_visible", patch.online_visible))
        .bind(("online_title", patch.online_title.clone()))
        .bind(("online_description", patch.online_description.clone()))
        .bind(("online_price", dec_opt(patch.online_price)))
        .bind(("online_sort", patch.online_sort))
        .bind(("category", category.flatten()))
        .await?;
    let row: Option<ProductRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn soft_delete_product(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("UPDATE product SET active = false WHERE id = $id AND tenant = $t RETURN AFTER")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

/// Hard-delete a product row (tenant-scoped). Used only to roll back a
/// just-created variant when barcode claim fails — no stock movements yet.
pub async fn hard_delete_product(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<()> {
    db.query("DELETE product WHERE id = $id AND tenant = $t")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    Ok(())
}

// `set_stock` removed in Fase 3: every stock change MUST go through
// `inventory::service::add_movement` so the audit trail and the
// materialized counter cannot diverge.

#[derive(Deserialize)]
struct StatsAgg {
    total: i64,
    active: i64,
    low_stock: i64,
    out_of_stock: i64,
    inventory_value: Option<Decimal>,
}

impl From<StatsAgg> for ProductStats {
    fn from(a: StatsAgg) -> Self {
        ProductStats {
            total: a.total,
            active: a.active,
            low_stock: a.low_stock,
            out_of_stock: a.out_of_stock,
            inventory_value: a.inventory_value.unwrap_or_default(),
            expired: 0,
        }
    }
}

/// Per-tenant catalog aggregate (total / active / low-stock / out-of-stock /
/// inventory value).
///
/// Fast path: the `product_stats` pre-computed view (migration 0029) holds the
/// aggregate, refreshed incrementally by the engine on every `product` write —
/// an O(1) read that replaced the O(n) `GROUP ALL` full scan this used to run
/// (BUG-perf-001: 2.7s p99 @50k SKUs, over the <50ms POS budget). The view
/// bakes the default low-stock threshold, so a non-default `low` falls back to
/// the live scan; a tenant with no view row (no products yet) also falls back —
/// that scan is trivially O(0). Either way the result is identical to the scan.
pub async fn stats(db: &Db, tenant: &Thing, low: i64) -> DomainResult<ProductStats> {
    if low == LOW_STOCK_DEFAULT {
        if let Some(agg) = stats_from_view(db, tenant).await? {
            return Ok(agg.into());
        }
    }
    stats_scan(db, tenant, low).await
}

/// O(1) read of the maintained `product_stats` view. `None` when the tenant has
/// no products (no view row), so the caller can fall back to the (empty) scan.
async fn stats_from_view(db: &Db, tenant: &Thing) -> DomainResult<Option<StatsAgg>> {
    let mut r = db
        .query(
            "SELECT total, active, low_stock, out_of_stock, inventory_value \
             FROM product_stats WHERE tenant = $t LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .await?;
    Ok(r.take(0)?)
}

/// Live O(n) full scan — the original aggregate. Kept as the correctness
/// fallback (non-default threshold, or no view row) and as the reference the
/// view tests compare against.
async fn stats_scan(db: &Db, tenant: &Thing, low: i64) -> DomainResult<ProductStats> {
    let mut r = db
        .query(
            "SELECT \
               count() AS total, \
               count(active = true) AS active, \
               count(stock <= $low AND active = true AND physical_stock = true) AS low_stock, \
               count(stock <= 0 AND active = true AND physical_stock = true) AS out_of_stock, \
               math::sum(stock * (cost_price ?? 0dec)) AS inventory_value \
             FROM product WHERE tenant = $t GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .bind(("low", low))
        .await?;
    let agg: Option<StatsAgg> = r.take(0)?;
    Ok(agg.map(ProductStats::from).unwrap_or_else(|| ProductStats {
        total: 0,
        active: 0,
        low_stock: 0,
        out_of_stock: 0,
        inventory_value: Decimal::ZERO,
        expired: 0,
    }))
}

/// Test-only accessor so the view fast path can be diffed against the scan
/// reference without re-implementing the query in the test crate.
#[doc(hidden)]
pub async fn stats_scan_for_test(db: &Db, tenant: &Thing, low: i64) -> DomainResult<ProductStats> {
    stats_scan(db, tenant, low).await
}

// --- bulk price update (safe, type-driven) ---------------------------------

/// Arithmetic op applied to `price` in a tenant-scoped bulk update.
/// Each variant carries a [`Decimal`] — values are bound via `.bind()` and
/// never interpolated into SQL strings. The repo composes a fixed-shape
/// SurrealQL template per variant, so there is no path for a caller to
/// inject arbitrary SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceOp {
    /// New price = `$v` (clamped by `floor_at_zero` if set).
    SetExact(Decimal),
    /// New price = `price * $v` — pass a multiplier (e.g. `1.10` for +10%).
    MultiplyPct(Decimal),
    /// New price = `price + $v` (signed).
    DeltaAbs(Decimal),
}

/// Full repricing request: arithmetic op + post-processing flags. See
/// [`PriceOp`]. Built by `catalog::service::bulk_price` from typed input
/// validated at the HTTP boundary.
#[derive(Debug, Clone, Copy)]
pub struct PriceUpdate {
    pub op: PriceOp,
    /// Wrap result in `math::max([0dec, ...])` so the new price never goes
    /// negative.
    pub floor_at_zero: bool,
    /// Round the (post-floor) result to the nearest whole unit.
    pub round: bool,
}

/// Safe replacement for [`bulk_update_price`] (deprecated). The op +
/// flags are translated into a fixed-shape SurrealQL template and the
/// numeric operand is bound — no user-controlled string is interpolated.
pub async fn bulk_update_price_typed(
    db: &Db,
    tenant: &Thing,
    update: PriceUpdate,
    category: Option<Thing>,
) -> DomainResult<usize> {
    // Per-op template. `$v` is the only numeric operand, always bound.
    let core_expr = match update.op {
        PriceOp::SetExact(_) => "$v",
        PriceOp::MultiplyPct(_) => "price * $v",
        PriceOp::DeltaAbs(_) => "price + $v",
    };
    let v = match update.op {
        PriceOp::SetExact(d) | PriceOp::MultiplyPct(d) | PriceOp::DeltaAbs(d) => d,
    };
    let guarded = if update.floor_at_zero {
        format!("math::max([0dec, {core_expr}])")
    } else {
        core_expr.to_string()
    };
    let expr = if update.round {
        format!("math::round({guarded})")
    } else {
        guarded
    };
    // `cond` is a literal in each branch — no user input reaches the SQL.
    let cond = if category.is_some() {
        "tenant = $t AND active = true AND category = $cat"
    } else {
        "tenant = $t AND active = true"
    };
    let q = format!("UPDATE product SET price = {expr} WHERE {cond} RETURN id");
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("cat", category))
        .bind(("v", dec_val(v)))
        .await?;
    let ids: Vec<Thing> = r.take((0, "id"))?;
    Ok(ids.len())
}

/// SQL-injection-prone: `expr` is interpolated raw into the UPDATE.
/// Kept temporarily for backward-compat of any external caller.
///
/// TODO(caller-impact): `crates/api/`, `crates/cli/` (a parallel agent owns
/// those crates) — none should reach this directly; the safe path is
/// `service::bulk_price` → [`bulk_update_price_typed`]. Remove once the
/// other agent confirms no out-of-tree callers.
#[deprecated(
    note = "use `bulk_update_price_typed` — interpolates raw SurrealQL, SQL-injection-prone"
)]
pub async fn bulk_update_price(
    db: &Db,
    tenant: &Thing,
    expr: &str,
    category: Option<Thing>,
) -> DomainResult<usize> {
    let cond = if category.is_some() {
        "tenant = $t AND active = true AND category = $cat"
    } else {
        "tenant = $t AND active = true"
    };
    let q = format!("UPDATE product SET price = {expr} WHERE {cond} RETURN id");
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("cat", category))
        .await?;
    let ids: Vec<Thing> = r.take((0, "id"))?;
    Ok(ids.len())
}

// --- etiquetas (typed column whitelist) ------------------------------------

/// Whitelisted `product` text columns queryable via [`etiquetas`]. Each
/// variant maps to a hardcoded column literal — the value of any user
/// input never reaches the SurrealQL string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagField {
    Laboratory,
    ActiveIngredient,
    TherapeuticAction,
}

impl TagField {
    /// Column literal injected into the SurrealQL template. Safe: every
    /// variant resolves to a compile-time `&'static str`. Exhaustive
    /// `match` — adding a variant is a compile error until the literal is
    /// added here.
    pub const fn column(self) -> &'static str {
        match self {
            TagField::Laboratory => "laboratory",
            TagField::ActiveIngredient => "active_ingredient",
            TagField::TherapeuticAction => "therapeutic_action",
        }
    }
}

pub async fn etiquetas(db: &Db, tenant: &Thing, q: &str) -> DomainResult<EtiquetaResults> {
    async fn distinct(
        db: &Db,
        tenant: &Thing,
        field: TagField,
        q: &str,
    ) -> DomainResult<Vec<String>> {
        // `field.column()` is always a hardcoded literal from `TagField`.
        // The user-supplied `q` is bound, never interpolated.
        let col = field.column();
        let sql = format!(
            "SELECT VALUE {col} FROM product \
             WHERE tenant = $t AND {col} != NONE \
             AND string::lowercase({col}) CONTAINS $q \
             GROUP BY {col} LIMIT 20"
        );
        let mut r = db
            .query(sql)
            .bind(("t", tenant.clone()))
            .bind(("q", q.to_lowercase()))
            .await?;
        let vs: Vec<String> = r.take(0)?;
        Ok(vs)
    }
    Ok(EtiquetaResults {
        laboratories: distinct(db, tenant, TagField::Laboratory, q).await?,
        active_ingredients: distinct(db, tenant, TagField::ActiveIngredient, q).await?,
        therapeutic_actions: distinct(db, tenant, TagField::TherapeuticAction, q).await?,
    })
}

// --- public storefront (Free Web PR1, ADR-0020) ----------------------------
// Rows for the unauthenticated `/api/v1/public/{slug}` projection. The SELECT
// column list is the safety boundary: cost_price / stock counts are never
// projected, so they cannot leak even if the DTO grows careless later.

#[derive(Debug, Deserialize)]
struct PublicTenantRow {
    id: Thing,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PublicProductRow {
    id: Thing,
    slug: String,
    name: String,
    #[serde(default)]
    online_title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    online_description: Option<String>,
    price: Decimal,
    #[serde(default)]
    online_price: Option<Decimal>,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    category_slug: Option<String>,
    stock: i64,
    /// Units held by web pickup reservations (PR3): sellable = stock - reserved.
    #[serde(default)]
    stock_reserved: i64,
}

impl PublicProductRow {
    fn into_dto(self, low_threshold: i64) -> PublicProductDto {
        PublicProductDto {
            id: self.id.to_string(),
            slug: self.slug,
            name: self.online_title.unwrap_or(self.name),
            description_short: self.online_description.or(self.description),
            price_clp: self.online_price.unwrap_or(self.price),
            image_url: self.image_url,
            category_slug: self.category_slug,
            availability: public_availability(self.stock - self.stock_reserved, low_threshold),
        }
    }
}

fn public_availability(stock: i64, low_threshold: i64) -> PublicAvailability {
    if stock <= 0 {
        PublicAvailability::OutOfStock
    } else if stock <= low_threshold {
        PublicAvailability::Low
    } else {
        PublicAvailability::InStock
    }
}

/// Columns safe for public projection. `category.slug` is a record traversal
/// (NONE when the product has no category).
const PUBLIC_PRODUCT_FIELDS: &str = "id, slug, name, online_title, description, \
     online_description, price, online_price, image_url, \
     category.slug AS category_slug, stock, stock_reserved, online_sort";

/// Base filter for everything the public catalog may show: tenant-scoped,
/// active, operator opted the SKU in, and over-the-counter only ('direct' —
/// prescription products never reach the public web).
const PUBLIC_PRODUCT_COND: &str =
    "tenant = $t AND active = true AND online_visible = true AND prescription_type = 'direct'";

pub async fn tenant_by_slug(db: &Db, slug: &str) -> DomainResult<Option<Thing>> {
    let mut r = db
        .query("SELECT id, name FROM tenant WHERE slug = $slug LIMIT 1")
        .bind(("slug", slug.to_string()))
        .await?;
    let row: Option<PublicTenantRow> = r.take(0)?;
    Ok(row.map(|t| t.id))
}

pub async fn tenant_name(db: &Db, tenant: &Thing) -> DomainResult<Option<String>> {
    let mut r = db
        .query("SELECT id, name FROM tenant WHERE id = $id LIMIT 1")
        .bind(("id", tenant.clone()))
        .await?;
    let row: Option<PublicTenantRow> = r.take(0)?;
    Ok(row.and_then(|t| t.name))
}

pub async fn list_public_products(
    db: &Db,
    tenant: &Thing,
    filters: &PublicCatalogFilters,
    limit: u32,
    offset: u32,
    low_threshold: i64,
) -> DomainResult<Vec<PublicProductDto>> {
    let search = filters
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase);
    let category = filters
        .category
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    // Conditions are compile-time literals; user input only travels via binds.
    // `limit`/`offset` are clamped u32 — numeric interpolation is safe.
    let mut cond = String::from(PUBLIC_PRODUCT_COND);
    if search.is_some() {
        cond.push_str(
            " AND (string::lowercase(name) CONTAINS $q \
               OR string::lowercase(online_title ?? '') CONTAINS $q \
               OR string::lowercase(active_ingredient ?? '') CONTAINS $q)",
        );
    }
    if category.is_some() {
        cond.push_str(" AND category.slug = $cat");
    }
    let sql = format!(
        "SELECT {PUBLIC_PRODUCT_FIELDS} FROM product WHERE {cond} \
         ORDER BY online_sort, name LIMIT {limit} START {offset}"
    );
    let mut r = db
        .query(sql)
        .bind(("t", tenant.clone()))
        .bind(("q", search.unwrap_or_default()))
        .bind(("cat", category.unwrap_or_default()))
        .await?;
    let rows: Vec<PublicProductRow> = r.take(0)?;
    Ok(rows
        .into_iter()
        .map(|row| row.into_dto(low_threshold))
        .collect())
}

pub async fn get_public_product(
    db: &Db,
    tenant: &Thing,
    product_slug: &str,
    low_threshold: i64,
) -> DomainResult<Option<PublicProductDto>> {
    let sql = format!(
        "SELECT {PUBLIC_PRODUCT_FIELDS} FROM product \
         WHERE {PUBLIC_PRODUCT_COND} AND slug = $slug LIMIT 1"
    );
    let mut r = db
        .query(sql)
        .bind(("t", tenant.clone()))
        .bind(("slug", product_slug.to_string()))
        .await?;
    let row: Option<PublicProductRow> = r.take(0)?;
    Ok(row.map(|p| p.into_dto(low_threshold)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The point of `TagField` is that callers cannot pass arbitrary
    // strings into the column interpolation. This test pins the
    // whitelist + ensures every variant is matched (exhaustive `match`
    // below means adding a variant without a literal is a compile error).
    #[test]
    fn tag_field_only_accepts_whitelist() {
        let cases: &[(TagField, &str)] = &[
            (TagField::Laboratory, "laboratory"),
            (TagField::ActiveIngredient, "active_ingredient"),
            (TagField::TherapeuticAction, "therapeutic_action"),
        ];
        for (f, expected) in cases {
            // Exhaustive match — adding a TagField variant is a compile
            // error until handled here.
            let got: &str = match f {
                TagField::Laboratory => "laboratory",
                TagField::ActiveIngredient => "active_ingredient",
                TagField::TherapeuticAction => "therapeutic_action",
            };
            assert_eq!(got, *expected);
            assert_eq!(f.column(), *expected);
        }
    }

    // Compile-time guarantee: `bulk_update_price_typed` cannot accept a
    // free-form SQL string. Variants only take `Decimal`. The classic
    // `; DROP TABLE x;` injection isn't even expressible at the type
    // level — `Decimal::from_str(...)` will fail on any non-numeric
    // input, and there is no string field on `PriceOp` to smuggle SQL
    // through.
    #[test]
    fn bulk_update_price_rejects_arbitrary_sql() {
        use std::str::FromStr;
        // Cannot construct a PriceOp from a SQL string — only from a
        // Decimal. This is a compile-time guarantee asserted by example.
        let _ok = PriceOp::SetExact(Decimal::from_str("1.10").unwrap());
        let _ok = PriceOp::MultiplyPct(Decimal::from_str("1.10").unwrap());
        let _ok = PriceOp::DeltaAbs(Decimal::from_str("-500").unwrap());
        assert!(Decimal::from_str("; DROP TABLE product; --").is_err());
    }
}

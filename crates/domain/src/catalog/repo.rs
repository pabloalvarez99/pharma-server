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
             discount_percent = $discount_percent RETURN AFTER",
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
        .bind(("attrs", attrs))
        .bind(("physical_stock", physical_stock))
        .await?;
    let row: Option<ProductRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

/// Active (and inactive) variants of a parent, tenant-scoped.
pub async fn list_variants(
    db: &Db,
    tenant: &Thing,
    parent: &Thing,
) -> DomainResult<Vec<ProductDto>> {
    let mut r = db
        .query(
            "SELECT * FROM product WHERE tenant = $t AND parent_id = $p \
             ORDER BY name",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", parent.clone()))
        .await?;
    let rows: Vec<ProductRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// `true` if the product has at least one active child variant.
pub async fn has_active_variants(db: &Db, tenant: &Thing, parent: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query(
            "SELECT id FROM product WHERE tenant = $t AND parent_id = $p \
             AND active = true LIMIT 1",
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
    let limit = f.limit.unwrap_or(100).min(500);
    let offset = f.offset.unwrap_or(0);
    let q = format!(
        "SELECT * FROM product WHERE {} ORDER BY name LIMIT {} START {}",
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
    let mut r = db
        .query("SELECT * FROM product WHERE id = $id AND tenant = $t LIMIT 1")
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

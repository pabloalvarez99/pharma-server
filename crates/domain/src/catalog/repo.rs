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
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

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

pub async fn list_products(
    db: &Db,
    tenant: &Thing,
    f: &ProductFilters,
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
        conds.push("stock <= $low".to_string());
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

pub async fn set_stock(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
    new_stock: i64,
) -> DomainResult<ProductDto> {
    let mut r = db
        .query("UPDATE product SET stock = $s WHERE id = $id AND tenant = $t RETURN AFTER")
        .bind(("s", new_stock))
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<ProductRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn stats(db: &Db, tenant: &Thing, low: i64) -> DomainResult<ProductStats> {
    #[derive(Deserialize)]
    struct Agg {
        total: i64,
        active: i64,
        low_stock: i64,
        out_of_stock: i64,
        inventory_value: Option<Decimal>,
    }
    let mut r = db
        .query(
            "SELECT \
               count() AS total, \
               count(active = true) AS active, \
               count(stock <= $low AND active = true) AS low_stock, \
               count(stock <= 0 AND active = true) AS out_of_stock, \
               math::sum(stock * (cost_price ?? 0dec)) AS inventory_value \
             FROM product WHERE tenant = $t GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .bind(("low", low))
        .await?;
    let agg: Option<Agg> = r.take(0)?;
    let agg = agg.unwrap_or(Agg {
        total: 0,
        active: 0,
        low_stock: 0,
        out_of_stock: 0,
        inventory_value: None,
    });
    Ok(ProductStats {
        total: agg.total,
        active: agg.active,
        low_stock: agg.low_stock,
        out_of_stock: agg.out_of_stock,
        inventory_value: agg.inventory_value.unwrap_or_default(),
        expired: 0,
    })
}

/// Returns number of products repriced.
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

pub async fn etiquetas(db: &Db, tenant: &Thing, q: &str) -> DomainResult<EtiquetaResults> {
    async fn distinct(db: &Db, tenant: &Thing, field: &str, q: &str) -> DomainResult<Vec<String>> {
        let sql = format!(
            "SELECT VALUE {field} FROM product \
             WHERE tenant = $t AND {field} != NONE \
             AND string::lowercase({field}) CONTAINS $q \
             GROUP BY {field} LIMIT 20"
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
        laboratories: distinct(db, tenant, "laboratory", q).await?,
        active_ingredients: distinct(db, tenant, "active_ingredient", q).await?,
        therapeutic_actions: distinct(db, tenant, "therapeutic_action", q).await?,
    })
}

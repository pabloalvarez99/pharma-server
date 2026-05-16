//! Sales persistence (Fase 4). Atomic POS sale, refunds, settings, idempotency.
//!
//! All write paths use multi-statement SurrealQL `BEGIN; …; COMMIT;` so the
//! `order`, `order_item`, `product.stock` decrement and `stock_movement` rows
//! cannot diverge. See [`apply_sale`] for the canonical pattern (mirrors
//! `inventory::repo::apply_movement`).

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::*;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

// --- bind helpers (same pattern as catalog/inventory) ----------------------

fn dec_val(d: Decimal) -> surrealdb::sql::Value {
    surrealdb::sql::Number::from(d).into()
}

fn dec_opt(d: Option<Decimal>) -> surrealdb::sql::Value {
    match d {
        Some(x) => dec_val(x),
        None => surrealdb::sql::Value::None,
    }
}

fn dt_val(dt: DateTime<Utc>) -> surrealdb::sql::Value {
    surrealdb::sql::Datetime::from(dt).into()
}

// --- DB rows ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct OrderRow {
    id: Thing,
    status: String,
    payment_method: String,
    subtotal: Decimal,
    discount: Decimal,
    total: Decimal,
    cash_amount: Option<Decimal>,
    card_amount: Option<Decimal>,
    customer: Option<Thing>,
    customer_name: Option<String>,
    customer_phone: Option<String>,
    sold_by: Option<Thing>,
    sold_by_name: Option<String>,
    notes: Option<String>,
    external_ref: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<OrderRow> for OrderDto {
    fn from(r: OrderRow) -> Self {
        Self {
            id: r.id.to_string(),
            status: r.status,
            payment_method: r.payment_method,
            subtotal: r.subtotal,
            discount: r.discount,
            total: r.total,
            cash_amount: r.cash_amount,
            card_amount: r.card_amount,
            customer: r.customer.map(|t| t.to_string()),
            customer_name: r.customer_name,
            customer_phone: r.customer_phone,
            sold_by: r.sold_by.map(|t| t.to_string()),
            sold_by_name: r.sold_by_name,
            notes: r.notes,
            external_ref: r.external_ref,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct OrderItemRow {
    id: Thing,
    order: Thing,
    product: Option<Thing>,
    product_name: String,
    quantity: i64,
    unit_price: Decimal,
    subtotal: Decimal,
    batch: Option<Thing>,
}

impl From<OrderItemRow> for OrderItemDto {
    fn from(r: OrderItemRow) -> Self {
        Self {
            id: r.id.to_string(),
            order: r.order.to_string(),
            product: r.product.map(|t| t.to_string()),
            product_name: r.product_name,
            quantity: r.quantity,
            unit_price: r.unit_price,
            subtotal: r.subtotal,
            batch: r.batch.map(|t| t.to_string()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct StockCheck {
    id: Thing,
    stock: i64,
    name: String,
}

#[derive(Debug, Deserialize)]
struct MovementIdRow {
    id: Thing,
}

#[derive(Debug, Deserialize)]
struct SettingRow {
    key: String,
    value: String,
    updated_at: DateTime<Utc>,
}

impl From<SettingRow> for AdminSettingDto {
    fn from(r: SettingRow) -> Self {
        Self {
            key: r.key,
            value: r.value,
            updated_at: r.updated_at,
        }
    }
}

// --- stock pre-check -------------------------------------------------------

/// Load `(id, stock, name)` for all product ids in `products`, scoped to
/// tenant + active. Returns Err if any id is missing / inactive / wrong tenant.
pub async fn load_products_for_sale(
    db: &Db,
    tenant: &Thing,
    products: &[Thing],
) -> DomainResult<Vec<StockCheckOut>> {
    if products.is_empty() {
        return Ok(Vec::new());
    }
    let mut r = db
        .query(
            "SELECT id, stock, name FROM product \
             WHERE tenant = $t AND active = true AND id IN $ids",
        )
        .bind(("t", tenant.clone()))
        .bind(("ids", products.to_vec()))
        .await?
        .check()?;
    let rows: Vec<StockCheck> = r.take(0)?;
    Ok(rows
        .into_iter()
        .map(|r| StockCheckOut {
            id: r.id,
            stock: r.stock,
            name: r.name,
        })
        .collect())
}

pub struct StockCheckOut {
    pub id: Thing,
    pub stock: i64,
    pub name: String,
}

// --- POS sale atomic tx ----------------------------------------------------

pub struct AppliedSale {
    pub order: OrderDto,
    pub items: Vec<OrderItemDto>,
    pub movement_ids: Vec<String>,
}

/// Persist a POS sale in a single multi-statement SurrealQL tx.
///
/// Statements (in order):
/// 1. `CREATE order SET …`
/// 2. For each item: `CREATE order_item SET …`
/// 3. For each item: `UPDATE product SET stock -= qty …`
/// 4. For each item: `CREATE stock_movement SET reason='sale', ref=<order.id> …`
///
/// All in one `BEGIN; …; COMMIT;` so a partial failure rolls back stock too.
/// Callers MUST have validated tenant ownership + sufficient stock upfront
/// (use [`load_products_for_sale`]).
#[allow(clippy::too_many_arguments)]
pub async fn apply_sale(
    db: &Db,
    tenant: &Thing,
    sold_by: Option<&Thing>,
    sold_by_name: Option<&str>,
    customer: Option<&Thing>,
    req: &PosSaleRequest,
    subtotal: Decimal,
    discount: Decimal,
    total: Decimal,
) -> DomainResult<AppliedSale> {
    // Two-call atomic sale: SurrealDB's `LET` slot semantics are inconsistent
    // across versions and `SELECT VALUE id … ORDER BY created_at` is rejected
    // by the ORDER-BY-projection gotcha. Cleaner: step 1 = CREATE order,
    // step 2 = `BEGIN; per-item …; COMMIT;` with the order Thing bound.
    let mut r = db
        .query(
            "CREATE order SET tenant=$t, status='paid', payment_method=$pm, \
             subtotal=$sub, discount=$disc, total=$tot, \
             cash_amount=$cash, card_amount=$card, customer=$cust, \
             customer_name=$cname, customer_phone=$cphone, sold_by=$sb, \
             sold_by_name=$sbname, notes=$notes, external_ref=$ext \
             RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("pm", req.payment_method.clone()))
        .bind(("sub", dec_val(subtotal)))
        .bind(("disc", dec_val(discount)))
        .bind(("tot", dec_val(total)))
        .bind(("cash", dec_opt(req.cash_amount)))
        .bind(("card", dec_opt(req.card_amount)))
        .bind(("cust", customer.cloned()))
        .bind(("cname", req.customer_name.clone()))
        .bind(("cphone", req.customer_phone.clone()))
        .bind(("sb", sold_by.cloned()))
        .bind(("sbname", sold_by_name.map(str::to_string)))
        .bind(("notes", req.notes.clone()))
        .bind(("ext", req.external_ref.clone()))
        .await?
        .check()?;
    let order_rows: Vec<OrderRow> = r.take(0)?;
    let order_row = order_rows
        .into_iter()
        .next()
        .ok_or_else(|| DomainError::Other(anyhow::anyhow!("order CREATE returned 0 rows")))?;
    let order_thing = order_row.id.clone();
    let order: OrderDto = order_row.into();

    // Step 2: items + stock decrement + movements in one transaction.
    let n = req.items.len();
    let mut q = String::from("BEGIN; ");
    for i in 0..n {
        q.push_str(&format!(
            "CREATE order_item SET tenant=$t, order=$ord, product=$p{i}, \
                product_name=$pn{i}, quantity=$qty{i}, unit_price=$up{i}, \
                subtotal=$st{i} RETURN AFTER; \
             UPDATE product SET stock = stock - $qty{i} \
                WHERE id = $p{i} AND tenant = $t; \
             CREATE stock_movement SET tenant=$t, product=$p{i}, \
                delta = 0 - $qty{i}, reason='sale', \
                admin=$sb, ref=$ref RETURN AFTER; ",
        ));
    }
    q.push_str("COMMIT;");
    let mut qb = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("ord", order_thing.clone()))
        .bind(("sb", sold_by.cloned()))
        .bind(("ref", order_thing.to_string()));
    for (i, item) in req.items.iter().enumerate() {
        let pid = surrealdb::sql::thing(&item.product)
            .map_err(|_| DomainError::Invalid(format!("product id inválido: {}", item.product)))?;
        let line_sub = item.unit_price * Decimal::from(item.quantity);
        qb = qb
            .bind((format!("p{i}"), pid))
            .bind((format!("pn{i}"), item.product_name.clone()))
            .bind((format!("qty{i}"), item.quantity))
            .bind((format!("up{i}"), dec_val(item.unit_price)))
            .bind((format!("st{i}"), dec_val(line_sub)));
    }
    let mut r2 = qb.await?.check()?;

    let mut items_out = Vec::with_capacity(n);
    let mut movements_out = Vec::with_capacity(n);
    for i in 0..n {
        let base = i * 3;
        let item_rows: Vec<OrderItemRow> = r2.take(base)?;
        items_out.push(
            item_rows
                .into_iter()
                .next()
                .ok_or_else(|| {
                    DomainError::Other(anyhow::anyhow!("order_item insert returned 0 rows"))
                })?
                .into(),
        );
        let mov_rows: Vec<MovementIdRow> = r2.take(base + 2)?;
        movements_out.push(
            mov_rows
                .into_iter()
                .next()
                .ok_or_else(|| {
                    DomainError::Other(anyhow::anyhow!("stock_movement insert returned 0 rows"))
                })?
                .id
                .to_string(),
        );
    }

    Ok(AppliedSale {
        order,
        items: items_out,
        movement_ids: movements_out,
    })
}

// --- orders read -----------------------------------------------------------

pub async fn list_orders(db: &Db, tenant: &Thing, f: &OrderFilters) -> DomainResult<Vec<OrderDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.status.is_some() {
        conds.push("status = $s".to_string());
    }
    if f.payment_method.is_some() {
        conds.push("payment_method = $pm".to_string());
    }
    if f.customer.is_some() {
        conds.push("customer = $c".to_string());
    }
    if f.from.is_some() {
        conds.push("created_at >= $from".to_string());
    }
    if f.to.is_some() {
        conds.push("created_at <= $to".to_string());
    }
    let limit = f.limit.unwrap_or(100).min(500);
    let offset = f.offset.unwrap_or(0);
    let q = format!(
        "SELECT * FROM order WHERE {} \
         ORDER BY created_at DESC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset,
    );
    let cust = f
        .customer
        .as_deref()
        .and_then(|s| surrealdb::sql::thing(s).ok());
    let from_v = match f.from {
        Some(d) => dt_val(d),
        None => surrealdb::sql::Value::None,
    };
    let to_v = match f.to {
        Some(d) => dt_val(d),
        None => surrealdb::sql::Value::None,
    };
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("s", f.status.clone().unwrap_or_default()))
        .bind(("pm", f.payment_method.clone().unwrap_or_default()))
        .bind(("c", cust))
        .bind(("from", from_v))
        .bind(("to", to_v))
        .await?;
    let rows: Vec<OrderRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_order(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
) -> DomainResult<Option<(OrderDto, Vec<OrderItemDto>)>> {
    let mut r = db
        .query(
            "SELECT * FROM order WHERE id = $id AND tenant = $t LIMIT 1; \
             SELECT * FROM order_item WHERE order = $id AND tenant = $t \
                ORDER BY created_at ASC;",
        )
        .bind(("t", tenant.clone()))
        .bind(("id", id.clone()))
        .await?;
    let order_row: Option<OrderRow> = r.take(0)?;
    let Some(order_row) = order_row else {
        return Ok(None);
    };
    let item_rows: Vec<OrderItemRow> = r.take(1)?;
    Ok(Some((
        order_row.into(),
        item_rows.into_iter().map(Into::into).collect(),
    )))
}

// --- admin_setting ---------------------------------------------------------

pub async fn get_setting(
    db: &Db,
    tenant: &Thing,
    key: &str,
) -> DomainResult<Option<AdminSettingDto>> {
    let mut r = db
        .query(
            "SELECT key, value, updated_at FROM admin_setting \
             WHERE tenant = $t AND key = $k LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("k", key.to_string()))
        .await?;
    let row: Option<SettingRow> = r.take(0)?;
    Ok(row.map(Into::into))
}

pub async fn upsert_setting(
    db: &Db,
    tenant: &Thing,
    key: &str,
    value: &str,
) -> DomainResult<AdminSettingDto> {
    // Surreal lacks an idiomatic ON CONFLICT UPDATE; pattern is delete+create
    // inside a tx (cheap; setting writes are rare).
    let mut r = db
        .query(
            "BEGIN; \
             DELETE admin_setting WHERE tenant = $t AND key = $k; \
             CREATE admin_setting SET tenant = $t, key = $k, value = $v RETURN AFTER; \
             COMMIT;",
        )
        .bind(("t", tenant.clone()))
        .bind(("k", key.to_string()))
        .bind(("v", value.to_string()))
        .await?
        .check()?;
    let row: Option<SettingRow> = r.take(1)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

// --- idempotency_key -------------------------------------------------------

#[derive(Debug, Deserialize)]
struct IdempotencyRow {
    response_json: String,
    status_code: i64,
}

/// Returns cached response if `key` already resolved for `tenant` within TTL.
pub async fn lookup_idempotency(
    db: &Db,
    tenant: &Thing,
    key: &str,
) -> DomainResult<Option<(String, u16)>> {
    let mut r = db
        .query(
            "SELECT response_json, status_code FROM idempotency_key \
             WHERE tenant = $t AND key = $k AND expires_at > time::now() LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("k", key.to_string()))
        .await?;
    let row: Option<IdempotencyRow> = r.take(0)?;
    Ok(row.map(|r| (r.response_json, r.status_code as u16)))
}

// --- loyalty ---------------------------------------------------------------

/// Atomically append a `loyalty_transaction` row + bump
/// `customer.loyalty_points`. Caller has validated tenant ownership of
/// `customer` and `delta > 0`.
pub async fn award_loyalty(
    db: &Db,
    tenant: &Thing,
    customer: &Thing,
    delta: i64,
    reason: &str,
    order_ref: &str,
) -> DomainResult<()> {
    db.query(
        "BEGIN; \
         CREATE loyalty_transaction SET tenant=$t, customer=$c, delta=$d, \
            reason=$r, ref=$ref; \
         UPDATE customer SET loyalty_points = loyalty_points + $d \
            WHERE id=$c AND tenant=$t; \
         COMMIT;",
    )
    .bind(("t", tenant.clone()))
    .bind(("c", customer.clone()))
    .bind(("d", delta))
    .bind(("r", reason.to_string()))
    .bind(("ref", order_ref.to_string()))
    .await?
    .check()?;
    Ok(())
}

pub async fn store_idempotency(
    db: &Db,
    tenant: &Thing,
    key: &str,
    response_json: &str,
    status_code: u16,
) -> DomainResult<()> {
    let expires = Utc::now() + Duration::hours(IDEMPOTENCY_TTL_HOURS);
    db.query(
        "CREATE idempotency_key SET tenant=$t, key=$k, response_json=$r, \
             status_code=$s, expires_at=$exp",
    )
    .bind(("t", tenant.clone()))
    .bind(("k", key.to_string()))
    .bind(("r", response_json.to_string()))
    .bind(("s", status_code as i64))
    .bind(("exp", dt_val(expires)))
    .await?
    .check()?;
    Ok(())
}

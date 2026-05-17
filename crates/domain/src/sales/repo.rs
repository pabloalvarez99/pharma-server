//! Sales persistence (Fase 4). Atomic POS sale, refunds, settings, idempotency.
//!
//! The POS sale is one multi-statement SurrealQL `BEGIN; …; COMMIT;` that
//! creates the `order` (with a client-generated record id), its
//! `order_item`s, the `product.stock` decrement, the `stock_movement` rows
//! and the FEFO `product_batch.stock` decrements together — so a crash can
//! never leave a paid order without items/stock, and `product.stock` stays
//! equal to the sum of its batch stock. See [`apply_sale`].

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};
use crate::inventory::model::FefoAllocation;

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
    #[serde(default)]
    batches_json: Option<String>,
}

impl From<OrderItemRow> for OrderItemDto {
    fn from(r: OrderItemRow) -> Self {
        // Multi-lot breakdown (BACKLOG #3): parse the JSON-string column written
        // by `apply_sale`. Silently None on legacy rows or malformed payloads —
        // the primary `batch` field is the legacy fallback for those.
        let batches = r
            .batches_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<OrderItemBatchAllocation>>(s).ok());
        Self {
            id: r.id.to_string(),
            order: r.order.to_string(),
            product: r.product.map(|t| t.to_string()),
            product_name: r.product_name,
            quantity: r.quantity,
            unit_price: r.unit_price,
            subtotal: r.subtotal,
            batch: r.batch.map(|t| t.to_string()),
            batches,
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

/// Persist a POS sale in ONE multi-statement SurrealQL transaction.
///
/// `BEGIN;` then, in order:
/// 1. `CREATE type::thing('order',$oid) SET status='paid' …` — the order id
///    is generated client-side so every later statement can reference it
///    inside the same tx (no separate auto-committed `CREATE order` that a
///    crash could orphan).
/// 2. per item: `CREATE order_item …` (with the primary FEFO `batch` set),
///    `UPDATE product SET stock -= qty`, `CREATE stock_movement reason='sale'`.
/// 3. per FEFO allocation: `UPDATE product_batch SET stock -= n`. The
///    `product_batch.stock` `ASSERT >= 0` is the final oversell guard — a
///    concurrent drain between planning and commit aborts the whole tx.
///
/// Then `COMMIT;`.
///
/// `fefo_plans[i]` aligns with `req.items[i]`: `None` = product not
/// batch-tracked (decrement `product.stock` only, legacy behavior);
/// `Some(plan)` = consume those lots. Callers MUST have validated tenant
/// ownership + sufficient stock and built the plan
/// ([`super::service::post_sale`]).
#[allow(clippy::too_many_arguments)]
pub async fn apply_sale(
    db: &Db,
    tenant: &Thing,
    sold_by: Option<&Thing>,
    sold_by_name: Option<&str>,
    customer: Option<&Thing>,
    req: &PosSaleRequest,
    fefo_plans: &[Option<Vec<FefoAllocation>>],
    subtotal: Decimal,
    discount: Decimal,
    total: Decimal,
) -> DomainResult<AppliedSale> {
    let n = req.items.len();
    // Client-generated order id so the order CREATE lives inside the same
    // BEGIN/COMMIT as its items/stock/movements/batches. `Thing` is
    // `#[non_exhaustive]`, so we parse one with the public constructor.
    let oid = uuid::Uuid::new_v4().simple().to_string();
    let order_thing = surrealdb::sql::thing(&format!("order:{oid}"))
        .map_err(|e| DomainError::Other(anyhow::anyhow!("order id build: {e}")))?;

    let mut q = String::from(
        "BEGIN; \
         CREATE type::thing('order', $oid) SET tenant=$t, status='paid', \
            payment_method=$pm, subtotal=$sub, discount=$disc, total=$tot, \
            cash_amount=$cash, card_amount=$card, customer=$cust, \
            customer_name=$cname, customer_phone=$cphone, sold_by=$sb, \
            sold_by_name=$sbname, notes=$notes, external_ref=$ext \
            RETURN AFTER; ",
    );
    for i in 0..n {
        q.push_str(&format!(
            "CREATE order_item SET tenant=$t, order=$ord, product=$p{i}, \
                product_name=$pn{i}, quantity=$qty{i}, unit_price=$up{i}, \
                subtotal=$st{i}, batch=$bt{i}, batches_json=$bts{i} \
                RETURN AFTER; \
             UPDATE product SET stock = stock - $qty{i} \
                WHERE id = $p{i} AND tenant = $t; \
             CREATE stock_movement SET tenant=$t, product=$p{i}, \
                delta = 0 - $qty{i}, reason='sale', \
                admin=$sb, ref=$ref RETURN AFTER; ",
        ));
    }
    // FEFO batch decrements grouped at the tail so the per-item result
    // indices above stay fixed regardless of allocation count.
    let mut alloc_idx = 0usize;
    for allocs in fefo_plans.iter().flatten() {
        for _ in allocs {
            q.push_str(&format!(
                "UPDATE product_batch SET stock = stock - $ba{alloc_idx} \
                 WHERE id = $bid{alloc_idx} AND tenant = $t; ",
            ));
            alloc_idx += 1;
        }
    }
    q.push_str("COMMIT;");

    let mut qb = db
        .query(q)
        .bind(("oid", oid.clone()))
        .bind(("t", tenant.clone()))
        .bind(("ord", order_thing.clone()))
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
        .bind(("ref", order_thing.to_string()));
    for (i, item) in req.items.iter().enumerate() {
        let pid = surrealdb::sql::thing(&item.product)
            .map_err(|_| DomainError::Invalid(format!("product id inválido: {}", item.product)))?;
        let line_sub = item.unit_price * Decimal::from(item.quantity);
        // Primary batch = earliest-expiry lot consumed (first FEFO allocation),
        // kept for backward compat. Full breakdown goes to `batches_json`
        // (BACKLOG #3, migration 0013) so refund/audit flows can attribute
        // every consumed lot, not just the head.
        let plan = fefo_plans.get(i).and_then(|p| p.as_ref());
        let batch_val: surrealdb::sql::Value = match plan {
            Some(allocs) if !allocs.is_empty() => surrealdb::sql::thing(&allocs[0].batch)
                .map_err(|_| {
                    DomainError::Invalid(format!("batch id inválido: {}", allocs[0].batch))
                })?
                .into(),
            _ => surrealdb::sql::Value::None,
        };
        let batches_json: surrealdb::sql::Value = match plan {
            Some(allocs) if !allocs.is_empty() => {
                let payload: Vec<crate::sales::model::OrderItemBatchAllocation> = allocs
                    .iter()
                    .map(|a| crate::sales::model::OrderItemBatchAllocation {
                        batch: a.batch.clone(),
                        qty: a.qty,
                    })
                    .collect();
                serde_json::to_string(&payload)
                    .map_err(|e| DomainError::Other(anyhow::anyhow!("batches_json: {e}")))?
                    .into()
            }
            _ => surrealdb::sql::Value::None,
        };
        qb = qb
            .bind((format!("p{i}"), pid))
            .bind((format!("pn{i}"), item.product_name.clone()))
            .bind((format!("qty{i}"), item.quantity))
            .bind((format!("up{i}"), dec_val(item.unit_price)))
            .bind((format!("st{i}"), dec_val(line_sub)))
            .bind((format!("bt{i}"), batch_val))
            .bind((format!("bts{i}"), batches_json));
    }
    // Bind the tail batch-decrement params in the same order they were emitted.
    let mut alloc_idx = 0usize;
    for allocs in fefo_plans.iter().flatten() {
        for a in allocs {
            let bid = surrealdb::sql::thing(&a.batch)
                .map_err(|_| DomainError::Invalid(format!("batch id inválido: {}", a.batch)))?;
            qb = qb
                .bind((format!("ba{alloc_idx}"), a.qty))
                .bind((format!("bid{alloc_idx}"), bid));
            alloc_idx += 1;
        }
    }
    let mut r2 = qb.await?.check()?;

    // Statement indices (BEGIN/COMMIT excluded): 0 = order CREATE;
    // item i: order_item at 1+i*3, product UPDATE at 2+i*3, movement at 3+i*3.
    let order_rows: Vec<OrderRow> = r2.take(0)?;
    let order: OrderDto = order_rows
        .into_iter()
        .next()
        .ok_or_else(|| DomainError::Other(anyhow::anyhow!("order CREATE returned 0 rows")))?
        .into();

    let mut items_out = Vec::with_capacity(n);
    let mut movements_out = Vec::with_capacity(n);
    for i in 0..n {
        let item_rows: Vec<OrderItemRow> = r2.take(1 + i * 3)?;
        items_out.push(
            item_rows
                .into_iter()
                .next()
                .ok_or_else(|| {
                    DomainError::Other(anyhow::anyhow!("order_item insert returned 0 rows"))
                })?
                .into(),
        );
        let mov_rows: Vec<MovementIdRow> = r2.take(3 + i * 3)?;
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

// --- devolucion (refund) atomic tx -----------------------------------------

#[derive(Debug, Deserialize)]
struct DevolucionRow {
    id: Thing,
    order: Option<Thing>,
    tipo: String,
    motivo: String,
    notas: Option<String>,
    total_devuelto: Decimal,
    metodo_reembolso: Option<String>,
    procesado_por: Option<Thing>,
    created_at: DateTime<Utc>,
}

impl From<DevolucionRow> for DevolucionDto {
    fn from(r: DevolucionRow) -> Self {
        Self {
            id: r.id.to_string(),
            order: r.order.map(|t| t.to_string()),
            tipo: r.tipo,
            motivo: r.motivo,
            notas: r.notas,
            total_devuelto: r.total_devuelto,
            metodo_reembolso: r.metodo_reembolso,
            procesado_por: r.procesado_por.map(|t| t.to_string()),
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct DevolucionItemRow {
    id: Thing,
    devolucion: Thing,
    product: Option<Thing>,
    product_name: String,
    quantity: i64,
    unit_price: Decimal,
    restock: bool,
}

impl From<DevolucionItemRow> for DevolucionItemDto {
    fn from(r: DevolucionItemRow) -> Self {
        Self {
            id: r.id.to_string(),
            devolucion: r.devolucion.to_string(),
            product: r.product.map(|t| t.to_string()),
            product_name: r.product_name,
            quantity: r.quantity,
            unit_price: r.unit_price,
            restock: r.restock,
        }
    }
}

pub struct AppliedRefund {
    pub devolucion: DevolucionDto,
    pub items: Vec<DevolucionItemDto>,
    pub movement_ids: Vec<String>,
    pub order_marked_refunded: bool,
}

/// Persist a `devolucion` + its `devolucion_item`s atomically. Lines flagged
/// `restock=true` add their quantity back to `product.stock` and append a
/// `stock_movement(reason='return')` inside the same BEGIN/COMMIT (stock is
/// never written outside the audit trail — same invariant as the sale path).
/// When `order` is set, the order is moved to `status='refunded'` in the same
/// tx so a crash can't leave a refunded order still marked paid.
#[allow(clippy::too_many_arguments)]
pub async fn apply_refund(
    db: &Db,
    tenant: &Thing,
    processed_by: Option<&Thing>,
    order: Option<&Thing>,
    req: &NewDevolucion,
    total: Decimal,
) -> DomainResult<AppliedRefund> {
    let did = uuid::Uuid::new_v4().simple().to_string();
    let dev_thing = surrealdb::sql::thing(&format!("devolucion:{did}"))
        .map_err(|e| DomainError::Other(anyhow::anyhow!("devolucion id build: {e}")))?;

    let mut q = String::from(
        "BEGIN; \
         CREATE type::thing('devolucion', $did) SET tenant=$t, order=$ord, \
            tipo=$tipo, motivo=$mot, notas=$notas, total_devuelto=$tot, \
            metodo_reembolso=$mr, procesado_por=$by RETURN AFTER; ",
    );
    // Statement layout (BEGIN/COMMIT excluded). idx 0 = devolucion CREATE.
    // For item i: devolucion_item CREATE at `item_idx[i]`; if it restocks, a
    // product UPDATE follows then a stock_movement CREATE at `mov_idx[i]`.
    let mut stmt = 1usize;
    let mut item_idx = Vec::with_capacity(req.items.len());
    let mut mov_idx = Vec::with_capacity(req.items.len());
    for (i, it) in req.items.iter().enumerate() {
        q.push_str(&format!(
            "CREATE devolucion_item SET tenant=$t, devolucion=$dev, \
                product=$p{i}, product_name=$pn{i}, quantity=$qty{i}, \
                unit_price=$up{i}, restock=$rs{i} RETURN AFTER; ",
        ));
        item_idx.push(stmt);
        stmt += 1;
        if it.restock && it.product.is_some() {
            q.push_str(&format!(
                "UPDATE product SET stock = stock + $qty{i} \
                 WHERE id = $p{i} AND tenant = $t; ",
            ));
            stmt += 1;
            q.push_str(&format!(
                "CREATE stock_movement SET tenant=$t, product=$p{i}, \
                    delta=$qty{i}, reason='return', admin=$by, ref=$devref \
                    RETURN AFTER; ",
            ));
            mov_idx.push(Some(stmt));
            stmt += 1;
        } else {
            mov_idx.push(None);
        }
    }
    let order_marked = order.is_some();
    if order_marked {
        q.push_str("UPDATE order SET status='refunded' WHERE id=$ord AND tenant=$t; ");
    }
    q.push_str("COMMIT;");

    let mut qb = db
        .query(q)
        .bind(("did", did.clone()))
        .bind(("t", tenant.clone()))
        .bind(("ord", order.cloned()))
        .bind(("tipo", req.tipo.clone()))
        .bind(("mot", req.motivo.clone()))
        .bind(("notas", req.notas.clone()))
        .bind(("tot", dec_val(total)))
        .bind(("mr", req.metodo_reembolso.clone()))
        .bind(("by", processed_by.cloned()))
        .bind(("dev", dev_thing.clone()))
        .bind(("devref", dev_thing.to_string()));
    for (i, it) in req.items.iter().enumerate() {
        let pid: surrealdb::sql::Value = match it.product.as_deref() {
            Some(s) => surrealdb::sql::thing(s)
                .map_err(|_| DomainError::Invalid(format!("product id inválido: {s}")))?
                .into(),
            None => surrealdb::sql::Value::None,
        };
        qb = qb
            .bind((format!("p{i}"), pid))
            .bind((format!("pn{i}"), it.product_name.clone()))
            .bind((format!("qty{i}"), it.quantity))
            .bind((format!("up{i}"), dec_val(it.unit_price)))
            .bind((format!("rs{i}"), it.restock));
    }
    let mut r = qb.await?.check()?;

    let dev_rows: Vec<DevolucionRow> = r.take(0)?;
    let devolucion: DevolucionDto = dev_rows
        .into_iter()
        .next()
        .ok_or_else(|| DomainError::Other(anyhow::anyhow!("devolucion CREATE returned 0 rows")))?
        .into();

    let mut items_out = Vec::with_capacity(req.items.len());
    let mut movements_out = Vec::new();
    for i in 0..req.items.len() {
        let irows: Vec<DevolucionItemRow> = r.take(item_idx[i])?;
        items_out.push(
            irows
                .into_iter()
                .next()
                .ok_or_else(|| {
                    DomainError::Other(anyhow::anyhow!("devolucion_item insert returned 0 rows"))
                })?
                .into(),
        );
        if let Some(mi) = mov_idx[i] {
            let mrows: Vec<MovementIdRow> = r.take(mi)?;
            movements_out.push(
                mrows
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        DomainError::Other(anyhow::anyhow!("stock_movement insert returned 0 rows"))
                    })?
                    .id
                    .to_string(),
            );
        }
    }

    Ok(AppliedRefund {
        devolucion,
        items: items_out,
        movement_ids: movements_out,
        order_marked_refunded: order_marked,
    })
}

pub async fn list_devoluciones(
    db: &Db,
    tenant: &Thing,
    f: &DevolucionFilters,
) -> DomainResult<Vec<DevolucionDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.order.is_some() {
        conds.push("order = $o".to_string());
    }
    if f.tipo.is_some() {
        conds.push("tipo = $tp".to_string());
    }
    let limit = f.limit.unwrap_or(100).clamp(1, 500);
    let offset = f.offset.unwrap_or(0).max(0);
    let sql = format!(
        "SELECT * FROM devolucion WHERE {} ORDER BY created_at DESC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset
    );
    let mut qb = db.query(sql).bind(("t", tenant.clone()));
    if let Some(o) = &f.order {
        let ot = surrealdb::sql::thing(o)
            .map_err(|_| DomainError::Invalid(format!("order id inválido: {o}")))?;
        qb = qb.bind(("o", ot));
    }
    if let Some(tp) = &f.tipo {
        qb = qb.bind(("tp", tp.clone()));
    }
    let rows: Vec<DevolucionRow> = qb.await?.check()?.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
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

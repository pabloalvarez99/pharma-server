//! Expenses + simple daily sales report (revenue by UTC date).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use surrealdb::sql::{thing, Thing};

use crate::errors::{DomainError, DomainResult};

use super::model::*;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

fn dec_val(d: Decimal) -> surrealdb::sql::Value {
    surrealdb::sql::Number::from(d).into()
}

const VALID_PM: &[&str] = &["cash", "bank", "card", "transfer"];

#[derive(Debug, Deserialize)]
struct Row {
    id: Thing,
    category: String,
    description: String,
    amount: Decimal,
    payment_method: String,
    cash_session: Option<Thing>,
    supplier: Option<Thing>,
    note: Option<String>,
    created_by: Option<Thing>,
    incurred_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl From<Row> for ExpenseDto {
    fn from(r: Row) -> Self {
        Self {
            id: r.id.to_string(),
            category: r.category,
            description: r.description,
            amount: r.amount,
            payment_method: r.payment_method,
            cash_session: r.cash_session.map(|t| t.to_string()),
            supplier: r.supplier.map(|t| t.to_string()),
            note: r.note,
            created_by: r.created_by.map(|t| t.to_string()),
            incurred_at: r.incurred_at,
            created_at: r.created_at,
        }
    }
}

pub async fn create_expense(
    db: &Db,
    tenant: &Thing,
    created_by: Option<&Thing>,
    input: NewExpense,
) -> DomainResult<ExpenseDto> {
    if input.amount <= Decimal::ZERO {
        return Err(DomainError::Invalid("amount debe ser > 0".into()));
    }
    if input.category.trim().is_empty() {
        return Err(DomainError::Invalid("category requerido".into()));
    }
    if input.description.trim().is_empty() {
        return Err(DomainError::Invalid("description requerido".into()));
    }
    if !VALID_PM.contains(&input.payment_method.as_str()) {
        return Err(DomainError::Invalid(format!(
            "payment_method inválido: {}",
            input.payment_method
        )));
    }
    let session_thing: surrealdb::sql::Value = match input.cash_session.as_deref() {
        Some(s) if !s.is_empty() => {
            let t = thing(s)
                .map_err(|_| DomainError::Invalid(format!("cash_session id inválido: {s}")))?;
            if t.tb != "cash_register_session" {
                return Err(DomainError::Invalid(
                    "cash_session no es record<cash_register_session>".into(),
                ));
            }
            t.into()
        }
        _ => surrealdb::sql::Value::None,
    };
    let supplier_thing: surrealdb::sql::Value = match input.supplier.as_deref() {
        Some(s) if !s.is_empty() => {
            let t =
                thing(s).map_err(|_| DomainError::Invalid(format!("supplier id inválido: {s}")))?;
            if t.tb != "supplier" {
                return Err(DomainError::Invalid(
                    "supplier no es record<supplier>".into(),
                ));
            }
            t.into()
        }
        _ => surrealdb::sql::Value::None,
    };
    let incurred_at: surrealdb::sql::Value = match input.incurred_at {
        Some(dt) => surrealdb::sql::Datetime::from(dt).into(),
        None => surrealdb::sql::Datetime::from(Utc::now()).into(),
    };
    let mut r = db
        .query(
            "CREATE expense SET tenant=$t, category=$c, description=$d, \
             amount=$a, payment_method=$pm, cash_session=$cs, supplier=$su, \
             note=$nt, created_by=$cb, incurred_at=$ia RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("c", input.category))
        .bind(("d", input.description))
        .bind(("a", dec_val(input.amount)))
        .bind(("pm", input.payment_method))
        .bind(("cs", session_thing))
        .bind(("su", supplier_thing))
        .bind(("nt", input.note))
        .bind(("cb", created_by.cloned()))
        .bind(("ia", incurred_at))
        .await?
        .check()?;
    let row: Option<Row> = r.take(0)?;
    row.map(Into::into)
        .ok_or_else(|| DomainError::Other(anyhow::anyhow!("create expense returned 0")))
}

pub async fn list_expenses(
    db: &Db,
    tenant: &Thing,
    f: ExpenseFilters,
) -> DomainResult<Vec<ExpenseDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.category.is_some() {
        conds.push("category = $c".to_string());
    }
    if f.payment_method.is_some() {
        conds.push("payment_method = $pm".to_string());
    }
    if f.from.is_some() {
        conds.push("incurred_at >= $a".to_string());
    }
    if f.to.is_some() {
        conds.push("incurred_at <= $b".to_string());
    }
    let limit = f.limit.unwrap_or(100).clamp(1, 500);
    let offset = f.offset.unwrap_or(0).max(0);
    let sql = format!(
        "SELECT * FROM expense WHERE {} ORDER BY incurred_at DESC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset
    );
    let mut qb = db.query(sql).bind(("t", tenant.clone()));
    if let Some(c) = f.category {
        qb = qb.bind(("c", c));
    }
    if let Some(pm) = f.payment_method {
        qb = qb.bind(("pm", pm));
    }
    if let Some(a) = f.from {
        qb = qb.bind(("a", surrealdb::sql::Datetime::from(a)));
    }
    if let Some(b) = f.to {
        qb = qb.bind(("b", surrealdb::sql::Datetime::from(b)));
    }
    let rows: Vec<Row> = qb.await?.check()?.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Daily sales rollup over `order`. Tenant-scoped. `refunded`/`cancelled`
/// excluded. Returns one row per UTC date in the range, sorted ascending.
pub async fn sales_daily(
    db: &Db,
    tenant: &Thing,
    f: SalesReportFilters,
) -> DomainResult<Vec<DailySalesRow>> {
    let mut conds = vec![
        "tenant = $t".to_string(),
        "status NOT IN ['refunded','cancelled']".to_string(),
    ];
    if f.from.is_some() {
        conds.push("created_at >= $a".to_string());
    }
    if f.to.is_some() {
        conds.push("created_at <= $b".to_string());
    }
    // SurrealKv 2.x can't always GROUP BY a derived string column reliably
    // for our needs (string::slice on cast datetime returned an i64 in tests).
    // Pull the rows and bucket by UTC date in Rust — datasets are small
    // (single-shop scale), totals fit fine in memory.
    let sql = format!(
        "SELECT created_at, total, cash_amount, card_amount FROM order \
         WHERE {} ORDER BY created_at ASC",
        conds.join(" AND ")
    );
    let mut qb = db.query(sql).bind(("t", tenant.clone()));
    if let Some(a) = f.from {
        qb = qb.bind(("a", surrealdb::sql::Datetime::from(a)));
    }
    if let Some(b) = f.to {
        qb = qb.bind(("b", surrealdb::sql::Datetime::from(b)));
    }
    #[derive(Deserialize)]
    struct R {
        created_at: DateTime<Utc>,
        total: Decimal,
        cash_amount: Option<Decimal>,
        card_amount: Option<Decimal>,
    }
    let rows: Vec<R> = qb.await?.check()?.take(0)?;
    use std::collections::BTreeMap;
    let mut by_day: BTreeMap<String, DailySalesRow> = BTreeMap::new();
    for r in rows {
        let date = r.created_at.format("%Y-%m-%d").to_string();
        let entry = by_day.entry(date.clone()).or_insert(DailySalesRow {
            date: date.clone(),
            orders: 0,
            revenue: Decimal::ZERO,
            cash: Decimal::ZERO,
            card: Decimal::ZERO,
        });
        entry.orders += 1;
        entry.revenue += r.total;
        entry.cash += r.cash_amount.unwrap_or(Decimal::ZERO);
        entry.card += r.card_amount.unwrap_or(Decimal::ZERO);
    }
    Ok(by_day.into_values().collect())
}

/// Batches on hand that expire on or before `now + days` (default 30),
/// including already-expired ones (negative `days_to_expiry`). Tenant-scoped,
/// only `active` batches with `stock > 0`. Sorted by `expiry_date` ascending
/// — most urgent first. Product names are resolved in a second query (the
/// codebase deliberately avoids record-link traversal in SELECT under
/// kv-surrealkv).
pub async fn near_expiry(
    db: &Db,
    tenant: &Thing,
    f: NearExpiryFilters,
) -> DomainResult<Vec<NearExpiryRow>> {
    let days = f.days.unwrap_or(30).clamp(0, 3650);
    let now = Utc::now();
    let cutoff = now + chrono::Duration::days(days);

    #[derive(Deserialize)]
    struct B {
        id: Thing,
        product: Thing,
        batch_code: String,
        expiry_date: DateTime<Utc>,
        stock: i64,
    }
    let batches: Vec<B> = db
        .query(
            "SELECT id, product, batch_code, expiry_date, stock \
             FROM product_batch \
             WHERE tenant = $t AND active = true AND stock > 0 \
               AND expiry_date <= $c \
             ORDER BY expiry_date ASC",
        )
        .bind(("t", tenant.clone()))
        .bind(("c", surrealdb::sql::Datetime::from(cutoff)))
        .await?
        .check()?
        .take(0)?;

    if batches.is_empty() {
        return Ok(Vec::new());
    }

    // Resolve product names in one batched query keyed by the distinct ids.
    // `Thing` trips clippy `mutable_key_type` as a map/set key, so key the
    // dedup + lookup by its string form instead.
    use std::collections::{HashMap, HashSet};
    let ids: Vec<Thing> = {
        let mut seen: HashSet<String> = HashSet::new();
        batches
            .iter()
            .filter(|b| seen.insert(b.product.to_string()))
            .map(|b| b.product.clone())
            .collect()
    };
    #[derive(Deserialize)]
    struct P {
        id: Thing,
        name: String,
    }
    let prods: Vec<P> = db
        .query("SELECT id, name FROM product WHERE tenant = $t AND id IN $ids")
        .bind(("t", tenant.clone()))
        .bind(("ids", ids))
        .await?
        .check()?
        .take(0)?;
    let names: HashMap<String, String> = prods
        .into_iter()
        .map(|p| (p.id.to_string(), p.name))
        .collect();

    let today = now.date_naive();
    Ok(batches
        .into_iter()
        .map(|b| {
            let days_to_expiry = (b.expiry_date.date_naive() - today).num_days();
            NearExpiryRow {
                product_id: b.product.to_string(),
                product_name: names
                    .get(&b.product.to_string())
                    .cloned()
                    .unwrap_or_default(),
                batch_id: b.id.to_string(),
                batch_code: b.batch_code,
                expiry_date: b.expiry_date,
                stock: b.stock,
                days_to_expiry,
                expired: b.expiry_date <= now,
            }
        })
        .collect())
}

/// Daily gross-margin rollup. `revenue` = Σ `order_item.subtotal`;
/// `cost` = Σ `quantity * product.cost_price` over items with a known cost.
/// `refunded`/`cancelled` orders excluded. Tenant-scoped, sorted ascending
/// by UTC date. Three batched queries (orders → items → product costs) +
/// bucket in Rust — same kv-surrealkv-safe shape as `sales_daily`.
pub async fn margins_daily(
    db: &Db,
    tenant: &Thing,
    f: SalesReportFilters,
) -> DomainResult<Vec<DailyMarginRow>> {
    let mut conds = vec![
        "tenant = $t".to_string(),
        "status NOT IN ['refunded','cancelled']".to_string(),
    ];
    if f.from.is_some() {
        conds.push("created_at >= $a".to_string());
    }
    if f.to.is_some() {
        conds.push("created_at <= $b".to_string());
    }
    let sql = format!(
        "SELECT id, created_at FROM order WHERE {} ORDER BY created_at ASC",
        conds.join(" AND ")
    );
    let mut qb = db.query(sql).bind(("t", tenant.clone()));
    if let Some(a) = f.from {
        qb = qb.bind(("a", surrealdb::sql::Datetime::from(a)));
    }
    if let Some(b) = f.to {
        qb = qb.bind(("b", surrealdb::sql::Datetime::from(b)));
    }
    #[derive(Deserialize)]
    struct O {
        id: Thing,
        created_at: DateTime<Utc>,
    }
    let orders: Vec<O> = qb.await?.check()?.take(0)?;
    if orders.is_empty() {
        return Ok(Vec::new());
    }
    // order id (string) -> UTC date bucket.
    use std::collections::HashMap;
    let order_day: HashMap<String, String> = orders
        .iter()
        .map(|o| {
            (
                o.id.to_string(),
                o.created_at.format("%Y-%m-%d").to_string(),
            )
        })
        .collect();
    let order_ids: Vec<Thing> = orders.into_iter().map(|o| o.id).collect();

    #[derive(Deserialize)]
    struct It {
        order: Thing,
        product: Option<Thing>,
        quantity: i64,
        subtotal: Decimal,
    }
    let items: Vec<It> = db
        .query(
            "SELECT order, product, quantity, subtotal FROM order_item \
             WHERE tenant = $t AND order IN $ids",
        )
        .bind(("t", tenant.clone()))
        .bind(("ids", order_ids))
        .await?
        .check()?
        .take(0)?;
    if items.is_empty() {
        return Ok(Vec::new());
    }

    // Resolve product costs in one batched query (string-keyed: `Thing`
    // trips clippy `mutable_key_type` as a map key).
    use std::collections::HashSet;
    let pids: Vec<Thing> = {
        let mut seen: HashSet<String> = HashSet::new();
        items
            .iter()
            .filter_map(|i| i.product.clone())
            .filter(|p| seen.insert(p.to_string()))
            .collect()
    };
    #[derive(Deserialize)]
    struct P {
        id: Thing,
        cost_price: Option<Decimal>,
    }
    let costs: HashMap<String, Option<Decimal>> = if pids.is_empty() {
        HashMap::new()
    } else {
        let rows: Vec<P> = db
            .query("SELECT id, cost_price FROM product WHERE tenant = $t AND id IN $ids")
            .bind(("t", tenant.clone()))
            .bind(("ids", pids))
            .await?
            .check()?
            .take(0)?;
        rows.into_iter()
            .map(|p| (p.id.to_string(), p.cost_price))
            .collect()
    };

    use std::collections::BTreeMap;
    struct Acc {
        revenue: Decimal,
        cost: Decimal,
        without_cost: i64,
    }
    let mut by_day: BTreeMap<String, Acc> = BTreeMap::new();
    for it in items {
        let Some(day) = order_day.get(&it.order.to_string()) else {
            continue; // item of an excluded/out-of-range order
        };
        let e = by_day.entry(day.clone()).or_insert(Acc {
            revenue: Decimal::ZERO,
            cost: Decimal::ZERO,
            without_cost: 0,
        });
        e.revenue += it.subtotal;
        let unit_cost = it
            .product
            .as_ref()
            .and_then(|p| costs.get(&p.to_string()).cloned().flatten());
        match unit_cost {
            Some(c) => e.cost += c * Decimal::from(it.quantity),
            None => e.without_cost += 1,
        }
    }
    Ok(by_day
        .into_iter()
        .map(|(date, a)| {
            let margin = a.revenue - a.cost;
            let margin_pct = if a.revenue.is_zero() {
                Decimal::ZERO
            } else {
                (margin / a.revenue * Decimal::from(100)).round_dp(2)
            };
            DailyMarginRow {
                date,
                revenue: a.revenue,
                cost: a.cost,
                margin,
                margin_pct,
                items_without_cost: a.without_cost,
            }
        })
        .collect())
}

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

/// Builds a `WHERE` clause that **always** starts with `tenant = $tenant`
/// and `AND`-joins any `extra_clauses`. The tenant operand is appended to
/// `binds` so the caller only needs to bind its own extras.
///
/// Why a helper: report queries here used to compose the WHERE by pushing
/// `"tenant = $t"` as the first element of a `Vec<String>` at every call
/// site — a refactor could silently drop the tenant guard and leak rows
/// across tenants. Centralising it makes the guard structurally
/// guaranteed and unit-testable.
///
/// `extra_clauses` must NOT reference `tenant` directly — the debug
/// assertion catches obvious duplicates during development.
fn build_where_with_tenant(
    tenant: &Thing,
    extra_clauses: &[&str],
    binds: &mut Vec<(&'static str, surrealdb::sql::Value)>,
) -> String {
    debug_assert!(
        !extra_clauses.iter().any(|c| c.contains("tenant")),
        "extra_clauses must not reference `tenant` — it is added by the helper",
    );
    binds.push(("tenant", surrealdb::sql::Value::from(tenant.clone())));
    let mut clauses: Vec<&str> = Vec::with_capacity(1 + extra_clauses.len());
    clauses.push("tenant = $tenant");
    clauses.extend_from_slice(extra_clauses);
    format!("WHERE {}", clauses.join(" AND "))
}

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
    let session_opt: Option<Thing> = match input.cash_session.as_deref() {
        Some(s) if !s.is_empty() => {
            let t = thing(s)
                .map_err(|_| DomainError::Invalid(format!("cash_session id inválido: {s}")))?;
            if t.tb != "cash_register_session" {
                return Err(DomainError::Invalid(
                    "cash_session no es record<cash_register_session>".into(),
                ));
            }
            Some(t)
        }
        _ => None,
    };
    // A cash expense tied to a cash session is a real drawer withdrawal: post a
    // matching `cash_movement(tipo='retiro')` so arqueo/cierre reflect it.
    // Without this the expected drawer stays too high → phantom faltante at
    // cierre (the operator pays petty cash but the count never drops). Only
    // `cash` touches the drawer; bank/card/transfer do not. The session must
    // be open (mirrors `cash_register::add_movement`).
    let post_retiro = input.payment_method == "cash" && session_opt.is_some();
    // Hold the cash-session mutation lock across the status-check + the
    // CREATE-expense/CREATE-retiro transaction below, so this drawer write can't
    // interleave with a concurrent `close_session` (which would freeze `expected`
    // before our retiro lands → phantom faltante). Same lock `add_movement`/
    // `close_session` take. Bound to this scope; held only when we touch a drawer.
    let _drawer_guard = match (post_retiro, session_opt.as_ref()) {
        (true, Some(sid)) => Some(
            crate::cash_register::service::session_mutation_lock(tenant, &sid.to_string())
                .lock_owned()
                .await,
        ),
        _ => None,
    };
    if post_retiro {
        let sid = session_opt.clone().unwrap();
        let mut sr = db
            .query(
                "SELECT status FROM cash_register_session WHERE id = $id AND tenant = $t LIMIT 1",
            )
            .bind(("id", sid))
            .bind(("t", tenant.clone()))
            .await?
            .check()?;
        let status: Option<String> = sr.take((0, "status"))?;
        match status.as_deref() {
            Some("open") => {}
            Some(_) => {
                return Err(DomainError::Conflict(
                    "no se puede cargar un gasto en efectivo a una caja cerrada".into(),
                ))
            }
            None => {
                return Err(DomainError::Invalid(
                    "la sesión de caja no existe en este tenant".into(),
                ))
            }
        }
    }
    let retiro_reason = format!(
        "Gasto: {} — {}",
        input.category.trim(),
        input.description.trim()
    );
    let session_thing: surrealdb::sql::Value = match &session_opt {
        Some(t) => t.clone().into(),
        None => surrealdb::sql::Value::None,
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
    // When the expense draws cash from an open session, create the expense and
    // its `retiro` cash_movement in one BEGIN/COMMIT so a crash can't leave the
    // expense recorded without the drawer effect (or vice-versa). The expense
    // is always statement 0, so `r.take(0)` reads it back in both branches.
    let sql = if post_retiro {
        "BEGIN; \
         CREATE expense SET tenant=$t, category=$c, description=$d, \
            amount=$a, payment_method=$pm, cash_session=$cs, supplier=$su, \
            note=$nt, created_by=$cb, incurred_at=$ia RETURN AFTER; \
         CREATE cash_movement SET tenant=$t, session=$cs, tipo='retiro', \
            amount=$a, reason=$rsn, admin=$cb; \
         COMMIT;"
    } else {
        "CREATE expense SET tenant=$t, category=$c, description=$d, \
            amount=$a, payment_method=$pm, cash_session=$cs, supplier=$su, \
            note=$nt, created_by=$cb, incurred_at=$ia RETURN AFTER"
    };
    let mut r = db
        .query(sql)
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
        .bind(("rsn", retiro_reason))
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
        #[serde(default)]
        branch: Option<Thing>,
        batch_code: String,
        expiry_date: DateTime<Utc>,
        stock: i64,
    }
    // Sucursal: tri-estado, misma gramática que `inventory::repo::list_batches`
    // (ausente = todos los locales, `"none"`/`""` = casa matriz, id = ese local).
    // Si divergiera de la de lotes, el cliente tendría dos vocabularios para lo
    // mismo.
    let branch_thing = match f.branch.as_deref() {
        Some("none") | Some("") | None => None,
        Some(s) => Some(
            crate::stock::service::parse_branch(Some(s))?.ok_or_else(|| {
                DomainError::Invalid(format!("esperaba una sucursal, recibí {s}"))
            })?,
        ),
    };
    let branch_cond = match f.branch.as_deref() {
        Some("none") | Some("") => " AND branch = NONE",
        Some(_) => " AND branch = $br",
        None => "",
    };
    let batches: Vec<B> = db
        .query(format!(
            "SELECT id, product, branch, batch_code, expiry_date, stock \
             FROM product_batch \
             WHERE tenant = $t AND active = true AND stock > 0 \
               AND expiry_date <= $c{branch_cond} \
             ORDER BY expiry_date ASC"
        ))
        .bind(("t", tenant.clone()))
        .bind(("br", branch_thing))
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
    // Fetch directly by record-id (`FROM $ids`) = O(distinct products in the
    // report). `FROM product WHERE id IN $ids` full-scans the product table on
    // every call (BUG-perf-002 class). `WHERE tenant = $t` keeps the cross-tenant
    // guard — ids not owned by this tenant are dropped.
    let prods: Vec<P> = db
        .query("SELECT id, name FROM $ids WHERE tenant = $t")
        .bind(("t", tenant.clone()))
        .bind(("ids", ids))
        .await?
        .check()?
        .take(0)?;
    let names: HashMap<String, String> = prods
        .into_iter()
        .map(|p| (p.id.to_string(), p.name))
        .collect();

    // Nombres de sucursal, mismo patrón batcheado que los productos: una query
    // por reporte, no una por fila. Los lotes de casa matriz (`branch = NONE`)
    // no participan.
    let branch_ids: Vec<Thing> = {
        let mut seen: HashSet<String> = HashSet::new();
        batches
            .iter()
            .filter_map(|b| b.branch.as_ref())
            .filter(|t| seen.insert(t.to_string()))
            .cloned()
            .collect()
    };
    let branch_names: HashMap<String, String> = if branch_ids.is_empty() {
        HashMap::new()
    } else {
        let rows: Vec<P> = db
            .query("SELECT id, name FROM $ids WHERE tenant = $t")
            .bind(("t", tenant.clone()))
            .bind(("ids", branch_ids))
            .await?
            .check()?
            .take(0)?;
        rows.into_iter()
            .map(|p| (p.id.to_string(), p.name))
            .collect()
    };

    let today = now.date_naive();
    Ok(batches
        .into_iter()
        .map(|b| {
            let days_to_expiry = (b.expiry_date.date_naive() - today).num_days();
            let branch = b.branch.map(|t| t.to_string());
            NearExpiryRow {
                product_id: b.product.to_string(),
                product_name: names
                    .get(&b.product.to_string())
                    .cloned()
                    .unwrap_or_default(),
                batch_id: b.id.to_string(),
                batch_code: b.batch_code,
                branch_name: branch.as_ref().and_then(|id| branch_names.get(id).cloned()),
                branch,
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
        // Record-id fetch (`FROM $ids`) = O(distinct products), not a product
        // table full-scan (BUG-perf-002 class). `WHERE tenant = $t` retains the
        // cross-tenant guard.
        let rows: Vec<P> = db
            .query("SELECT id, cost_price FROM $ids WHERE tenant = $t")
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

/// Product sales ranking over the window with ABC (Pareto) classification.
/// `qty_sold` = Σ `order_item.quantity`; `revenue` = Σ `order_item.subtotal`.
/// `refunded`/`cancelled` excluded, tenant-scoped. Items are grouped by
/// product record; line items without a product fall back to grouping by
/// `product_name`. ABC is computed on cumulative revenue of the *full*
/// ranking (A ≤80%, B ≤95%, C rest) before `limit` truncates the result.
/// Same kv-surrealkv-safe shape as `margins_daily` (orders → items, bucket
/// in Rust).
pub async fn top_products(
    db: &Db,
    tenant: &Thing,
    f: TopProductsFilters,
) -> DomainResult<Vec<TopProductRow>> {
    let mut extra: Vec<&str> = vec!["status NOT IN ['refunded','cancelled']"];
    if f.from.is_some() {
        extra.push("created_at >= $a");
    }
    if f.to.is_some() {
        extra.push("created_at <= $b");
    }
    let mut binds: Vec<(&'static str, surrealdb::sql::Value)> = Vec::new();
    let where_clause = build_where_with_tenant(tenant, &extra, &mut binds);
    debug_assert!(where_clause.contains("tenant = $tenant"));
    let sql = format!("SELECT id FROM order {where_clause}");
    let mut qb = db.query(sql);
    for (k, v) in binds {
        qb = qb.bind((k, v));
    }
    if let Some(a) = f.from {
        qb = qb.bind(("a", surrealdb::sql::Datetime::from(a)));
    }
    if let Some(b) = f.to {
        qb = qb.bind(("b", surrealdb::sql::Datetime::from(b)));
    }
    #[derive(Deserialize)]
    struct O {
        id: Thing,
    }
    let orders: Vec<O> = qb.await?.check()?.take(0)?;
    if orders.is_empty() {
        return Ok(Vec::new());
    }
    let order_ids: Vec<Thing> = orders.into_iter().map(|o| o.id).collect();

    #[derive(Deserialize)]
    struct It {
        product: Option<Thing>,
        product_name: String,
        quantity: i64,
        subtotal: Decimal,
    }
    let items: Vec<It> = db
        .query(
            "SELECT product, product_name, quantity, subtotal FROM order_item \
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

    use std::collections::HashMap;
    struct Agg {
        product_id: Option<String>,
        product_name: String,
        qty: i64,
        revenue: Decimal,
    }
    // Group key: product id when present, else `name:<product_name>` so
    // catalogued and free-text lines never collide.
    let mut by_key: HashMap<String, Agg> = HashMap::new();
    for it in items {
        let pid = it.product.as_ref().map(|p| p.to_string());
        let key = pid
            .clone()
            .unwrap_or_else(|| format!("name:{}", it.product_name));
        let e = by_key.entry(key).or_insert(Agg {
            product_id: pid,
            product_name: it.product_name,
            qty: 0,
            revenue: Decimal::ZERO,
        });
        e.qty += it.quantity;
        e.revenue += it.subtotal;
    }

    let mut aggs: Vec<Agg> = by_key.into_values().collect();
    // Revenue desc, then qty desc, then name asc — stable, deterministic.
    aggs.sort_by(|x, y| {
        y.revenue
            .cmp(&x.revenue)
            .then(y.qty.cmp(&x.qty))
            .then(x.product_name.cmp(&y.product_name))
    });

    let total: Decimal = aggs.iter().map(|a| a.revenue).sum();
    let mut cumulative = Decimal::ZERO;
    let mut rows: Vec<TopProductRow> = Vec::with_capacity(aggs.len());
    for (i, a) in aggs.into_iter().enumerate() {
        let revenue_pct = if total.is_zero() {
            Decimal::ZERO
        } else {
            (a.revenue / total * Decimal::from(100)).round_dp(2)
        };
        cumulative += a.revenue;
        let cum_pct = if total.is_zero() {
            Decimal::ZERO
        } else {
            cumulative / total * Decimal::from(100)
        };
        let abc_class = if cum_pct <= Decimal::from(80) {
            "A"
        } else if cum_pct <= Decimal::from(95) {
            "B"
        } else {
            "C"
        };
        rows.push(TopProductRow {
            rank: (i as i64) + 1,
            product_id: a.product_id,
            product_name: a.product_name,
            qty_sold: a.qty,
            revenue: a.revenue,
            revenue_pct,
            abc_class: abc_class.to_string(),
        });
    }
    let limit = f.limit.unwrap_or(50).clamp(1, 500) as usize;
    rows.truncate(limit);
    Ok(rows)
}

/// Inventory turnover over the window. `qty_sold` = Σ `order_item.quantity`
/// for catalogued products on non-`refunded`/`cancelled` orders;
/// `turnover` = `qty_sold / product.stock` (current stock as proxy — no
/// historical snapshots kept, documented on [`StockRotationRow`]).
/// `turnover`/`days_of_inventory` are `None` when current stock ≤ 0.
/// `days_of_inventory` = `window_days / turnover`, only when both `from`
/// and `to` are supplied. Tenant-scoped, sorted by turnover desc (fastest
/// movers first; `None` turnover sorted last). Same kv-surrealkv-safe
/// shape as `top_products`.
pub async fn stock_rotation(
    db: &Db,
    tenant: &Thing,
    f: SalesReportFilters,
) -> DomainResult<Vec<StockRotationRow>> {
    let mut extra: Vec<&str> = vec!["status NOT IN ['refunded','cancelled']"];
    if f.from.is_some() {
        extra.push("created_at >= $a");
    }
    if f.to.is_some() {
        extra.push("created_at <= $b");
    }
    let mut binds: Vec<(&'static str, surrealdb::sql::Value)> = Vec::new();
    let where_clause = build_where_with_tenant(tenant, &extra, &mut binds);
    debug_assert!(where_clause.contains("tenant = $tenant"));
    let sql = format!("SELECT id FROM order {where_clause}");
    let mut qb = db.query(sql);
    for (k, v) in binds {
        qb = qb.bind((k, v));
    }
    if let Some(a) = f.from {
        qb = qb.bind(("a", surrealdb::sql::Datetime::from(a)));
    }
    if let Some(b) = f.to {
        qb = qb.bind(("b", surrealdb::sql::Datetime::from(b)));
    }
    #[derive(Deserialize)]
    struct O {
        id: Thing,
    }
    let orders: Vec<O> = qb.await?.check()?.take(0)?;
    if orders.is_empty() {
        return Ok(Vec::new());
    }
    let order_ids: Vec<Thing> = orders.into_iter().map(|o| o.id).collect();

    #[derive(Deserialize)]
    struct It {
        product: Option<Thing>,
        quantity: i64,
    }
    let items: Vec<It> = db
        .query(
            "SELECT product, quantity FROM order_item \
             WHERE tenant = $t AND order IN $ids",
        )
        .bind(("t", tenant.clone()))
        .bind(("ids", order_ids))
        .await?
        .check()?
        .take(0)?;

    use std::collections::HashMap;
    // Sum sold qty per catalogued product (string-keyed: `Thing` trips
    // clippy `mutable_key_type` as a map key). Free-text lines (no product)
    // have no stock to rotate — skipped.
    let mut sold: HashMap<String, i64> = HashMap::new();
    for it in items {
        if let Some(p) = it.product {
            *sold.entry(p.to_string()).or_insert(0) += it.quantity;
        }
    }
    if sold.is_empty() {
        return Ok(Vec::new());
    }

    let pids: Vec<Thing> = {
        let mut out = Vec::with_capacity(sold.len());
        for k in sold.keys() {
            if let Ok(t) = surrealdb::sql::thing(k) {
                out.push(t);
            }
        }
        out
    };
    #[derive(Deserialize)]
    struct P {
        id: Thing,
        name: String,
        stock: i64,
    }
    // Record-id fetch (`FROM $ids`) = O(distinct products sold), not a product
    // table full-scan (BUG-perf-002 class). `WHERE tenant = $t` retains the
    // cross-tenant guard.
    let prods: Vec<P> = db
        .query("SELECT id, name, stock FROM $ids WHERE tenant = $t")
        .bind(("t", tenant.clone()))
        .bind(("ids", pids))
        .await?
        .check()?
        .take(0)?;

    // Window length in days, only when both bounds are known.
    let window_days: Option<Decimal> = match (f.from, f.to) {
        (Some(a), Some(b)) => {
            let d = (b - a).num_days().max(1);
            Some(Decimal::from(d))
        }
        _ => None,
    };

    let mut rows: Vec<StockRotationRow> = prods
        .into_iter()
        .filter_map(|p| {
            let qty = *sold.get(&p.id.to_string())?;
            let (turnover, days_of_inventory) = if p.stock > 0 {
                let to = Decimal::from(qty) / Decimal::from(p.stock);
                let doi = window_days.and_then(|w| {
                    if to.is_zero() {
                        None
                    } else {
                        Some((w / to).round_dp(2))
                    }
                });
                (Some(to.round_dp(4)), doi)
            } else {
                (None, None)
            };
            Some(StockRotationRow {
                product_id: p.id.to_string(),
                product_name: p.name,
                qty_sold: qty,
                current_stock: p.stock,
                turnover,
                days_of_inventory,
            })
        })
        .collect();
    // Turnover desc; None (stock ≤ 0) sorted last, then by name for stability.
    rows.sort_by(|x, y| match (x.turnover, y.turnover) {
        (Some(xt), Some(yt)) => yt.cmp(&xt).then(x.product_name.cmp(&y.product_name)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => x.product_name.cmp(&y.product_name),
    });
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_tenant() -> Thing {
        Thing::from(("tenant", "abc"))
    }

    #[test]
    fn expenses_query_always_includes_tenant() {
        // No extras: the WHERE clause must still pin tenant.
        let mut binds: Vec<(&'static str, surrealdb::sql::Value)> = Vec::new();
        let t = fake_tenant();
        let w = build_where_with_tenant(&t, &[], &mut binds);
        assert_eq!(w, "WHERE tenant = $tenant");
        assert!(binds.iter().any(|(k, _)| *k == "tenant"));

        // With extras: tenant is first, extras are AND-joined after it.
        let mut binds: Vec<(&'static str, surrealdb::sql::Value)> = Vec::new();
        let w =
            build_where_with_tenant(&t, &["status NOT IN ['x']", "created_at >= $a"], &mut binds);
        assert!(w.contains("tenant = $tenant"));
        assert!(w.starts_with("WHERE tenant = $tenant AND "));
        assert!(w.contains("status NOT IN ['x']"));
        assert!(w.contains("created_at >= $a"));
    }

    #[test]
    #[should_panic(expected = "extra_clauses must not reference `tenant`")]
    fn build_where_panics_on_duplicate_tenant() {
        let mut binds: Vec<(&'static str, surrealdb::sql::Value)> = Vec::new();
        let t = fake_tenant();
        // Caller tries to add their own tenant clause — debug_assert fires.
        let _ = build_where_with_tenant(&t, &["tenant = $other"], &mut binds);
    }
}

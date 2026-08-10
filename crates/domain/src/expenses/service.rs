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

/// Daily sales rollup over `order`. Tenant-scoped. Returns one row per UTC
/// date in the range, sorted ascending.
///
/// `cash` es el efectivo **neto de vuelto**
/// ([`crate::invariants::cash_into_drawer`]) de las ventas que mueven efectivo,
/// la misma definición que el arqueo del cajón: los dos números tienen que
/// cerrar contra la misma plata.
///
/// Se excluye `cancelled` — documento anulado, nunca hubo venta. `refunded`
/// SÍ entra (migración 0049): la venta ocurrió, la plata entró, y lo devuelto
/// sale por [`DailySalesRow::refunds`] con la fecha en que volvió. Antes se
/// filtraba `refunded` también, y como `apply_refund` marcaba la orden entera
/// con cualquier devolución parcial, una venta de $15.000 con $5.000 devueltos
/// desaparecía COMPLETA: el dueño veía $0 en un día que vendió.
pub async fn sales_daily(
    db: &Db,
    tenant: &Thing,
    f: SalesReportFilters,
) -> DomainResult<Vec<DailySalesRow>> {
    let mut conds = vec!["tenant = $t".to_string(), "status != 'cancelled'".to_string()];
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
        "SELECT created_at, payment_method, total, cash_amount, card_amount FROM order \
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
        payment_method: String,
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
            refunds: Decimal::ZERO,
        });
        let card = r.card_amount.unwrap_or(Decimal::ZERO);
        entry.orders += 1;
        entry.revenue += r.total;
        // Efectivo NETO DE VUELTO, no `cash_amount` crudo: la columna "efectivo"
        // del día tiene que dar la misma plata que el arqueo del cajón (0046).
        // Sólo las ventas que mueven efectivo aportan: una debito/credito/
        // transferencia/fiado nunca llenó `cash_amount`, y si lo trae por datos
        // viejos no es plata en el cajón.
        if matches!(r.payment_method.as_str(), "pos_cash" | "pos_mixed") {
            entry.cash += crate::invariants::cash_into_drawer(r.total, r.cash_amount, card);
        }
        entry.card += card;
    }

    // Devoluciones del período, fechadas por la devolución. Consulta aparte
    // porque el día que vuelve la plata no es el día de la venta: un día sin
    // ventas pero con una devolución tiene que aparecer igual, con su fila.
    let mut rconds = vec!["tenant = $t".to_string()];
    if f.from.is_some() {
        rconds.push("created_at >= $a".to_string());
    }
    if f.to.is_some() {
        rconds.push("created_at <= $b".to_string());
    }
    let rsql = format!(
        "SELECT created_at, total_devuelto FROM devolucion WHERE {} ORDER BY created_at ASC",
        rconds.join(" AND ")
    );
    let mut rqb = db.query(rsql).bind(("t", tenant.clone()));
    if let Some(a) = f.from {
        rqb = rqb.bind(("a", surrealdb::sql::Datetime::from(a)));
    }
    if let Some(b) = f.to {
        rqb = rqb.bind(("b", surrealdb::sql::Datetime::from(b)));
    }
    #[derive(Deserialize)]
    struct D {
        created_at: DateTime<Utc>,
        total_devuelto: Decimal,
    }
    let devs: Vec<D> = rqb.await?.check()?.take(0)?;
    for d in devs {
        let date = d.created_at.format("%Y-%m-%d").to_string();
        let entry = by_day.entry(date.clone()).or_insert(DailySalesRow {
            date: date.clone(),
            orders: 0,
            revenue: Decimal::ZERO,
            cash: Decimal::ZERO,
            card: Decimal::ZERO,
            refunds: Decimal::ZERO,
        });
        entry.refunds += d.total_devuelto;
    }

    Ok(by_day.into_values().collect())
}

/// Ingresos del período por **método de pago** (efectivo / tarjeta /
/// transferencia / fiado / otro). Mismo filtro y misma exclusión de
/// `refunded`/`cancelled` que `sales_daily`, para que los dos reportes cuenten
/// la misma plata.
///
/// Reglas de atribución, explícitas porque son decisiones de negocio:
///   * `pos_cash` → todo a efectivo; `pos_debit`/`pos_credit` → todo a tarjeta.
///   * `pos_transferencia` (0043) → todo a transferencia. NO toca el efectivo
///     esperado del arqueo: ese sale de `cash_sales_running` (0030), que sólo
///     suma `pos_cash`/`pos_mixed`.
///   * `pos_mixed` → reparte `cash_amount` a efectivo y `card_amount` a
///     tarjeta; si por datos viejos no cuadra con el total, el resto cae en
///     `otro` en vez de desaparecer (el reporte nunca miente por omisión).
///   * `pos_fiado` → bucket propio: es ingreso devengado, todavía sin plata en
///     la mano.
///
/// Devuelve sólo los buckets con movimiento, ordenados de mayor a menor monto.
///
/// BRUTO de devoluciones, igual que el cajón (migración 0049): acá se responde
/// "¿por dónde entró la plata?", y entró por donde dice cada venta. Lo que se
/// devolvió se reporta con su propia fecha en
/// [`crate::expenses::model::DailySalesRow::refunds`] — restarlo de estos
/// buckets obligaría a decidir por cuál método salió y haría mutar el reporte
/// de ayer. `cancelled` sí queda afuera: documento anulado, nunca hubo venta.
pub async fn sales_by_method(
    db: &Db,
    tenant: &Thing,
    f: SalesReportFilters,
) -> DomainResult<Vec<SalesByMethodRow>> {
    let mut conds = vec!["tenant = $t".to_string(), "status != 'cancelled'".to_string()];
    if f.from.is_some() {
        conds.push("created_at >= $a".to_string());
    }
    if f.to.is_some() {
        conds.push("created_at <= $b".to_string());
    }
    // Sin `ORDER BY`: esto es un agregado, el orden de lectura no cambia el
    // resultado y la salida se ordena por monto al final. (De paso evita el
    // gotcha de SurrealDB — el campo del ORDER BY tiene que ir en la
    // proyección, y `created_at` acá no se necesita para nada más.)
    let sql = format!(
        "SELECT payment_method, total, cash_amount, card_amount FROM order WHERE {}",
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
        payment_method: String,
        total: Decimal,
        cash_amount: Option<Decimal>,
        card_amount: Option<Decimal>,
    }
    let rows: Vec<R> = qb.await?.check()?.take(0)?;

    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<&'static str, (i64, Decimal)> = BTreeMap::new();
    let add = |bucket: &'static str,
               amount: Decimal,
               buckets: &mut BTreeMap<&'static str, (i64, Decimal)>| {
        if amount.is_zero() {
            return;
        }
        let e = buckets.entry(bucket).or_insert((0, Decimal::ZERO));
        e.0 += 1;
        e.1 += amount;
    };
    for r in rows {
        match r.payment_method.as_str() {
            "pos_cash" => add("efectivo", r.total, &mut buckets),
            "pos_debit" | "pos_credit" => add("tarjeta", r.total, &mut buckets),
            "pos_transferencia" => add("transferencia", r.total, &mut buckets),
            "pos_fiado" => add("fiado", r.total, &mut buckets),
            "pos_mixed" => {
                let card = r.card_amount.unwrap_or(Decimal::ZERO);
                // El efectivo de una mixta viene con el vuelto incluido: lo que
                // ENTRÓ al negocio es a lo sumo lo que faltaba para el total.
                // Mismo invariante que el arqueo del cajón (0046).
                let cash_neto =
                    crate::invariants::cash_into_drawer(r.total, r.cash_amount, card);
                add("efectivo", cash_neto, &mut buckets);
                add("tarjeta", card.min(r.total), &mut buckets);
                let resto = r.total - cash_neto - card.min(r.total);
                add("otro", resto.max(Decimal::ZERO), &mut buckets);
            }
            _ => add("otro", r.total, &mut buckets),
        }
    }

    let label = |b: &str| match b {
        "efectivo" => "Efectivo",
        "tarjeta" => "Tarjeta",
        "transferencia" => "Transferencia",
        "fiado" => "Fiado",
        _ => "Otro",
    };
    let mut out: Vec<SalesByMethodRow> = buckets
        .into_iter()
        .map(|(method, (orders, amount))| SalesByMethodRow {
            method: method.to_string(),
            label: label(method).to_string(),
            orders,
            amount,
        })
        .collect();
    out.sort_by(|a, b| {
        b.amount
            .cmp(&a.amount)
            .then_with(|| a.method.cmp(&b.method))
    });
    Ok(out)
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

/// Unidades ya devueltas por `(orden, producto)`, para netear los reportes por
/// ítem (migración 0049).
///
/// Antes de la 0049 alcanzaba con filtrar `status != 'refunded'`: cualquier
/// devolución marcaba la orden entera y la orden desaparecía del reporte. Eso
/// era el bug — devolver 1 kilo de 3 borraba los 3 del ranking de productos.
/// Ahora la orden parcialmente devuelta se queda, así que hay que descontar
/// las unidades que volvieron, no la venta completa.
///
/// Una orden totalmente devuelta ya no aparece acá: sigue filtrada por
/// `status = 'refunded'`, que desde la 0049 significa devuelta ENTERA.
async fn refunded_units_by_order_product(
    db: &Db,
    tenant: &Thing,
    order_ids: &[Thing],
) -> DomainResult<std::collections::HashMap<(String, String), i64>> {
    use std::collections::HashMap;
    if order_ids.is_empty() {
        return Ok(HashMap::new());
    }
    #[derive(Deserialize)]
    struct D {
        id: Thing,
        order: Thing,
    }
    let devs: Vec<D> = db
        .query("SELECT id, order FROM devolucion WHERE tenant = $t AND order IN $ids")
        .bind(("t", tenant.clone()))
        .bind(("ids", order_ids.to_vec()))
        .await?
        .check()?
        .take(0)?;
    if devs.is_empty() {
        return Ok(HashMap::new());
    }
    let dev_order: HashMap<String, String> =
        devs.iter().map(|d| (d.id.to_string(), d.order.to_string())).collect();
    let dev_ids: Vec<Thing> = devs.into_iter().map(|d| d.id).collect();
    #[derive(Deserialize)]
    struct I {
        devolucion: Thing,
        product: Option<Thing>,
        quantity: i64,
    }
    let items: Vec<I> = db
        .query(
            "SELECT devolucion, product, quantity FROM devolucion_item \
             WHERE tenant = $t AND devolucion IN $ids",
        )
        .bind(("t", tenant.clone()))
        .bind(("ids", dev_ids))
        .await?
        .check()?
        .take(0)?;
    let mut out: HashMap<(String, String), i64> = HashMap::new();
    for i in items {
        let (Some(p), Some(o)) = (i.product, dev_order.get(&i.devolucion.to_string())) else {
            // Línea sin producto identificado: no se puede descontar de ningún
            // `order_item`. Queda como venta, igual que hoy.
            continue;
        };
        *out.entry((o.clone(), p.to_string())).or_default() += i.quantity;
    }
    Ok(out)
}

/// Descuenta de una línea de venta las unidades devueltas de ese `(orden,
/// producto)`, consumiendo el remanente compartido: el mismo producto puede
/// venir en más de una línea de la misma orden y la devolución no dice de cuál
/// salió. Devuelve `(unidades_netas, subtotal_neto)`.
///
/// El subtotal se prorratea por unidad en vez de recalcularse con el precio de
/// la devolución: el ranking tiene que reflejar lo que se COBRÓ en esa línea
/// (con su descuento), no lo que se reembolsó.
fn net_line(
    refunded: &mut std::collections::HashMap<(String, String), i64>,
    order: &str,
    product: Option<&Thing>,
    quantity: i64,
    subtotal: Decimal,
) -> (i64, Decimal) {
    let Some(p) = product else {
        return (quantity, subtotal);
    };
    let key = (order.to_string(), p.to_string());
    let Some(rem) = refunded.get_mut(&key) else {
        return (quantity, subtotal);
    };
    let take = quantity.min(*rem).max(0);
    *rem -= take;
    let net_qty = quantity - take;
    if take == 0 || quantity <= 0 {
        return (net_qty, subtotal);
    }
    let net_subtotal = subtotal * Decimal::from(net_qty) / Decimal::from(quantity);
    (net_qty, net_subtotal)
}

/// Daily gross-margin rollup. `revenue` = Σ `order_item.subtotal`;
/// `cost` = Σ `quantity * product.cost_price` over items with a known cost.
/// `refunded`/`cancelled` orders excluded — desde la 0049 `refunded` significa
/// devuelta ENTERA, y las unidades de una devolución PARCIAL se descuentan
/// línea por línea ([`refunded_units_by_order_product`]). Tenant-scoped, sorted
/// ascending by UTC date.
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
        .bind(("ids", order_ids.clone()))
        .await?
        .check()?
        .take(0)?;
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let mut refunded = refunded_units_by_order_product(db, tenant, &order_ids).await?;

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
        // Se descuentan las unidades devueltas de esta línea: la venta ocurrió,
        // pero la mercadería volvió y su costo tampoco se consumió.
        let (net_qty, net_subtotal) = net_line(
            &mut refunded,
            &it.order.to_string(),
            it.product.as_ref(),
            it.quantity,
            it.subtotal,
        );
        e.revenue += net_subtotal;
        let unit_cost = it
            .product
            .as_ref()
            .and_then(|p| costs.get(&p.to_string()).cloned().flatten());
        match unit_cost {
            Some(c) => e.cost += c * Decimal::from(net_qty),
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
        order: Thing,
        product: Option<Thing>,
        product_name: String,
        quantity: i64,
        subtotal: Decimal,
    }
    let items: Vec<It> = db
        .query(
            "SELECT order, product, product_name, quantity, subtotal FROM order_item \
             WHERE tenant = $t AND order IN $ids",
        )
        .bind(("t", tenant.clone()))
        .bind(("ids", order_ids.clone()))
        .await?
        .check()?
        .take(0)?;
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let mut refunded = refunded_units_by_order_product(db, tenant, &order_ids).await?;

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
        // Ranking de lo que se VENDIÓ y se quedó vendido: las unidades que
        // volvieron por una devolución parcial no cuentan. Una orden devuelta
        // entera ya quedó fuera por el filtro de `status`.
        let (net_qty, net_revenue) = net_line(
            &mut refunded,
            &it.order.to_string(),
            it.product.as_ref(),
            it.quantity,
            it.subtotal,
        );
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
        e.qty += net_qty;
        e.revenue += net_revenue;
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
        order: Thing,
        product: Option<Thing>,
        quantity: i64,
    }
    let items: Vec<It> = db
        .query(
            "SELECT order, product, quantity FROM order_item \
             WHERE tenant = $t AND order IN $ids",
        )
        .bind(("t", tenant.clone()))
        .bind(("ids", order_ids.clone()))
        .await?
        .check()?
        .take(0)?;
    let mut refunded = refunded_units_by_order_product(db, tenant, &order_ids).await?;

    use std::collections::HashMap;
    // Sum sold qty per catalogued product (string-keyed: `Thing` trips
    // clippy `mutable_key_type` as a map key). Free-text lines (no product)
    // have no stock to rotate — skipped.
    //
    // Netas de devoluciones parciales: la unidad que volvió está otra vez en
    // la góndola, así que no rotó. Contarla infla la rotación justo del
    // producto que la gente devuelve — el peor lugar donde equivocarse.
    let mut sold: HashMap<String, i64> = HashMap::new();
    for it in items {
        let (net_qty, _) = net_line(
            &mut refunded,
            &it.order.to_string(),
            it.product.as_ref(),
            it.quantity,
            Decimal::ZERO,
        );
        if let Some(p) = it.product {
            if net_qty > 0 {
                *sold.entry(p.to_string()).or_insert(0) += net_qty;
            }
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

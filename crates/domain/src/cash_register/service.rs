//! Cash register service. All operations tenant-scoped + audited.

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

fn dec_opt(d: Option<Decimal>) -> surrealdb::sql::Value {
    match d {
        Some(x) => dec_val(x),
        None => surrealdb::sql::Value::None,
    }
}

#[derive(Debug, Deserialize)]
struct SessionRow {
    id: Thing,
    user: Thing,
    register_name: String,
    opening_cash: Decimal,
    opening_notes: Option<String>,
    closing_cash_counted: Option<Decimal>,
    closing_cash_expected: Option<Decimal>,
    discrepancia: Option<Decimal>,
    closing_notes: Option<String>,
    opened_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    status: String,
}

impl From<SessionRow> for CashSessionDto {
    fn from(r: SessionRow) -> Self {
        Self {
            id: r.id.to_string(),
            user: r.user.to_string(),
            register_name: r.register_name,
            opening_cash: r.opening_cash,
            opening_notes: r.opening_notes,
            closing_cash_counted: r.closing_cash_counted,
            closing_cash_expected: r.closing_cash_expected,
            discrepancia: r.discrepancia,
            closing_notes: r.closing_notes,
            opened_at: r.opened_at,
            closed_at: r.closed_at,
            status: r.status,
        }
    }
}

/// Open a new register session for `user`. Fails if `user` already has an
/// open session — one drawer at a time per cashier.
pub async fn open_session(
    db: &Db,
    tenant: &Thing,
    user: &Thing,
    input: OpenSessionInput,
) -> DomainResult<CashSessionDto> {
    if input.opening_cash.is_sign_negative() {
        return Err(DomainError::Invalid("opening_cash debe ser >= 0".into()));
    }
    // Check for an existing open session (per user).
    #[derive(Deserialize)]
    struct C {
        count: i64,
    }
    let mut r = db
        .query(
            "SELECT count() AS count FROM cash_register_session \
             WHERE tenant=$t AND user=$u AND status='open' GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .bind(("u", user.clone()))
        .await?
        .check()?;
    let c: Option<C> = r.take(0)?;
    if c.map(|x| x.count).unwrap_or(0) > 0 {
        return Err(DomainError::Conflict(
            "el usuario ya tiene una caja abierta".into(),
        ));
    }
    let mut r = db
        .query(
            "CREATE cash_register_session SET tenant=$t, user=$u, \
             register_name=$rn, opening_cash=$oc, opening_notes=$nt, \
             status='open' RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("u", user.clone()))
        .bind(("rn", input.register_name))
        .bind(("oc", dec_val(input.opening_cash)))
        .bind(("nt", input.notes))
        .await?
        .check()?;
    let row: Option<SessionRow> = r.take(0)?;
    row.map(Into::into)
        .ok_or_else(|| DomainError::Other(anyhow::anyhow!("open session returned 0 rows")))
}

pub async fn list_sessions(
    db: &Db,
    tenant: &Thing,
    f: SessionFilters,
) -> DomainResult<Vec<CashSessionDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.status.is_some() {
        conds.push("status = $s".to_string());
    }
    if f.user.is_some() {
        conds.push("user = $u".to_string());
    }
    let limit = f.limit.unwrap_or(100).clamp(1, 500);
    let offset = f.offset.unwrap_or(0).max(0);
    let sql = format!(
        "SELECT * FROM cash_register_session WHERE {} \
         ORDER BY opened_at DESC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset
    );
    let mut qb = db.query(sql).bind(("t", tenant.clone()));
    if let Some(s) = f.status {
        qb = qb.bind(("s", s));
    }
    if let Some(u) = f.user {
        let ut = thing(&u).map_err(|_| DomainError::Invalid(format!("user id inválido: {u}")))?;
        qb = qb.bind(("u", ut));
    }
    let rows: Vec<SessionRow> = qb.await?.check()?.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_session(db: &Db, tenant: &Thing, id: &str) -> DomainResult<CashSessionDto> {
    let sid = thing(id).map_err(|_| DomainError::Invalid(format!("session id inválido: {id}")))?;
    if sid.tb != "cash_register_session" {
        return Err(DomainError::Invalid(
            "id no es cash_register_session".into(),
        ));
    }
    let mut r = db
        .query("SELECT * FROM cash_register_session WHERE id=$i AND tenant=$t LIMIT 1")
        .bind(("i", sid))
        .bind(("t", tenant.clone()))
        .await?
        .check()?;
    let row: Option<SessionRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn add_movement(
    db: &Db,
    tenant: &Thing,
    admin: Option<&Thing>,
    session_id: &str,
    input: CashMovementInput,
) -> DomainResult<CashMovementDto> {
    if input.tipo != "ingreso" && input.tipo != "retiro" {
        return Err(DomainError::Invalid(
            "tipo debe ser 'ingreso' o 'retiro'".into(),
        ));
    }
    if input.amount <= Decimal::ZERO {
        return Err(DomainError::Invalid("amount debe ser > 0".into()));
    }
    if input.reason.trim().is_empty() {
        return Err(DomainError::Invalid("reason requerido".into()));
    }
    let session = get_session(db, tenant, session_id).await?;
    if session.status != "open" {
        return Err(DomainError::Conflict(
            "no se puede mover dinero de una caja cerrada".into(),
        ));
    }
    let sid = thing(session_id).unwrap();
    #[derive(Deserialize)]
    struct Row {
        id: Thing,
        tipo: String,
        amount: Decimal,
        reason: String,
        created_at: DateTime<Utc>,
    }
    let mut r = db
        .query(
            "CREATE cash_movement SET tenant=$t, session=$s, tipo=$tp, \
             amount=$a, reason=$rs, admin=$ad RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("s", sid))
        .bind(("tp", input.tipo))
        .bind(("a", dec_val(input.amount)))
        .bind(("rs", input.reason))
        .bind(("ad", admin.cloned()))
        .await?
        .check()?;
    let row: Option<Row> = r.take(0)?;
    let row = row.ok_or_else(|| DomainError::Other(anyhow::anyhow!("movement returned 0")))?;
    Ok(CashMovementDto {
        id: row.id.to_string(),
        session: session_id.to_string(),
        tipo: row.tipo,
        amount: row.amount,
        reason: row.reason,
        created_at: row.created_at,
    })
}

pub async fn list_movements(
    db: &Db,
    tenant: &Thing,
    session_id: &str,
) -> DomainResult<Vec<CashMovementDto>> {
    let sid = thing(session_id)
        .map_err(|_| DomainError::Invalid(format!("session id inválido: {session_id}")))?;
    #[derive(Deserialize)]
    struct Row {
        id: Thing,
        session: Thing,
        tipo: String,
        amount: Decimal,
        reason: String,
        created_at: DateTime<Utc>,
    }
    let mut r = db
        .query(
            "SELECT id, session, tipo, amount, reason, created_at \
             FROM cash_movement WHERE tenant=$t AND session=$s \
             ORDER BY created_at ASC",
        )
        .bind(("t", tenant.clone()))
        .bind(("s", sid))
        .await?
        .check()?;
    let rows: Vec<Row> = r.take(0)?;
    Ok(rows
        .into_iter()
        .map(|r| CashMovementDto {
            id: r.id.to_string(),
            session: r.session.to_string(),
            tipo: r.tipo,
            amount: r.amount,
            reason: r.reason,
            created_at: r.created_at,
        })
        .collect())
}

/// Read the cash side of orders that hit this session: the running total of
/// `cash_amount` for `payment_method IN ('pos_cash','pos_mixed')` with
/// `status NOT IN ('refunded','cancelled')` and `created_at >= opened_at`.
///
/// O(1) field load of the maintained `cash_sales_running` aggregate (migration
/// 0030). Replaces the previous `math::sum(cash_amount)` scan over `order`,
/// which the planner served as a range scan over the whole shift window +
/// per-row doc fetch — O(orders in the shift), the cierre p99 budget violation
/// (BUG-perf-005). The field is kept exact by the `cash_sales_running_maint`
/// event on every order CREATE/UPDATE/DELETE, so this read is byte-identical to
/// the old scan for an open session evaluated at `now`.
async fn read_cash_running(db: &Db, tenant: &Thing, session: &Thing) -> DomainResult<Decimal> {
    #[derive(Deserialize)]
    struct S {
        cash_sales_running: Option<Decimal>,
    }
    let mut r = db
        .query(
            "SELECT cash_sales_running FROM cash_register_session \
             WHERE id=$i AND tenant=$t LIMIT 1",
        )
        .bind(("i", session.clone()))
        .bind(("t", tenant.clone()))
        .await?
        .check()?;
    let row: Option<S> = r.take(0)?;
    Ok(row
        .and_then(|r| r.cash_sales_running)
        .unwrap_or(Decimal::ZERO))
}

async fn sum_movements(
    db: &Db,
    tenant: &Thing,
    session: &Thing,
    tipo: &str,
) -> DomainResult<Decimal> {
    #[derive(Deserialize)]
    struct S {
        sum: Option<Decimal>,
    }
    let mut r = db
        .query(
            "SELECT math::sum(amount) AS sum FROM cash_movement \
             WHERE tenant=$t AND session=$s AND tipo=$tp GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .bind(("s", session.clone()))
        .bind(("tp", tipo.to_string()))
        .await?
        .check()?;
    let row: Option<S> = r.take(0)?;
    Ok(row.and_then(|r| r.sum).unwrap_or(Decimal::ZERO))
}

/// Compute the expected drawer cash for an open session as of now.
/// Helper that powers both the close path and the live `/arqueo` peek. The
/// cash-sales side is the maintained `cash_sales_running` aggregate (migration
/// 0030), so this no longer scans `order`.
pub async fn compute_summary(
    db: &Db,
    tenant: &Thing,
    session_id: &str,
) -> DomainResult<(CashSessionDto, Decimal, Decimal, Decimal, Decimal)> {
    let session = get_session(db, tenant, session_id).await?;
    let sid = thing(session_id).unwrap();
    let movements_in = sum_movements(db, tenant, &sid, "ingreso").await?;
    let movements_out = sum_movements(db, tenant, &sid, "retiro").await?;
    let cash_sales = read_cash_running(db, tenant, &sid).await?;
    let expected = crate::invariants::expected_drawer(
        session.opening_cash,
        cash_sales,
        movements_in,
        movements_out,
    );
    Ok((session, cash_sales, movements_in, movements_out, expected))
}

/// Close the session: record the counted cash, compute expected + discrepancy
/// vs all the cash motion since open, freeze the row.
pub async fn close_session(
    db: &Db,
    tenant: &Thing,
    session_id: &str,
    input: CloseSessionInput,
) -> DomainResult<CloseSummary> {
    let now = Utc::now();
    let (session, cash_sales, movements_in, movements_out, expected) =
        compute_summary(db, tenant, session_id).await?;
    if session.status != "open" {
        return Err(DomainError::Conflict("la caja ya está cerrada".into()));
    }
    if input.closing_cash_counted.is_sign_negative() {
        return Err(DomainError::Invalid(
            "closing_cash_counted debe ser >= 0".into(),
        ));
    }
    let discrepancia = crate::invariants::discrepancy(input.closing_cash_counted, expected);
    let sid = thing(session_id).unwrap();
    db.query(
        "UPDATE cash_register_session SET \
            closing_cash_counted=$cc, closing_cash_expected=$ce, \
            discrepancia=$d, closing_notes=$n, closed_at=$at, status='closed' \
         WHERE id=$i AND tenant=$t",
    )
    .bind(("cc", dec_val(input.closing_cash_counted)))
    .bind(("ce", dec_val(expected)))
    .bind(("d", dec_val(discrepancia)))
    .bind(("n", input.notes))
    .bind(("at", surrealdb::sql::Datetime::from(now)))
    .bind(("i", sid))
    .bind(("t", tenant.clone()))
    .await?
    .check()?;
    let session = get_session(db, tenant, session_id).await?;
    Ok(CloseSummary {
        session,
        cash_sales,
        movements_in,
        movements_out,
    })
}

/// Live arqueo: same numbers a close would compute, without freezing.
pub async fn arqueo(db: &Db, tenant: &Thing, session_id: &str) -> DomainResult<CloseSummary> {
    let (session, cash_sales, movements_in, movements_out, expected) =
        compute_summary(db, tenant, session_id).await?;
    // For an open session, surface the expected cash on the dto's
    // `closing_cash_expected` so the operator app can render it without a
    // separate field. Counted/discrepancia remain None.
    let mut session = session;
    session.closing_cash_expected = Some(expected);
    Ok(CloseSummary {
        session,
        cash_sales,
        movements_in,
        movements_out,
    })
}

// Suppress unused dec_opt — kept for parity with sales/repo helpers in case
// a future field needs it.
#[allow(dead_code)]
fn _keep_dec_opt(d: Option<Decimal>) -> surrealdb::sql::Value {
    dec_opt(d)
}

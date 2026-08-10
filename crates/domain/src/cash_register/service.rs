//! Cash register service. All operations tenant-scoped + audited.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use surrealdb::sql::{thing, Thing};
use tokio::sync::Mutex as AsyncMutex;

use crate::errors::{DomainError, DomainResult};

use super::model::*;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Per-(tenant,user) serialization of the cash-session OPEN critical section
/// (count-open-sessions check + CREATE). Without it `open_session` is a
/// check-then-act TOCTOU race: two concurrent opens for the same cashier
/// (a double-clicked "Abrir caja", two POS tabs) both read count=0 and both
/// CREATE, leaving the cashier with TWO open drawers. The two sessions then
/// split `cash_sales_running` and movements, so arqueo/cierre compute a bogus
/// `expected` and a phantom `discrepancia` — a money-integrity break. Same
/// approach as `sales::service::SALE_LOCKS` and `dte::caf::ASSIGN_LOCK`.
/// Different cashiers never share a lock, so throughput is unaffected.
static OPEN_LOCKS: OnceLock<std::sync::Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> =
    OnceLock::new();

fn open_session_lock(tenant: &Thing, user: &Thing) -> Arc<AsyncMutex<()>> {
    let locks = OPEN_LOCKS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut guard = locks.lock().expect("OPEN_LOCKS mutex poisoned");
    guard
        .entry(format!("{tenant}|{user}"))
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

/// Per-(tenant,session) serialization of the cash-session MUTATION critical
/// section, shared by `close_session` and `add_movement`. Both are check-then-act
/// over a single session's drawer: each reads `status='open'` then writes. Without
/// this lock two concurrent `close_session` calls (a double-clicked "Cerrar caja",
/// two tabs) both pass the open-check and both UPDATE, and a `cash_movement`
/// CREATE can slip in between a close's `compute_summary` snapshot and its UPDATE —
/// the movement is then frozen out of `expected`, so `discrepancia` is phantom: a
/// money-integrity break. Holding one lock across both paths makes the snapshot →
/// write atomic per session. `open_session` uses a separate per-(tenant,user) lock
/// (different invariant: one drawer per cashier); they never contend.
static SESSION_LOCKS: OnceLock<std::sync::Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> =
    OnceLock::new();

fn session_lock(tenant: &Thing, session_id: &str) -> Arc<AsyncMutex<()>> {
    let locks = SESSION_LOCKS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut guard = locks.lock().expect("SESSION_LOCKS mutex poisoned");
    guard
        .entry(format!("{tenant}|{session_id}"))
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

/// Acquire the per-(tenant,session) cash-mutation lock for callers OUTSIDE this
/// module that also write a session's drawer — specifically a cash `expense`
/// that posts a `retiro` cash_movement. Such a write must serialize against
/// `close_session` exactly like `add_movement` does, or the retiro can land after
/// a concurrent close froze `expected`, leaving the drawer dropped but `expected`
/// stale → phantom faltante. Caller holds `.lock().await` across its
/// status-check + write.
pub(crate) fn session_mutation_lock(tenant: &Thing, session_id: &str) -> Arc<AsyncMutex<()>> {
    session_lock(tenant, session_id)
}

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
    /// `default` porque las sesiones abiertas antes de la migración 0041 no
    /// tienen estos campos.
    #[serde(default)]
    register: Option<Thing>,
    #[serde(default)]
    branch: Option<Thing>,
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
            register: r.register.map(|t| t.to_string()),
            branch: r.branch.map(|t| t.to_string()),
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
    // Serialize the check-then-create per cashier so two concurrent opens can't
    // both pass the "no open session" check and create a second drawer. Held
    // only across the count + CREATE below; dropped at function return.
    let open_lock = open_session_lock(tenant, user);
    let _open_guard = open_lock.lock().await;
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
    // Sucursal de la caja (V2, migración 0041). La caja física manda: si el
    // cajero elige un `register`, la sesión hereda la sucursal DONDE ESA CAJA
    // ESTÁ, y el nombre de la caja se toma del record (no del texto libre) para
    // que el arqueo diga lo mismo que la configuración. Sin `register`, se
    // acepta una sucursal explícita. Sin ninguno de los dos: casa matriz, que es
    // el comportamiento de todo negocio de un solo local.
    let (register, branch, register_name) = match input.register.as_deref() {
        Some(rid) => {
            let rt =
                thing(rid).map_err(|_| DomainError::Invalid(format!("caja inválida: {rid}")))?;
            let reg = crate::branches::repo::get_register(db, tenant, &rt)
                .await?
                .ok_or_else(|| DomainError::Invalid("la caja elegida no existe".into()))?;
            if !reg.active {
                return Err(DomainError::Invalid(
                    "la caja elegida está desactivada".into(),
                ));
            }
            let b = crate::stock::service::parse_branch(reg.branch.as_deref())?;
            (Some(rt), b, reg.name)
        }
        None => {
            let b = crate::stock::service::parse_branch(input.branch.as_deref())?;
            crate::stock::service::ensure_branch(db, tenant, b.as_ref(), "la sucursal").await?;
            (None, b, input.register_name)
        }
    };

    let mut r = db
        .query(
            "CREATE cash_register_session SET tenant=$t, user=$u, \
             register_name=$rn, register=$rg, branch=$br, \
             opening_cash=$oc, opening_notes=$nt, \
             status='open' RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("u", user.clone()))
        .bind(("rn", register_name))
        .bind(("rg", register))
        .bind(("br", branch))
        .bind(("oc", dec_val(input.opening_cash)))
        .bind(("nt", input.notes))
        .await?
        .check()?;
    let row: Option<SessionRow> = r.take(0)?;
    row.map(Into::into)
        .ok_or_else(|| DomainError::Other(anyhow::anyhow!("open session returned 0 rows")))
}

/// Sucursal de la sesión de caja ABIERTA de un cajero, si tiene una.
///
/// Es la fuente #2 de la sucursal de una venta (después del `branch` explícito
/// del request): con esto "la venta descuenta de la sucursal de la caja" sale
/// solo, sin que el POS tenga que mandar nada. `Ok(None)` tanto si el cajero no
/// tiene caja abierta como si la tiene en la casa matriz — en ambos casos la
/// venta cae al bucket casa matriz, que es lo correcto.
pub async fn open_session_branch(
    db: &Db,
    tenant: &Thing,
    user: &Thing,
) -> DomainResult<Option<Thing>> {
    #[derive(Deserialize)]
    struct Row {
        #[serde(default)]
        branch: Option<Thing>,
    }
    let mut r = db
        .query(
            // `opened_at` va en la proyección a propósito: SurrealDB exige que el
            // campo del ORDER BY esté seleccionado ("Missing order idiom").
            "SELECT branch, opened_at FROM cash_register_session \
             WHERE tenant=$t AND user=$u AND status='open' \
             ORDER BY opened_at DESC LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("u", user.clone()))
        .await?
        .check()?;
    let row: Option<Row> = r.take(0)?;
    Ok(row.and_then(|r| r.branch))
}

/// Qué caja se lleva el efectivo de una venta que cobra `user` (migración 0050).
///
/// Existe porque `cash_sales_running_maint` no puede scopear por caja si la fila
/// de `order` no dice cuál la tomó. Hasta 0049 el evento le aplicaba el delta a
/// TODAS las cajas abiertas del tenant, así que con dos cajeros una venta de
/// $5.000 le entraba a las dos.
///
/// **Precedencia, y las tres ramas están ahí por una razón concreta:**
///
/// 1. **La caja abierta del vendedor.** Es el caso exacto y el que arregla el
///    defecto: cada cajero tiene a lo sumo una (`open_session` rechaza la
///    segunda del mismo usuario, bajo lock por `(tenant, user)`), así que no hay
///    ambigüedad posible.
/// 2. **Si el vendedor no tiene y el tenant tiene EXACTAMENTE UNA abierta, esa.**
///    No es una concesión: es el día real de la dueña que abre la caja a la
///    mañana y del empleado que vende. Hoy esa venta le entra a la caja de la
///    dueña —por accidente, porque el evento se las aplica a todas— y **está
///    bien que le entre**: la plata terminó en ese cajón. Un filtro estricto por
///    vendedor la dejaría sin caja y le inventaría un faltante del tamaño del
///    día. Con una sola caja abierta no hay a quién más asignarla.
/// 3. **Ninguna.** Ni el vendedor tiene caja ni hay una sola candidata. La venta
///    existe igual; simplemente no le entra a ningún arqueo, que es la verdad:
///    nadie abrió un cajón donde asentarla.
///
/// Neto: con 0 o 1 caja abierta el comportamiento es **idéntico** al de antes de
/// 0050. Sólo cambia con 2 o más, que es exactamente donde estaba mal.
pub async fn sale_cash_session(
    db: &Db,
    tenant: &Thing,
    user: Option<&Thing>,
) -> DomainResult<Option<Thing>> {
    #[derive(Deserialize)]
    struct Row {
        id: Thing,
    }
    if let Some(u) = user {
        let mut r = db
            .query(
                // `opened_at` en la proyección por lo mismo que en
                // `open_session_branch`: SurrealDB exige que el campo del ORDER
                // BY esté seleccionado ("Missing order idiom").
                "SELECT id, opened_at FROM cash_register_session \
                 WHERE tenant=$t AND user=$u AND status='open' \
                 ORDER BY opened_at DESC LIMIT 1",
            )
            .bind(("t", tenant.clone()))
            .bind(("u", u.clone()))
            .await?
            .check()?;
        let row: Option<Row> = r.take(0)?;
        if let Some(row) = row {
            return Ok(Some(row.id));
        }
    }
    // `LIMIT 2`, no `LIMIT 1`: hace falta distinguir "hay exactamente una" de
    // "hay varias". Con `LIMIT 1` las dos se ven igual y elegiríamos una caja al
    // azar, que es una forma más silenciosa del mismo bug.
    let mut r = db
        .query(
            "SELECT id, opened_at FROM cash_register_session \
             WHERE tenant=$t AND status='open' \
             ORDER BY opened_at DESC LIMIT 2",
        )
        .bind(("t", tenant.clone()))
        .await?
        .check()?;
    let rows: Vec<Row> = r.take(0)?;
    if rows.len() == 1 {
        return Ok(Some(rows.into_iter().next().unwrap().id));
    }
    Ok(None)
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
    // Serialize against a concurrent close of the same session: without the lock
    // the status check below can pass while a close is mid-flight, and the CREATE
    // then lands after the close froze `expected` — a movement excluded from the
    // drawer math. Held across the check + CREATE; dropped at return.
    let lock = session_lock(tenant, session_id);
    let _guard = lock.lock().await;
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

/// Read the cash side of orders that hit this session: the running total of the
/// **efectivo neto de vuelto** ([`crate::invariants::cash_into_drawer`]) for
/// `payment_method IN ('pos_cash','pos_mixed')` with `status NOT IN
/// ('refunded','cancelled')` and `created_at >= opened_at`.
///
/// O(1) field load of the maintained `cash_sales_running` aggregate (migration
/// 0030, corregida por 0046). Replaces the previous live scan over `order`,
/// which the planner served as a range scan over the whole shift window +
/// per-row doc fetch — O(orders in the shift), the cierre p99 budget violation
/// (BUG-perf-005). The field is kept exact by the `cash_sales_running_maint`
/// event on every order CREATE/UPDATE/DELETE.
///
/// Ojo al leer el historial: hasta 0046 el evento sumaba `cash_amount` crudo,
/// que es lo ENTREGADO. Un `math::sum(cash_amount)` ya no es el oráculo de este
/// campo — el oráculo es `cash_into_drawer` orden por orden.
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
    // Serialize the snapshot → freeze against a concurrent close/movement of the
    // same session. Two concurrent closes without this lock both pass the
    // status check below and both UPDATE (the second silently overwriting the
    // first's counted/discrepancia). Held across compute + UPDATE.
    let lock = session_lock(tenant, session_id);
    let _guard = lock.lock().await;
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
    // `AND status='open'` is a compare-and-swap guard: defense-in-depth so a
    // freeze can never overwrite an already-closed row even if the lock above
    // were bypassed. Under the lock the status check is authoritative.
    db.query(
        "UPDATE cash_register_session SET \
            closing_cash_counted=$cc, closing_cash_expected=$ce, \
            discrepancia=$d, closing_notes=$n, closed_at=$at, status='closed' \
         WHERE id=$i AND tenant=$t AND status='open'",
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

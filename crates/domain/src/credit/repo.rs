//! Cuenta corriente / fiado persistence. Append-only ledger; el saldo se calcula
//! sumando movimientos (nunca se muta un total). Tenant-scoped: el caller pasa el
//! tenant `Thing` del JWT.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::{CustomerAccountDto, DebtorRow, DebtorsReport, LedgerEntryDto};

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

fn dec_val(d: Decimal) -> surrealdb::sql::Value {
    surrealdb::sql::Number::from(d).into()
}

#[derive(Debug, Deserialize)]
struct LedgerRow {
    id: Thing,
    kind: String,
    amount: Decimal,
    order: Option<Thing>,
    note: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<LedgerRow> for LedgerEntryDto {
    fn from(r: LedgerRow) -> Self {
        Self {
            id: r.id.to_string(),
            kind: r.kind,
            amount: r.amount,
            order: r.order.map(|t| t.to_string()),
            note: r.note,
            created_at: r.created_at,
        }
    }
}

/// Registrar el CARGO de una venta fiada. Idempotente por `order`: si ya existe
/// un cargo para esa venta (reintento del POS), no duplica — devuelve `Ok`
/// silencioso. Llamado desde el sale service tras confirmar la orden.
pub async fn post_cargo(
    db: &Db,
    tenant: &Thing,
    customer: &Thing,
    order: &Thing,
    amount: Decimal,
    created_by: Option<&Thing>,
) -> DomainResult<()> {
    // Idempotencia: ¿ya hay cargo para esta venta?
    let mut existing = db
        .query("SELECT id FROM customer_ledger WHERE tenant = $t AND order = $o LIMIT 1")
        .bind(("t", tenant.clone()))
        .bind(("o", order.clone()))
        .await?;
    let hit: Option<Thing> = existing.take((0, "id"))?;
    if hit.is_some() {
        return Ok(());
    }
    db.query(
        "CREATE customer_ledger SET tenant = $t, customer = $c, kind = 'cargo', \
         amount = $amount, order = $o, created_by = $by",
    )
    .bind(("t", tenant.clone()))
    .bind(("c", customer.clone()))
    .bind(("amount", dec_val(amount)))
    .bind(("o", order.clone()))
    .bind(("by", created_by.cloned()))
    .await?;
    Ok(())
}

/// Registrar un ABONO (pago del cliente contra su deuda). Devuelve el movimiento.
pub async fn post_abono(
    db: &Db,
    tenant: &Thing,
    customer: &Thing,
    amount: Decimal,
    cash_session: Option<&Thing>,
    note: Option<&str>,
    created_by: Option<&Thing>,
) -> DomainResult<LedgerEntryDto> {
    let mut r = db
        .query(
            "CREATE customer_ledger SET tenant = $t, customer = $c, kind = 'abono', \
             amount = $amount, cash_session = $cs, note = $note, created_by = $by RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("c", customer.clone()))
        .bind(("amount", dec_val(amount)))
        .bind(("cs", cash_session.cloned()))
        .bind(("note", note.map(|s| s.to_string())))
        .bind(("by", created_by.cloned()))
        .await?;
    let row: Option<LedgerRow> = r.take(0)?;
    row.map(LedgerEntryDto::from)
        .ok_or_else(|| DomainError::Invalid("no se pudo registrar el abono".into()))
}

/// Estado de cuenta: saldo + totales + movimientos (más recientes primero).
pub async fn account(
    db: &Db,
    tenant: &Thing,
    customer: &Thing,
) -> DomainResult<CustomerAccountDto> {
    let mut r = db
        .query(
            "SELECT id, kind, amount, order, note, created_at FROM customer_ledger \
             WHERE tenant = $t AND customer = $c ORDER BY created_at DESC",
        )
        .bind(("t", tenant.clone()))
        .bind(("c", customer.clone()))
        .await?;
    let rows: Vec<LedgerRow> = r.take(0)?;

    let mut total_charged = Decimal::ZERO;
    let mut total_paid = Decimal::ZERO;
    for row in &rows {
        if row.kind == "cargo" {
            total_charged += row.amount;
        } else {
            total_paid += row.amount;
        }
    }
    Ok(CustomerAccountDto {
        customer: customer.to_string(),
        balance: total_charged - total_paid,
        total_charged,
        total_paid,
        entries: rows.into_iter().map(LedgerEntryDto::from).collect(),
    })
}

/// Saldo adeudado (cargos - abonos). Positivo = el cliente debe.
pub async fn balance(db: &Db, tenant: &Thing, customer: &Thing) -> DomainResult<Decimal> {
    Ok(account(db, tenant, customer).await?.balance)
}

#[derive(Debug, Deserialize)]
struct DebtorLedgerRow {
    customer: Thing,
    kind: String,
    amount: Decimal,
    created_at: DateTime<Utc>,
    #[serde(default)]
    customer_name: Option<String>,
    #[serde(default)]
    customer_phone: Option<String>,
}

/// "¿Cuánto me deben?" — cuentas por cobrar del negocio. Recorre el ledger del
/// tenant una vez y agrupa por cliente; sólo salen los que quedan con saldo > 0
/// (un cliente que ya pagó todo no es un deudor). Ordenado por saldo desc: lo
/// primero que el dueño quiere ver es quién le debe más.
pub async fn debtors(db: &Db, tenant: &Thing) -> DomainResult<DebtorsReport> {
    let mut r = db
        .query(
            "SELECT customer, kind, amount, created_at, \
             customer.name AS customer_name, customer.phone AS customer_phone \
             FROM customer_ledger WHERE tenant = $t ORDER BY created_at ASC",
        )
        .bind(("t", tenant.clone()))
        .await?;
    let rows: Vec<DebtorLedgerRow> = r.take(0)?;

    // Acumulado por cliente preservando el orden de aparición (BTreeMap por id
    // string para un resultado determinístico antes del sort final).
    let mut by_customer: BTreeMap<String, DebtorRow> = BTreeMap::new();
    for row in rows {
        let key = row.customer.to_string();
        let entry = by_customer.entry(key.clone()).or_insert_with(|| DebtorRow {
            customer: key,
            name: row
                .customer_name
                .clone()
                .unwrap_or_else(|| "(cliente)".into()),
            phone: row.customer_phone.clone(),
            balance: Decimal::ZERO,
            last_movement: row.created_at,
        });
        if row.kind == "cargo" {
            entry.balance += row.amount;
        } else {
            entry.balance -= row.amount;
        }
        if row.created_at > entry.last_movement {
            entry.last_movement = row.created_at;
        }
    }

    let mut rows: Vec<DebtorRow> = by_customer
        .into_values()
        .filter(|d| d.balance > Decimal::ZERO)
        .collect();
    // Mayor deuda primero (lo que el dueño quiere ver arriba).
    rows.sort_by_key(|d| std::cmp::Reverse(d.balance));

    let total: Decimal = rows.iter().map(|d| d.balance).sum();
    Ok(DebtorsReport {
        total_por_cobrar: total,
        debtor_count: rows.len(),
        rows,
    })
}

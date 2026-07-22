//! Cuenta corriente / fiado persistence. Append-only ledger; el saldo se calcula
//! sumando movimientos (nunca se muta un total). Tenant-scoped: el caller pasa el
//! tenant `Thing` del JWT.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::{CustomerAccountDto, LedgerEntryDto};

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

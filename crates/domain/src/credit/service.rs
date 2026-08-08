//! Cuenta corriente / fiado service: validaciones sobre el ledder (repo puro).

use rust_decimal::Decimal;
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::{CustomerAccountDto, LedgerEntryDto};
use super::repo;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Registrar un abono validado: monto positivo y que no supere la deuda vigente
/// (no se puede abonar más de lo que se debe → dejaría el saldo negativo, que en
/// fiado no tiene sentido). Devuelve el movimiento creado.
#[allow(clippy::too_many_arguments)]
pub async fn record_abono(
    db: &Db,
    tenant: &Thing,
    customer: &Thing,
    amount: Decimal,
    cash_session: Option<&Thing>,
    note: Option<&str>,
    created_by: Option<&Thing>,
) -> DomainResult<LedgerEntryDto> {
    if amount <= Decimal::ZERO {
        return Err(DomainError::Invalid(
            "el abono debe ser mayor a cero".into(),
        ));
    }
    let debt = repo::balance(db, tenant, customer).await?;
    if debt <= Decimal::ZERO {
        return Err(DomainError::Invalid(
            "el cliente no tiene deuda pendiente".into(),
        ));
    }
    if amount > debt {
        return Err(DomainError::Invalid(format!(
            "el abono ({amount}) supera la deuda pendiente ({debt})"
        )));
    }
    repo::post_abono(db, tenant, customer, amount, cash_session, note, created_by).await
}

/// Estado de cuenta del cliente (passthrough al repo).
pub async fn account(
    db: &Db,
    tenant: &Thing,
    customer: &Thing,
) -> DomainResult<CustomerAccountDto> {
    repo::account(db, tenant, customer).await
}

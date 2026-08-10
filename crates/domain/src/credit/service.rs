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
    // Abono en efectivo con caja: la plata entra al cajón, así que el arqueo
    // tiene que verla. Se sostiene el lock de mutación de la sesión desde el
    // chequeo de estado hasta que commitea la transacción del repo — el mismo
    // que toman `cash_register::add_movement` y `close_session`, para que un
    // cierre concurrente no congele el esperado justo antes de que aterrice el
    // ingreso. Mismo patrón que `expenses::service::create_expense`.
    let _drawer_guard = match cash_session {
        Some(sid) => {
            let guard = crate::cash_register::service::session_mutation_lock(
                tenant,
                &sid.to_string(),
            )
            .lock_owned()
            .await;
            let mut sr = db
                .query(
                    "SELECT status FROM cash_register_session WHERE id = $id AND tenant = $t LIMIT 1",
                )
                .bind(("id", sid.clone()))
                .bind(("t", tenant.clone()))
                .await?;
            let status: Option<String> = sr.take((0, "status"))?;
            match status.as_deref() {
                Some("open") => {}
                Some(_) => {
                    return Err(DomainError::Conflict(
                        "no se puede abonar en efectivo a una caja cerrada".into(),
                    ))
                }
                None => {
                    return Err(DomainError::Invalid(
                        "la sesión de caja no existe en este tenant".into(),
                    ))
                }
            }
            Some(guard)
        }
        None => None,
    };
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

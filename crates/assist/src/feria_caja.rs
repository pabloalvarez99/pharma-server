//! Apertura implícita del puesto en vertical feria.
//!
//! En farmacia la caja es un ritual explícito (fondo + arqueo). En feria el
//! puesto se abre al cobrar el primer día: $0 de fondo, sin pedir «abre la
//! caja con $50.000». El cash de la venta entra al arqueo del día (migración
//! 0050 exige sesión abierta para contarlo).

use rust_decimal::Decimal;
use surrealdb::sql::Thing;

use db::Db;
use domain::cash_register::model::{OpenSessionInput, SessionFilters};
use domain::cash_register::service as caja;
use domain::errors::DomainError;
use domain::provisioning::SETTING_VERTICAL;
use domain::rubro::pack_for;
use domain::sales::service as sales;
use domain::DomainResult;

/// `true` cuando el tenant es feria / agent-home (espejo de `esFeria()` en
/// Android `ServiciosDeRubro.kt`: `pack.features.agent_home || pack.rubro == "feria"`).
pub async fn es_feria(db: &Db, tenant: &Thing) -> DomainResult<bool> {
    let raw = sales::get_setting(db, tenant, SETTING_VERTICAL)
        .await?
        .map(|s| s.value)
        .unwrap_or_default();
    let pack = pack_for(&raw);
    Ok(pack.features.agent_home || pack.rubro == "feria")
}

/// Hay al menos una sesión de caja `open` en el tenant (cualquier cajero).
pub async fn hay_caja_abierta(db: &Db, tenant: &Thing) -> DomainResult<bool> {
    let open = caja::list_sessions(
        db,
        tenant,
        SessionFilters {
            status: Some("open".into()),
            user: None,
            limit: Some(1),
            offset: None,
        },
    )
    .await?;
    Ok(!open.is_empty())
}

/// En feria, si no hay caja abierta, abre el puesto con $0 a nombre del actor.
///
/// - Ya hay sesión open → Ok (no-op).
/// - `Conflict("el usuario ya tiene una caja abierta")` → Ok (carrera / double-tap).
/// - Cualquier otro error de dominio se propaga.
pub async fn asegurar_caja_feria(db: &Db, tenant: &Thing, actor: &Thing) -> DomainResult<()> {
    if hay_caja_abierta(db, tenant).await? {
        return Ok(());
    }
    match caja::open_session(
        db,
        tenant,
        actor,
        OpenSessionInput {
            register_name: "puesto".into(),
            register: None,
            branch: None,
            opening_cash: Decimal::ZERO,
            notes: Some("Apertura implícita feria".into()),
        },
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(DomainError::Conflict(msg)) if msg.contains("el usuario ya tiene una caja abierta") => {
            Ok(())
        }
        Err(e) => Err(e),
    }
}

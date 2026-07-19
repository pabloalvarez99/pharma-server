//! Prescriptions + pharmacist-shift business logic.
//!
//! Prescriptions are immutable at this layer: only `create` and reads are
//! exposed (Ley 20.000). Controlled-drug entries require doctor identification.

use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::*;
use super::repo;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

fn parse_thing(s: &str) -> DomainResult<Thing> {
    surrealdb::sql::thing(s).map_err(|_| DomainError::Invalid(format!("id inválido: {s}")))
}

fn parse_typed(s: &str, table: &str) -> DomainResult<Thing> {
    let t = parse_thing(s)?;
    if t.tb != table {
        return Err(DomainError::Invalid(format!(
            "id debe ser de la tabla `{table}`"
        )));
    }
    Ok(t)
}

// --- prescriptions ---------------------------------------------------------

pub async fn create_prescription(
    db: &Db,
    tenant: &Thing,
    mut input: NewPrescription,
) -> DomainResult<PrescriptionDto> {
    input.patient_name = input.patient_name.trim().to_string();
    input.patient_rut = input.patient_rut.trim().to_string();
    if input.patient_name.is_empty() {
        return Err(DomainError::Invalid("nombre del paciente requerido".into()));
    }
    if input.patient_rut.is_empty() {
        return Err(DomainError::Invalid("RUT del paciente requerido".into()));
    }
    if input.controlled {
        let doctor_ok = input
            .doctor_name
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
            && input
                .doctor_rut
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
        if !doctor_ok {
            return Err(DomainError::Invalid(
                "receta controlada requiere nombre y RUT del médico".into(),
            ));
        }
    }
    let product = match input.product.as_deref() {
        Some(s) if !s.is_empty() => {
            let t = parse_typed(s, "product")?;
            if !repo::product_belongs(db, tenant, &t).await? {
                return Err(DomainError::Invalid(
                    "el producto no existe en este tenant".into(),
                ));
            }
            Some(t)
        }
        _ => None,
    };
    let customer = match input.customer.as_deref() {
        Some(s) if !s.is_empty() => {
            let t = parse_typed(s, "customer")?;
            if !repo::customer_belongs(db, tenant, &t).await? {
                return Err(DomainError::Invalid(
                    "el cliente no existe en este tenant".into(),
                ));
            }
            Some(t)
        }
        _ => None,
    };
    repo::create_prescription(db, tenant, product, customer, &input).await
}

pub async fn list_prescriptions(
    db: &Db,
    tenant: &Thing,
    filters: PrescriptionFilters,
) -> DomainResult<Vec<PrescriptionDto>> {
    repo::list_prescriptions(db, tenant, &filters).await
}

pub async fn get_prescription(db: &Db, tenant: &Thing, id: &str) -> DomainResult<PrescriptionDto> {
    let id = parse_thing(id)?;
    repo::get_prescription(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)
}

/// Controlled-drug ledger (libro de recetas). Wraps `list_prescriptions` with
/// `controlled = true` enforced.
pub async fn list_controlled(
    db: &Db,
    tenant: &Thing,
    mut filters: PrescriptionFilters,
) -> DomainResult<Vec<PrescriptionDto>> {
    filters.controlled = Some(true);
    repo::list_prescriptions(db, tenant, &filters).await
}

// --- pharmacist shifts -----------------------------------------------------

pub async fn create_shift(
    db: &Db,
    tenant: &Thing,
    input: NewPharmacistShift,
) -> DomainResult<PharmacistShiftDto> {
    let user = parse_typed(&input.user, "user")?;
    if !repo::user_belongs(db, tenant, &user).await? {
        return Err(DomainError::Invalid(
            "el usuario no existe en este tenant".into(),
        ));
    }
    repo::create_shift(db, tenant, &user, input.started_at, input.notes.as_deref()).await
}

pub async fn list_shifts(
    db: &Db,
    tenant: &Thing,
    filters: ShiftFilters,
) -> DomainResult<Vec<PharmacistShiftDto>> {
    repo::list_shifts(db, tenant, &filters).await
}

pub async fn update_shift(
    db: &Db,
    tenant: &Thing,
    id: &str,
    patch: UpdatePharmacistShift,
) -> DomainResult<PharmacistShiftDto> {
    let pid = parse_thing(id)?;
    // Reject closing a shift that is already closed.
    if patch.ended_at.is_some() {
        let cur = repo::get_shift(db, tenant, &pid)
            .await?
            .ok_or(DomainError::NotFound)?;
        if cur.ended_at.is_some() {
            return Err(DomainError::Conflict("el turno ya está cerrado".into()));
        }
        if let Some(end) = patch.ended_at {
            if end < cur.started_at {
                return Err(DomainError::Invalid(
                    "ended_at debe ser posterior a started_at".into(),
                ));
            }
        }
    }
    repo::update_shift(db, tenant, &pid, &patch).await
}

//! Prescriptions + pharmacist-shift persistence. Tenant-scoped.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};

use super::model::*;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Chrono `DateTime<Utc>` serializes as an RFC3339 *string* via serde, but the
/// SurrealQL `datetime` schema rejects strings on bind. Wrap as a native
/// `Datetime` value so the field round-trips correctly.
fn dt_val(d: DateTime<Utc>) -> surrealdb::sql::Value {
    surrealdb::sql::Datetime::from(d).into()
}

fn dt_opt(d: Option<DateTime<Utc>>) -> surrealdb::sql::Value {
    match d {
        Some(x) => dt_val(x),
        None => surrealdb::sql::Value::None,
    }
}

// --- prescription rows -----------------------------------------------------

#[derive(Debug, Deserialize)]
struct PrescriptionRow {
    id: Thing,
    product: Option<Thing>,
    customer: Option<Thing>,
    patient_name: String,
    patient_rut: String,
    doctor_name: Option<String>,
    doctor_rut: Option<String>,
    controlled: bool,
    folio: Option<String>,
    dispensed_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl From<PrescriptionRow> for PrescriptionDto {
    fn from(r: PrescriptionRow) -> Self {
        Self {
            id: r.id.to_string(),
            product: r.product.map(|p| p.to_string()),
            customer: r.customer.map(|c| c.to_string()),
            patient_name: r.patient_name,
            patient_rut: r.patient_rut,
            doctor_name: r.doctor_name,
            doctor_rut: r.doctor_rut,
            controlled: r.controlled,
            folio: r.folio,
            dispensed_at: r.dispensed_at,
            created_at: r.created_at,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn create_prescription(
    db: &Db,
    tenant: &Thing,
    product: Option<Thing>,
    customer: Option<Thing>,
    input: &NewPrescription,
) -> DomainResult<PrescriptionDto> {
    let dispensed = input.dispensed_at.unwrap_or_else(Utc::now);
    let mut r = db
        .query(
            "CREATE prescription SET tenant = $t, product = $product, customer = $customer, \
             patient_name = $patient_name, patient_rut = $patient_rut, \
             doctor_name = $doctor_name, doctor_rut = $doctor_rut, \
             controlled = $controlled, folio = $folio, dispensed_at = $dispensed_at \
             RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("product", product))
        .bind(("customer", customer))
        .bind(("patient_name", input.patient_name.clone()))
        .bind(("patient_rut", input.patient_rut.clone()))
        .bind(("doctor_name", input.doctor_name.clone()))
        .bind(("doctor_rut", input.doctor_rut.clone()))
        .bind(("controlled", input.controlled))
        .bind(("folio", input.folio.clone()))
        .bind(("dispensed_at", dt_val(dispensed)))
        .await?;
    let row: Option<PrescriptionRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn list_prescriptions(
    db: &Db,
    tenant: &Thing,
    f: &PrescriptionFilters,
) -> DomainResult<Vec<PrescriptionDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.patient_rut.is_some() {
        conds.push("patient_rut = $rut".to_string());
    }
    if f.controlled.is_some() {
        conds.push("controlled = $ctrl".to_string());
    }
    if f.from.is_some() {
        conds.push("created_at >= $from".to_string());
    }
    if f.to.is_some() {
        conds.push("created_at <= $to".to_string());
    }
    let limit = f.limit.unwrap_or(100).min(500);
    let offset = f.offset.unwrap_or(0);
    let q = format!(
        "SELECT * FROM prescription WHERE {} ORDER BY created_at DESC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset
    );
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("rut", f.patient_rut.clone().unwrap_or_default()))
        .bind(("ctrl", f.controlled.unwrap_or(false)))
        .bind(("from", dt_opt(f.from)))
        .bind(("to", dt_opt(f.to)))
        .await?;
    let rows: Vec<PrescriptionRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_prescription(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
) -> DomainResult<Option<PrescriptionDto>> {
    let mut r = db
        .query("SELECT * FROM prescription WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<PrescriptionRow> = r.take(0)?;
    Ok(row.map(Into::into))
}

pub async fn product_belongs(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("SELECT id FROM product WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

pub async fn customer_belongs(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("SELECT id FROM customer WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

// --- pharmacist shifts -----------------------------------------------------

#[derive(Debug, Deserialize)]
struct ShiftRow {
    id: Thing,
    user: Thing,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    notes: Option<String>,
}

impl From<ShiftRow> for PharmacistShiftDto {
    fn from(r: ShiftRow) -> Self {
        Self {
            id: r.id.to_string(),
            user: r.user.to_string(),
            started_at: r.started_at,
            ended_at: r.ended_at,
            notes: r.notes,
        }
    }
}

pub async fn user_belongs(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    let mut r = db
        .query("SELECT id FROM user WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<Thing> = r.take((0, "id"))?;
    Ok(row.is_some())
}

pub async fn create_shift(
    db: &Db,
    tenant: &Thing,
    user: &Thing,
    started_at: Option<DateTime<Utc>>,
    notes: Option<&str>,
) -> DomainResult<PharmacistShiftDto> {
    let started = started_at.unwrap_or_else(Utc::now);
    let mut r = db
        .query(
            "CREATE pharmacist_shift SET tenant = $t, user = $u, started_at = $started, \
             notes = $notes RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("u", user.clone()))
        .bind(("started", dt_val(started)))
        .bind(("notes", notes.map(str::to_string)))
        .await?;
    let row: Option<ShiftRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn list_shifts(
    db: &Db,
    tenant: &Thing,
    f: &ShiftFilters,
) -> DomainResult<Vec<PharmacistShiftDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.user.is_some() {
        conds.push("user = $u".to_string());
    }
    match f.open {
        Some(true) => conds.push("ended_at = NONE".to_string()),
        Some(false) => conds.push("ended_at != NONE".to_string()),
        None => {}
    }
    let limit = f.limit.unwrap_or(100).min(500);
    let offset = f.offset.unwrap_or(0);
    let q = format!(
        "SELECT * FROM pharmacist_shift WHERE {} ORDER BY started_at DESC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset
    );
    let user = f
        .user
        .as_deref()
        .and_then(|s| surrealdb::sql::thing(s).ok());
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("u", user))
        .await?;
    let rows: Vec<ShiftRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get_shift(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
) -> DomainResult<Option<PharmacistShiftDto>> {
    let mut r = db
        .query("SELECT * FROM pharmacist_shift WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await?;
    let row: Option<ShiftRow> = r.take(0)?;
    Ok(row.map(Into::into))
}

pub async fn update_shift(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
    patch: &UpdatePharmacistShift,
) -> DomainResult<PharmacistShiftDto> {
    let mut sets = Vec::new();
    if patch.ended_at.is_some() {
        sets.push("ended_at = $ended");
    }
    if patch.notes.is_some() {
        sets.push("notes = $notes");
    }
    if sets.is_empty() {
        return get_shift(db, tenant, id)
            .await?
            .ok_or(DomainError::NotFound);
    }
    let q = format!(
        "UPDATE pharmacist_shift SET {} WHERE id = $id AND tenant = $t RETURN AFTER",
        sets.join(", ")
    );
    let mut r = db
        .query(q)
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .bind(("ended", dt_opt(patch.ended_at)))
        .bind(("notes", patch.notes.clone()))
        .await?;
    let row: Option<ShiftRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

//! Prescriptions + pharmacist-shift DTOs and input types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// --- prescriptions ---------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PrescriptionDto {
    pub id: String,
    pub product: Option<String>,
    pub customer: Option<String>,
    pub patient_name: String,
    pub patient_rut: String,
    pub doctor_name: Option<String>,
    pub doctor_rut: Option<String>,
    pub controlled: bool,
    pub folio: Option<String>,
    pub dispensed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewPrescription {
    /// Optional product record id (`product:xxx`).
    pub product: Option<String>,
    /// Optional customer record id (`customer:xxx`).
    pub customer: Option<String>,
    pub patient_name: String,
    pub patient_rut: String,
    pub doctor_name: Option<String>,
    pub doctor_rut: Option<String>,
    #[serde(default)]
    pub controlled: bool,
    pub folio: Option<String>,
    pub dispensed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct PrescriptionFilters {
    pub patient_rut: Option<String>,
    pub controlled: Option<bool>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// --- pharmacist shifts -----------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PharmacistShiftDto {
    pub id: String,
    pub user: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewPharmacistShift {
    /// User record id (`user:xxx`). Must belong to this tenant.
    pub user: String,
    pub started_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct UpdatePharmacistShift {
    /// Setting this closes the shift.
    pub ended_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct ShiftFilters {
    pub user: Option<String>,
    pub open: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

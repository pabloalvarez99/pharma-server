//! Customers DTOs and input types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// --- responses -------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CustomerDto {
    pub id: String,
    pub name: String,
    pub rut: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub loyalty_points: i64,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LoyaltyTxDto {
    pub id: String,
    pub customer: String,
    pub delta: i64,
    pub reason: String,
    #[serde(rename = "ref")]
    pub ref_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Tenant-wide loyalty aggregate. Sales (Fase 4) drives the numbers; until
/// then totals are 0 and `top_customers` is empty.
#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct LoyaltyStats {
    pub members: i64,
    pub points_outstanding: i64,
    pub points_earned_30d: i64,
    pub points_redeemed_30d: i64,
    pub top_customers: Vec<LoyaltyTopCustomer>,
    /// `true` while sales-side accumulation is not wired (Fase 4).
    pub pending_sales_integration: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LoyaltyTopCustomer {
    pub id: String,
    pub name: String,
    pub points: i64,
}

// --- inputs ----------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewCustomer {
    pub name: String,
    pub rut: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct UpdateCustomer {
    pub name: Option<String>,
    pub rut: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct CustomerFilters {
    pub search: Option<String>,
    pub active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct LoyaltyFilters {
    pub customer: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// --- bulk import (Supabase migration) -------------------------------------
//
// Idempotent batch ingestion. Upsert key is `(tenant, email)` because the
// `customer` schema (migration 0005) is SCHEMAFULL and has no `external_id`
// column — accepting `external_id` in the DTO keeps the import script
// agnostic of source-system internals but it is currently ignored for
// persistence (re-runs match on email only).

/// Maximum customers per `POST /api/v1/admin/import-customers` request.
/// Each row is a small object (≤1 KB), so 200 keeps the worst-case payload
/// well under the default body limit and bounds DB roundtrips.
pub const IMPORT_CUSTOMERS_MAX_BATCH: usize = 200;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ImportCustomerInput {
    pub email: String,
    pub name: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub rut: Option<String>,
    /// Optional foreign key to the source system (e.g. Supabase user id).
    /// Accepted for forward compatibility; not persisted yet (no migration).
    #[serde(default)]
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ImportCustomersRequest {
    pub customers: Vec<ImportCustomerInput>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ImportError {
    /// 0-based index in the original request `customers` array.
    pub idx: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ImportCustomersResponse {
    pub created: u32,
    pub updated: u32,
    pub errors: Vec<ImportError>,
}

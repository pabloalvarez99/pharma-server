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

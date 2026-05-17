use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExpenseDto {
    pub id: String,
    pub category: String,
    pub description: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub payment_method: String,
    pub cash_session: Option<String>,
    pub supplier: Option<String>,
    pub note: Option<String>,
    pub created_by: Option<String>,
    pub incurred_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewExpense {
    pub category: String,
    pub description: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub amount: Decimal,
    /// `cash | bank | card | transfer`. Defaults to `cash` when omitted.
    #[serde(default = "default_payment_method")]
    pub payment_method: String,
    pub cash_session: Option<String>,
    pub supplier: Option<String>,
    pub note: Option<String>,
    pub incurred_at: Option<DateTime<Utc>>,
}

fn default_payment_method() -> String {
    "cash".into()
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct ExpenseFilters {
    pub category: Option<String>,
    pub payment_method: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DailySalesRow {
    /// Date in YYYY-MM-DD (UTC).
    pub date: String,
    pub orders: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub revenue: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub cash: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub card: Decimal,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct SalesReportFilters {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct NearExpiryFilters {
    /// Lookahead window in days (default 30). Batches expiring on or before
    /// `now + days` — including already-expired ones — are returned.
    pub days: Option<i64>,
}

/// One soon-to-expire (or already-expired) product batch with on-hand stock.
/// Sorted by `expiry_date` ascending so the most urgent lots come first.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NearExpiryRow {
    pub product_id: String,
    pub product_name: String,
    pub batch_id: String,
    pub batch_code: String,
    /// Expiry date (UTC).
    pub expiry_date: DateTime<Utc>,
    pub stock: i64,
    /// Whole days from today (UTC) until expiry. Negative = already expired.
    pub days_to_expiry: i64,
    /// True when `expiry_date <= now`.
    pub expired: bool,
}

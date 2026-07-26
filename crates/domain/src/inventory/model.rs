//! Inventory DTOs and inputs. Money serializes as JSON string
//! (`rust_decimal::serde::str`) to avoid float drift in clients.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Default low-stock threshold (mirrors `catalog::LOW_STOCK_DEFAULT`) until
/// per-tenant `setting` lands (Fase 7).
pub const LOW_STOCK_DEFAULT: i64 = 5;
/// "Próximos a vencer" window in days for [`InventorySummary`].
pub const EXPIRING_SOON_DAYS: i64 = 30;
/// ABC cumulative-percent breakpoints (A ≤ 80, B ≤ 95, C rest).
pub const ABC_PCT_A: f64 = 80.0;
pub const ABC_PCT_B: f64 = 95.0;

// --- stock_movement --------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StockMovementDto {
    pub id: String,
    pub product: String,
    pub delta: i64,
    pub reason: String,
    pub admin: Option<String>,
    pub r#ref: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct MovementFilters {
    pub product: Option<String>,
    pub reason: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewMovement {
    /// Product record id (`product:xxx`).
    pub product: String,
    /// Signed delta. Non-zero. Negative requires sufficient stock.
    pub delta: i64,
    pub reason: String,
    /// External reference (PO id, return id, etc.).
    pub r#ref: Option<String>,
}

/// Body for `POST /stock-movements/adjust` — set or delta with required reason.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AdjustMovement {
    pub product: String,
    pub set: Option<i64>,
    pub delta: Option<i64>,
    pub reason: String,
    pub r#ref: Option<String>,
}

// --- product_batch ---------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BatchDto {
    pub id: String,
    pub product: String,
    /// Sucursal donde está físicamente el lote. `None` = casa matriz
    /// (migración 0042). El FEFO de una venta sólo consume lotes de la sucursal
    /// donde se vende.
    pub branch: Option<String>,
    pub batch_code: String,
    pub expiry_date: DateTime<Utc>,
    pub stock: i64,
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub cost: Option<Decimal>,
    pub notes: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewBatch {
    pub product: String,
    /// Sucursal donde queda el lote. Ausente / `""` / `"none"` = casa matriz.
    #[serde(default)]
    pub branch: Option<String>,
    pub batch_code: String,
    pub expiry_date: DateTime<Utc>,
    /// Initial stock for the batch. If > 0 a `stock_movement` of equal delta
    /// (`reason = "batch_received"`) is emitted in the same transaction.
    #[serde(default)]
    pub stock: i64,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub cost: Option<Decimal>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct UpdateBatch {
    pub batch_code: Option<String>,
    pub expiry_date: Option<DateTime<Utc>>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub cost: Option<Decimal>,
    pub notes: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct BatchFilters {
    pub product: Option<String>,
    /// Acota a una sucursal. `"none"` = sólo casa matriz; ausente = todas
    /// (la vista de inventario del dueño quiere ver el total del negocio).
    pub branch: Option<String>,
    /// `true` (default) only batches still in stock & not expired.
    pub only_available: Option<bool>,
    /// Days to consider "expiring soon".
    pub expiring_within_days: Option<i64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// --- falta -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FaltaDto {
    pub id: String,
    pub product: Option<String>,
    pub name: String,
    pub qty: i64,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub notes: Option<String>,
    pub resolved: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewFalta {
    pub product: Option<String>,
    pub name: String,
    #[serde(default = "default_qty")]
    pub qty: i64,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub notes: Option<String>,
}

fn default_qty() -> i64 {
    1
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct UpdateFalta {
    pub name: Option<String>,
    pub qty: Option<i64>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub notes: Option<String>,
    pub resolved: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct FaltaFilters {
    pub resolved: Option<bool>,
    pub product: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// --- summary / abc / reorder ----------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct InventorySummary {
    pub total_skus: i64,
    pub active_skus: i64,
    pub low_stock: i64,
    pub out_of_stock: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub inventory_value: Decimal,
    pub expiring_soon: i64,
    pub expired: i64,
    pub open_faltas: i64,
}

/// ABC class: A = top-value (cumulative ≤ 80%), B (≤ 95%), C rest.
#[derive(Debug, Clone, Copy, Serialize, ToSchema, PartialEq, Eq)]
pub enum AbcClass {
    A,
    B,
    C,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AbcRow {
    pub product: String,
    pub name: String,
    pub stock: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub value: Decimal,
    pub cumulative_pct: f64,
    pub class: AbcClass,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AbcReport {
    /// `"value_stock_fallback"` until sales (Fase 4) lands; then `"sales_90d"`.
    pub method: &'static str,
    pub rows: Vec<AbcRow>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReorderRow {
    pub product: String,
    pub name: String,
    pub stock: i64,
    pub low_threshold: i64,
    pub suggested_qty: i64,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReorderReport {
    /// `"low_stock_stub"` until sales history exists; then
    /// `"avg_daily_sales*lead_time + safety - stock"`.
    pub method: &'static str,
    pub rows: Vec<ReorderRow>,
}

// --- FEFO planning ---------------------------------------------------------

/// One allocation slice from a [`product_batch`] for a FEFO consumption plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FefoAllocation {
    pub batch: String,
    pub qty: i64,
}

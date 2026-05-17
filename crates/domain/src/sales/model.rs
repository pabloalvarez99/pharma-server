//! Sales DTOs and inputs (Fase 4). Money serializes as JSON string
//! (`rust_decimal::serde::str`) to avoid float drift in clients.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::interactions::InteractionDetail;

/// POS payment methods accepted on counter.
pub const POS_METHODS: &[&str] = &["pos_cash", "pos_debit", "pos_credit", "pos_mixed"];

/// Default loyalty rule: 1 point per $1000 CLP (overridable via
/// `admin_setting.loyalty_points_per_clp`).
pub const LOYALTY_CLP_PER_POINT_DEFAULT: i64 = 1000;

/// Idempotency-Key TTL — 24 hours (Fase 8 cron purges).
pub const IDEMPOTENCY_TTL_HOURS: i64 = 24;

// --- order -----------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OrderDto {
    pub id: String,
    pub status: String,
    pub payment_method: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub subtotal: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub discount: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total: Decimal,
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub cash_amount: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub card_amount: Option<Decimal>,
    pub customer: Option<String>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub sold_by: Option<String>,
    pub sold_by_name: Option<String>,
    pub notes: Option<String>,
    pub external_ref: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OrderItemDto {
    pub id: String,
    pub order: String,
    pub product: Option<String>,
    pub product_name: String,
    pub quantity: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub subtotal: Decimal,
    /// Primary FEFO lot (earliest expiry consumed). Kept for backward compat;
    /// full breakdown lives in [`Self::batches`].
    pub batch: Option<String>,
    /// Multi-lot split (BACKLOG #3): every FEFO allocation consumed for the
    /// line. `None` on non-batch-tracked products and on rows persisted before
    /// migration `0013_order_item_batches`. Sum of `qty` equals `quantity`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batches: Option<Vec<OrderItemBatchAllocation>>,
}

/// One FEFO allocation persisted on an [`OrderItemDto`]. The same shape is
/// returned by `inventory::plan_fefo`, but as a DTO it carries strings (record
/// ids) rather than `Thing`s so it can cross the API boundary.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct OrderItemBatchAllocation {
    pub batch: String,
    pub qty: i64,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct OrderFilters {
    pub status: Option<String>,
    pub payment_method: Option<String>,
    pub customer: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// --- POST /pos/sale request -----------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PosSaleRequest {
    pub items: Vec<PosSaleItem>,
    pub payment_method: String,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub cash_amount: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub card_amount: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub discount: Option<Decimal>,
    pub customer: Option<String>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub notes: Option<String>,
    /// Optional external reference (e.g. SII boleta folio when issued).
    pub external_ref: Option<String>,
    #[serde(default)]
    pub prescriptions: Vec<PosPrescriptionInput>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PosSaleItem {
    /// Product record id (`product:xxx`). Required — POS does not allow free items.
    pub product: String,
    pub product_name: String,
    pub quantity: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_price: Decimal,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PosPrescriptionInput {
    pub product: Option<String>,
    pub patient_name: String,
    pub patient_rut: String,
    pub doctor_name: Option<String>,
    pub doctor_rut: Option<String>,
    pub folio: Option<String>,
    /// If omitted, computed from product.active_ingredient via
    /// [`super::controlled::is_controlled`].
    pub controlled: Option<bool>,
}

/// Response from `POST /pos/sale`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PosSaleResponse {
    pub order: OrderDto,
    pub items: Vec<OrderItemDto>,
    pub stock_movements: Vec<String>,
    pub prescriptions: Vec<String>,
    pub loyalty_points_awarded: i64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub interaction_warnings: Vec<InteractionDetail>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub low_stock_alerts: Vec<LowStockAlert>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LowStockAlert {
    pub product: String,
    pub product_name: String,
    pub stock: i64,
    pub threshold: i64,
}

// --- devolucion ------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DevolucionDto {
    pub id: String,
    pub order: Option<String>,
    pub tipo: String,
    pub motivo: String,
    pub notas: Option<String>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total_devuelto: Decimal,
    pub metodo_reembolso: Option<String>,
    pub procesado_por: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewDevolucion {
    pub order: Option<String>,
    pub tipo: String,
    pub motivo: String,
    pub notas: Option<String>,
    pub items: Vec<NewDevolucionItem>,
    pub metodo_reembolso: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewDevolucionItem {
    pub product: Option<String>,
    pub product_name: String,
    pub quantity: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_price: Decimal,
    #[serde(default = "default_restock")]
    pub restock: bool,
}

fn default_restock() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DevolucionItemDto {
    pub id: String,
    pub devolucion: String,
    pub product: Option<String>,
    pub product_name: String,
    pub quantity: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_price: Decimal,
    pub restock: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RefundResponse {
    pub devolucion: DevolucionDto,
    pub items: Vec<DevolucionItemDto>,
    pub stock_movements: Vec<String>,
    pub order_marked_refunded: bool,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct DevolucionFilters {
    pub order: Option<String>,
    pub tipo: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// --- admin_setting ---------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdminSettingDto {
    pub key: String,
    pub value: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewAdminSetting {
    pub key: String,
    pub value: String,
}

//! Purchasing DTOs and input types. Money fields serialize as JSON strings
//! (`rust_decimal::serde::str`) to avoid float drift in clients.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// --- responses -------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SupplierDto {
    pub id: String,
    pub name: String,
    pub rut: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub default_invoice_format: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SupplierProductMappingDto {
    pub id: String,
    pub supplier: String,
    pub product: String,
    pub supplier_code: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SupplierPriceDto {
    pub id: String,
    pub supplier: String,
    pub product: Option<String>,
    pub supplier_code: Option<String>,
    pub description: Option<String>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_cost: Decimal,
    pub currency: String,
    pub valid_from: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

// --- inputs ----------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewSupplier {
    pub name: String,
    pub rut: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub default_invoice_format: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct UpdateSupplier {
    pub name: Option<String>,
    pub rut: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub default_invoice_format: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct SupplierFilters {
    pub search: Option<String>,
    pub active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewSupplierPrice {
    /// Supplier record id (`supplier:xxx`).
    pub supplier: String,
    /// Optional internal product record id (`product:xxx`).
    pub product: Option<String>,
    pub supplier_code: Option<String>,
    pub description: Option<String>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_cost: Decimal,
    pub currency: Option<String>,
    pub valid_from: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct SupplierPriceFilters {
    pub supplier: Option<String>,
    pub product: Option<String>,
    pub supplier_code: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct MapSupplierProduct {
    /// Product record id (`product:xxx`).
    pub product: String,
    pub supplier_code: String,
}

// --- compare ---------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CompareItem {
    /// Internal product record id (`product:xxx`). Optional — if absent,
    /// `supplier_code` is used to locate matching price-list rows.
    pub product: Option<String>,
    pub supplier_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CompareRequest {
    pub items: Vec<CompareItem>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CompareBest {
    pub supplier: String,
    pub supplier_name: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_cost: Decimal,
    pub price_id: String,
    pub valid_from: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CompareResult {
    pub product: Option<String>,
    pub supplier_code: Option<String>,
    pub best: Option<CompareBest>,
    /// Current product `cost_price` if product is resolvable; otherwise `None`.
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub current_cost: Option<Decimal>,
    /// `current_cost − best.unit_cost`. Positive = saving available.
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub savings: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CompareResponse {
    pub items: Vec<CompareResult>,
}

// --- purchase orders (Fase 5-full, BACKLOG #8 slice 1) ---------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PurchaseOrderItemDto {
    pub id: String,
    /// Internal product record id (`product:xxx`) if catalogued, else `None`.
    pub product: Option<String>,
    pub product_name: String,
    pub quantity: i64,
    /// Cumulative quantity already received against this line (0 until a
    /// receipt posts; ≤ `quantity`). Drives partial-receipt math.
    #[serde(default)]
    pub qty_received: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_cost: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub subtotal: Decimal,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PurchaseOrderDto {
    pub id: String,
    pub supplier: String,
    pub status: String,
    pub currency: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total: Decimal,
    pub notes: Option<String>,
    pub external_ref: Option<String>,
    /// Lines. Populated by `get`; `list` returns an empty vec (header only).
    pub items: Vec<PurchaseOrderItemDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewPurchaseOrderItem {
    /// Optional internal product record id (`product:xxx`). When absent the
    /// line is free-text (`product_name` only) — supports off-catalog buys.
    pub product: Option<String>,
    pub product_name: String,
    pub quantity: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_cost: Decimal,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewPurchaseOrder {
    /// Supplier record id (`supplier:xxx`).
    pub supplier: String,
    pub currency: Option<String>,
    pub notes: Option<String>,
    pub external_ref: Option<String>,
    pub items: Vec<NewPurchaseOrderItem>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct PurchaseOrderFilters {
    pub supplier: Option<String>,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// --- receiving (recepción de mercadería) -----------------------------------

/// One line of a goods receipt against a sent/approved purchase order.
/// `po_line_id` targets the `purchase_order_item`; `qty_received` is the
/// quantity that physically arrived in *this* receipt (additive across
/// partial receipts, capped at the line's remaining-to-receive).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReceivePurchaseOrderLine {
    /// `purchase_order_item:xxx` — the line being received.
    pub po_line_id: String,
    pub qty_received: i64,
    /// Optional lot/batch code. When present (with `expiry_date`) a
    /// `product_batch` row is created/topped-up for traceability.
    pub lot: Option<String>,
    pub expiry_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReceivePurchaseOrder {
    pub lines: Vec<ReceivePurchaseOrderLine>,
    pub notes: Option<String>,
}

// --- accounts payable (Fase 5-full, BACKLOG #8 slice 3) --------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PurchasePaymentDto {
    pub id: String,
    pub purchase_order: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub currency: String,
    pub payment_method: String,
    pub cash_session: Option<String>,
    pub reference: Option<String>,
    pub note: Option<String>,
    pub paid_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewPurchasePayment {
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub currency: Option<String>,
    pub payment_method: Option<String>,
    /// `cash_register_session:xxx` — required when `payment_method='cash'`
    /// and a session is open; surfaces the payment in the drawer arqueo.
    pub cash_session: Option<String>,
    pub reference: Option<String>,
    pub note: Option<String>,
    pub paid_at: Option<DateTime<Utc>>,
}

/// Snapshot of a PO's accounts-payable state: header `total` + sum of
/// payments + remaining `balance` + `fully_paid` flag (balance ≤ 0).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PurchasePaymentSummary {
    pub purchase_order: String,
    pub status: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub paid: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub balance: Decimal,
    pub fully_paid: bool,
    pub payments: Vec<PurchasePaymentDto>,
}

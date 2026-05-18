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

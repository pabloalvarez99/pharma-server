//! Catalog DTOs and input types. API-facing; money fields serialize as JSON
//! strings (`rust_decimal::serde::str`) to avoid float drift in clients.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Default low-stock threshold until per-tenant `setting` lands (Fase 7).
pub const LOW_STOCK_DEFAULT: i64 = 5;

// --- responses -------------------------------------------------------------

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CategoryDto {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProductDto {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub price: Decimal,
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub cost_price: Option<Decimal>,
    pub stock: i64,
    /// `false` = servicio (no descuenta inventario, sin lotes). DEFAULT `true`
    /// en DB (migración 0031): todo producto físico mantiene el chequeo de stock.
    pub physical_stock: bool,
    pub category: Option<String>,
    pub image_url: Option<String>,
    pub active: bool,
    pub external_id: Option<String>,
    pub laboratory: Option<String>,
    pub therapeutic_action: Option<String>,
    pub active_ingredient: Option<String>,
    pub prescription_type: String,
    pub presentation: Option<String>,
    pub discount_percent: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProductStats {
    pub total: i64,
    pub active: i64,
    pub low_stock: i64,
    pub out_of_stock: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub inventory_value: Decimal,
    /// Always 0 until `product_batch` exists (Fase 3 — inventory).
    pub expired: i64,
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct EtiquetaResults {
    pub laboratories: Vec<String>,
    pub active_ingredients: Vec<String>,
    pub therapeutic_actions: Vec<String>,
}

// --- inputs ----------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewProduct {
    pub name: String,
    /// Optional; auto-generated tenant-unique slug if omitted.
    pub slug: Option<String>,
    pub description: Option<String>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub price: Decimal,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub cost_price: Option<Decimal>,
    #[serde(default)]
    pub stock: i64,
    /// Category record id (`category:xxx`) — validated tenant-scoped.
    pub category: Option<String>,
    pub image_url: Option<String>,
    pub external_id: Option<String>,
    pub laboratory: Option<String>,
    pub therapeutic_action: Option<String>,
    pub active_ingredient: Option<String>,
    pub prescription_type: Option<String>,
    pub presentation: Option<String>,
    pub discount_percent: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct UpdateProduct {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub price: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub cost_price: Option<Decimal>,
    pub category: Option<String>,
    pub image_url: Option<String>,
    pub active: Option<bool>,
    pub external_id: Option<String>,
    pub laboratory: Option<String>,
    pub therapeutic_action: Option<String>,
    pub active_ingredient: Option<String>,
    pub prescription_type: Option<String>,
    pub presentation: Option<String>,
    pub discount_percent: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewCategory {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct UpdateCategory {
    pub name: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub active: Option<bool>,
}

/// Manual stock adjustment. Exactly one of `set` / `delta` required.
/// Full audited movement (`stock_movement`) arrives in Fase 3 — this writes
/// `product.stock` directly.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StockAdjust {
    pub set: Option<i64>,
    pub delta: Option<i64>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BulkPriceMode {
    /// `value` is a percentage delta (e.g. 10 = +10%, -5 = -5%).
    Percent,
    /// `value` is an absolute amount added to each price.
    Amount,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct BulkPrice {
    pub mode: BulkPriceMode,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub value: Decimal,
    /// Limit to a category record id; `None` = all active products.
    pub category: Option<String>,
    /// Round resulting price to whole CLP (default true).
    #[serde(default = "default_true")]
    pub round: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct ProductFilters {
    pub search: Option<String>,
    pub category: Option<String>,
    pub active: Option<bool>,
    /// Return products with `stock <= low_stock` (defaults to threshold).
    pub low_stock: Option<i64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

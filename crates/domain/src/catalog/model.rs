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
    /// Per-rubro flexible attributes (migration 0033): keys declared by the
    /// rubro pack (`GET /api/v1/rubro-pack`), e.g. `talla`, `duracion_min`.
    /// Absent for products with no rubro extras. On variants: talla/color/sku.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attrs: Option<serde_json::Value>,
    /// Parent product when this row is a sellable variant (migration 0034,
    /// Opción A). `None` = product plano o padre de matriz de tallas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Tenant barcode from `product_barcode` when present (enriched on read /
    /// create-variant). Plain products and parents often omit it (caja en hijos).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barcode: Option<String>,
    /// Read-side sum of **active** children stock when this row is a multi-SKU
    /// parent. Never written to `product.stock` (ledger invariant + farmacia
    /// plain SKUs keep stock on the product itself).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variants_stock: Option<i64>,
    /// Count of active children (list badge without N+1). Present iff parent
    /// has ≥1 active variant — same keying as `variants_stock`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant_count: Option<i64>,
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
    /// `false` = servicio: la venta salta el chequeo de stock y el ítem queda
    /// fuera de las alertas de stock bajo y del tablero (migración 0031).
    /// Ausente (`None`) = el DEFAULT de la base (`true`), o sea un bien físico:
    /// todo cliente escrito antes de este campo sigue creando productos físicos.
    ///
    /// **Sólo se decide al crear**, y por eso no está en [`UpdateProduct`]: el
    /// stock inicial de un producto físico emite su movimiento de sucursal en el
    /// `CREATE` (migración 0041). Darlo vuelta después dejaría un `product.stock`
    /// sin el `product_branch_stock` que le corresponde, y para eso no hay
    /// reconciliación escrita. Un ítem creado del lado equivocado se borra y se
    /// crea de nuevo.
    #[serde(default)]
    pub physical_stock: Option<bool>,
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
    /// Per-rubro flexible attributes (migration 0033). Omitted on wire → `None`.
    /// Keys come from the rubro pack (e.g. `talla`, `color`, `sku`, `duracion_min`).
    #[serde(default)]
    pub attrs: Option<serde_json::Value>,
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
    /// Replace entire attrs object when `Some`. `None` = leave unchanged.
    /// To clear: send `Some(json!({}))` or explicit null object (client choice).
    #[serde(default)]
    pub attrs: Option<serde_json::Value>,
    /// Optional barcode change for this product (variant or plain SKU).
    /// - omitted / `null` → leave mapping unchanged
    /// - non-empty string → reassign tenant-unique EAN (409 if taken by another product)
    /// - empty string `""` → remove barcode mapping (frees EAN)
    #[serde(default)]
    pub barcode: Option<String>,
    /// Free Web storefront opt-in per SKU (migration 0036, ADR-0020).
    pub online_visible: Option<bool>,
    pub online_title: Option<String>,
    pub online_description: Option<String>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub online_price: Option<Decimal>,
    pub online_sort: Option<i64>,
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
    /// Cursor: id of the LAST product of the previous page. `Some("")` opens
    /// the walk (first page), `None` = classic `offset` paging.
    ///
    /// Es la forma barata de avanzar: `offset` recorre desde el principio del
    /// catálogo cada vez y se degrada mientras más se avanza, mientras que el
    /// cursor arranca el recorrido en el nombre del producto que el cliente ya
    /// tiene. Cuando viene `after`, `offset` se ignora.
    ///
    /// La primera página se pide con `Some("")` y no con `None` a propósito: el
    /// cursor necesita un orden total (`name, id`) y la página del POS no puede
    /// pagarlo, así que pedirlo es explícito. Todas las páginas del recorrido
    /// —incluida la primera— tienen que salir con el mismo orden o el corte no
    /// cierra. Ver [`crate::catalog::repo::list_products_opts`].
    ///
    /// Con `search` el cursor sigue siendo correcto pero no acelera nada: ahí
    /// el plan lo manda el índice full-text.
    pub after: Option<String>,
}

// --- public storefront (Free Web PR1, ADR-0020) ----------------------------
// Unauthenticated projection served under `/api/v1/public/{slug}/…`. Safety
// contract: NEVER add cost/margin/stock-count fields here — this JSON leaves
// the building without a JWT.

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PublicStoreDto {
    pub name: String,
    /// Tenant slug (the `{slug}` path segment).
    pub slug: String,
    /// ISO-4217 del tenant (`money.currency`; CLP si no configuró nada).
    pub currency: String,
    pub whatsapp_e164: Option<String>,
    pub address_line: Option<String>,
    pub hours_label: Option<String>,
    /// `true` in PR1 (pickup is the only fulfillment until PR3).
    pub pickup_enabled: bool,
    pub pickup_instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PublicProductDto {
    /// Record id (`product:xyz`).
    pub id: String,
    pub slug: String,
    /// `online_title ?? name`.
    pub name: String,
    /// `online_description ?? description`.
    pub description_short: Option<String>,
    /// `online_price ?? price`.
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub price_clp: Decimal,
    pub image_url: Option<String>,
    pub category_slug: Option<String>,
    /// Coarse availability from `stock` — never the integer count.
    pub availability: PublicAvailability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublicAvailability {
    InStock,
    Low,
    OutOfStock,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct PublicCatalogFilters {
    pub q: Option<String>,
    /// Category slug.
    pub category: Option<String>,
    /// Default 50, clamped to `[1, 100]`.
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PublicCatalogPage {
    pub store: PublicStoreDto,
    pub items: Vec<PublicProductDto>,
    /// `Some` when a full page was returned (there may be more rows).
    pub next_offset: Option<u32>,
}

/// Input to create a sellable variant under a parent product (migración 0034).
/// Stock and barcode live on the child; `attrs` carries talla/color/sku.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewVariant {
    /// Display name; defaults to `{parent.name} — {talla/color/sku}` when omitted.
    pub name: Option<String>,
    /// Optional; auto-generated tenant-unique slug if omitted.
    pub slug: Option<String>,
    /// Unit price; inherits parent when omitted.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub price: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub cost_price: Option<Decimal>,
    #[serde(default)]
    pub stock: i64,
    /// EAN / barcode of this SKU (tenant-unique). POS scan resolves here.
    pub barcode: Option<String>,
    /// Variant discriminators (talla, color, sku, …). Keys from rubro pack.
    #[serde(default)]
    pub attrs: Option<serde_json::Value>,
    pub external_id: Option<String>,
    pub image_url: Option<String>,
}

//! Sales DTOs and inputs (Fase 4). Money serializes as JSON string
//! (`rust_decimal::serde::str`) to avoid float drift in clients.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::interactions::InteractionDetail;

/// POS payment methods accepted on counter. `pos_fiado` = venta a cuenta
/// corriente (el cliente queda debiendo): NO mueve caja, exige `customer`, y
/// genera un cargo en el ledger de fiado (migración 0039 / `crate::credit`).
/// `pos_transferencia` = "te hago la transfer" (migración 0043): ingreso
/// electrónico, liquida exacto y **no entra al efectivo esperado del arqueo**
/// — misma ruta que `pos_debit`/`pos_credit`, porque el agregado
/// `cash_sales_running` (0030) sólo suma `pos_cash`/`pos_mixed`.
pub const POS_METHODS: &[&str] = &[
    "pos_cash",
    "pos_debit",
    "pos_credit",
    "pos_mixed",
    "pos_fiado",
    "pos_transferencia",
];

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
    /// Provenance (`pos`/`web`/`agent`); NONE on legacy counter rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pickup_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready_at: Option<DateTime<Utc>>,
    /// Sucursal donde se vendió (`branch:<key>`); NONE = casa matriz / sitio
    /// único. De acá salen los reportes de venta por local.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
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

// --- receipt / boleta (read-only POS ticket data) --------------------------

// El pie de boleta ya no es una constante. Era
// `"Gracias por su compra · Tu Farmacia"`, y Tu Farmacia es **otro producto**:
// cada negocio que imprimía una boleta repartía publicidad ajena en su propio
// papel. Ahora es por tenant, con default derivado del nombre del negocio:
// ver [`crate::settings::receipt_footer_note`].

/// One printable line on a [`ReceiptDto`]. `line_total = qty * unit_price`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReceiptItem {
    pub name: String,
    pub qty: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub line_total: Decimal,
}

/// Self-contained data a POS needs to render/print a sale ticket. Read-only
/// projection of an `order` + its `order_item`s + the tenant name + the
/// `loyalty_transaction` awarded for the sale. Money serializes as strings.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReceiptDto {
    pub order_id: String,
    /// SII boleta folio (`order.external_ref`) when issued, else the order
    /// record-id key (the local sequential-ish number).
    pub folio_or_number: String,
    pub datetime: DateTime<Utc>,
    pub tenant_name: String,
    pub items: Vec<ReceiptItem>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub subtotal: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub discount: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total: Decimal,
    pub payment_method: String,
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub cash_amount: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub card_amount: Option<Decimal>,
    /// Vuelto: `cash_amount - total` for `pos_cash`, `(cash + card) - total`
    /// for `pos_mixed` (overpayment falls on the cash side); `null` for a pure
    /// card sale (no vuelto). See `get_receipt`.
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub change: Option<Decimal>,
    pub loyalty_points_awarded: i64,
    /// Cashier display name (`order.sold_by_name`).
    pub cashier: Option<String>,
    pub footer_note: String,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct OrderFilters {
    pub status: Option<String>,
    pub payment_method: Option<String>,
    pub customer: Option<String>,
    /// Provenance channel (`pos` | `web` | `agent`). Legacy counter rows carry
    /// NONE, so filtering by `pos` does NOT return pre-0019 orders.
    pub channel: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// --- web pickup orders (Free Web PR3, ADR-0020) ------------------------------

/// Web pickup order caps: cart size and per-line quantity.
pub const WEB_ORDER_MAX_ITEMS: usize = 50;
pub const WEB_ORDER_MAX_QTY: i64 = 999;

/// Pickup-code alphabet: no 0/O/1/I/L (unambiguous over the counter/phone).
pub const PICKUP_CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// Hours a web reservation holds stock before it can be expired.
pub const WEB_ORDER_RESERVE_HOURS: i64 = 24;

/// `POST /api/v1/public/{slug}/orders/web` body. Serialize is required for
/// the idempotency body fingerprint (same canonical-bytes scheme as
/// [`PosSaleRequest`]). Distinct from the legacy push-ingest
/// `sales::web_order::WebOrderRequest` (different seam, different contract).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebPickupOrderRequest {
    pub customer: WebPickupCustomer,
    #[serde(default)]
    pub fulfillment: Option<WebPickupFulfillment>,
    pub items: Vec<WebPickupItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebPickupCustomer {
    pub name: String,
    pub phone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebPickupFulfillment {
    /// Only `pickup` is accepted in PR3 (`delivery` reserved for later).
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebPickupItem {
    /// Product record id (`product:xxx`) as exposed by the public catalog.
    pub product_id: String,
    pub qty: i64,
}

/// 201 body — PR4 (storefront scripts) and PR5 (proxy) depend on these exact
/// field names.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WebPickupOrderResponse {
    pub order_id: String,
    pub pickup_code: String,
    pub status: String,
    pub currency: String,
    /// Total CLP as decimal string (server-computed; client prices ignored).
    pub total: String,
    pub expires_at: DateTime<Utc>,
}

// --- POST /pos/sale request -----------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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
    /// Sucursal donde se vende (`branch:<key>`). Ausente = el server la deduce
    /// de la sesión de caja abierta del cajero; si tampoco hay, la venta
    /// descuenta de la casa matriz. El stock se descuenta del bucket de ESTA
    /// sucursal (migración 0041), así que una venta en el local A no puede
    /// consumir lo que está en el local B.
    #[serde(default)]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PosSaleItem {
    /// Product record id (`product:xxx`). Required — POS does not allow free items.
    pub product: String,
    pub product_name: String,
    pub quantity: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub unit_price: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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

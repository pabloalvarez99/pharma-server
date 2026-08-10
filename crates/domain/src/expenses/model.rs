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
    /// Plata devuelta ESE día (Σ `devolucion.total_devuelto`), migración 0049.
    ///
    /// Línea aparte y no restada de `revenue`/`cash` a propósito: el día que se
    /// devuelve no es el día que se vendió, y una venta parcialmente devuelta
    /// sigue siendo una venta. Neto del día = `revenue - refunds`; mostrar sólo
    /// el neto esconde los dos hechos.
    ///
    /// Se fecha por la DEVOLUCIÓN, igual que el retiro del cajón, para que los
    /// dos números cierren contra la misma plata. Fecharla por la venta
    /// original haría mutar el reporte de ayer, que es justo lo que el arqueo
    /// no puede permitirse.
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub refunds: Decimal,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct SalesReportFilters {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

/// Ingresos del período desglosados por **cómo entró la plata** (migración 0043
/// suma `pos_transferencia` al POS).
///
/// Se reporta por MONTO, que es lo que el dueño pregunta ("¿cuánto me entró por
/// transferencia?"). Una venta mixta (`pos_mixed`) reparte su `cash_amount` a
/// efectivo y su `card_amount` a tarjeta — la plata entró por dos vías y
/// atribuirla a una sola mentiría —, y por eso cuenta en el `orders` de ambos
/// buckets a los que aportó monto. La suma de `amount` sobre todos los buckets
/// es el ingreso del período: **fiado incluido**, porque una venta a cuenta es
/// ingreso devengado aunque todavía no haya plata en la mano (el reporte lo
/// separa justamente para que se vea).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SalesByMethodRow {
    /// Bucket estable para máquinas: `efectivo` | `tarjeta` | `transferencia` |
    /// `fiado` | `otro`.
    pub method: String,
    /// Etiqueta es-CL lista para pantalla ("Transferencia").
    pub label: String,
    /// Ventas que aportaron a este bucket (una mixta cuenta en los dos).
    pub orders: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub amount: Decimal,
}

/// Inventory-turnover row over the window. `turnover` = `qty_sold /
/// current_stock`. The server keeps no historical stock snapshots, so
/// *current* `product.stock` is used as the denominator proxy (documented
/// approximation, same honesty stance as `items_without_cost`). `turnover`
/// and `days_of_inventory` are `null` when current stock is ≤ 0 (can't
/// divide) — those products still surface with `qty_sold` so stockouts of
/// fast movers stay visible. `days_of_inventory` (= `window_days /
/// turnover`) is only computed when both `from` and `to` are given.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StockRotationRow {
    pub product_id: String,
    pub product_name: String,
    pub qty_sold: i64,
    pub current_stock: i64,
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub turnover: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = Option<String>)]
    pub days_of_inventory: Option<Decimal>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct TopProductsFilters {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    /// Cap on returned rows (1..=500, default 50). The ABC class is always
    /// computed over the *full* ranking before the cap is applied.
    pub limit: Option<i64>,
}

/// One product's sales ranking over the window. `revenue_pct` is the share
/// of total revenue (2dp). `abc_class` is the Pareto bucket computed on the
/// cumulative revenue of the full ranking: `A` ≤80%, `B` ≤95%, `C` rest.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TopProductRow {
    pub rank: i64,
    pub product_id: Option<String>,
    pub product_name: String,
    pub qty_sold: i64,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub revenue: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub revenue_pct: Decimal,
    pub abc_class: String,
}

/// Gross-margin rollup for one UTC date. `revenue` = sum of
/// `order_item.subtotal`; `cost` = sum of `quantity * product.cost_price`
/// over the items whose product cost is known. `items_without_cost` counts
/// line items excluded from `cost` (product unset or `cost_price` null) so
/// the margin can be read honestly.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DailyMarginRow {
    /// Date in YYYY-MM-DD (UTC).
    pub date: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub revenue: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub cost: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub margin: Decimal,
    /// `margin / revenue * 100`, rounded to 2 decimals. Zero when revenue is 0.
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub margin_pct: Decimal,
    pub items_without_cost: i64,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct NearExpiryFilters {
    /// Lookahead window in days (default 30). Batches expiring on or before
    /// `now + days` — including already-expired ones — are returned.
    pub days: Option<i64>,
    /// Sucursal donde está el lote (migración 0042). Misma gramática que
    /// `BatchFilters::branch`: ausente = todos los locales (el dueño mira el
    /// negocio entero), `"none"` = sólo casa matriz, `branch:<key>` = ese local.
    pub branch: Option<String>,
}

/// One soon-to-expire (or already-expired) product batch with on-hand stock.
/// Sorted by `expiry_date` ascending so the most urgent lots come first.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NearExpiryRow {
    pub product_id: String,
    pub product_name: String,
    pub batch_id: String,
    pub batch_code: String,
    /// Sucursal donde está FÍSICAMENTE el lote (migración 0042). `None` = casa
    /// matriz. Es el dato que hace accionable la alerta: dice a qué local hay
    /// que ir a sacar el frasco.
    pub branch: Option<String>,
    /// Nombre de la sucursal, resuelto para que la UI no tenga que cruzar.
    /// `None` en casa matriz.
    pub branch_name: Option<String>,
    /// Expiry date (UTC).
    pub expiry_date: DateTime<Utc>,
    pub stock: i64,
    /// Whole days from today (UTC) until expiry. Negative = already expired.
    pub days_to_expiry: i64,
    /// True when `expiry_date <= now`.
    pub expired: bool,
}

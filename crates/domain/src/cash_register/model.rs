use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CashSessionDto {
    pub id: String,
    pub user: String,
    pub register_name: String,
    /// Caja física de la sesión (`register:<key>`, migración 0032/0041), si el
    /// cajero eligió una. `null` = caja suelta identificada sólo por nombre.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub register: Option<String>,
    /// Sucursal donde está abierta la caja (`branch:<key>`). `null` = casa
    /// matriz / negocio de un solo local. La venta que no manda sucursal
    /// descuenta de ESTA.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub opening_cash: Decimal,
    pub opening_notes: Option<String>,
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub closing_cash_counted: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub closing_cash_expected: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub discrepancia: Option<Decimal>,
    pub closing_notes: Option<String>,
    pub opened_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct OpenSessionInput {
    #[serde(default = "default_register_name")]
    pub register_name: String,
    /// Caja física (`register:<key>`). Si viene, la sucursal de la sesión sale
    /// de `register.branch` y `register_name` se completa con su nombre.
    #[serde(default)]
    pub register: Option<String>,
    /// Sucursal explícita (`branch:<key>`) cuando no se elige una caja física.
    /// Ignorada si `register` viene (la caja manda: está físicamente en un local).
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub opening_cash: Decimal,
    pub notes: Option<String>,
}

fn default_register_name() -> String {
    "caja-1".into()
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CloseSessionInput {
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub closing_cash_counted: Decimal,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CashMovementInput {
    pub tipo: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CashMovementDto {
    pub id: String,
    pub session: String,
    pub tipo: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub amount: Decimal,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CloseSummary {
    pub session: CashSessionDto,
    /// Efectivo neto de vuelto ([`crate::invariants::cash_into_drawer`]) de las
    /// órdenes pos_cash/pos_mixed de la sesión (`cancelled` excluidas).
    /// NO es `Σ order.cash_amount`: eso es lo entregado, vuelto incluido.
    ///
    /// Es **bruto de devoluciones**, a propósito (migración 0049): lo que entró
    /// al cobrar. La plata que se devolvió salió por un
    /// `cash_movement(tipo='retiro')` y está en [`Self::movements_out`], con su
    /// propia fecha. Comparar los dos números pide mostrar las dos líneas —
    /// "ventas en efectivo" y "devoluciones" — nunca una sola restada.
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub cash_sales: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub movements_in: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub movements_out: Decimal,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct SessionFilters {
    pub status: Option<String>,
    pub user: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

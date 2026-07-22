//! Cuenta corriente / fiado DTOs (V1). Money serializes as JSON string
//! (`rust_decimal::serde::str`) like the rest of the money surface.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// One immutable ledger movement. `kind` = `cargo` (el cliente debe) o `abono`
/// (pagó). `amount` siempre positivo; el signo lo da `kind`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LedgerEntryDto {
    pub id: String,
    pub kind: String,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub amount: Decimal,
    /// Venta ligada (solo en `cargo` desde POS fiado).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Estado de cuenta de un cliente: saldo actual + total fiado + total abonado +
/// los movimientos (más recientes primero).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CustomerAccountDto {
    pub customer: String,
    /// `total_charged - total_paid`. Positivo = el cliente debe.
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub balance: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total_charged: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total_paid: Decimal,
    pub entries: Vec<LedgerEntryDto>,
}

/// Un cliente con deuda vigente, para el reporte "¿cuánto me deben?".
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DebtorRow {
    pub customer: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub balance: Decimal,
    /// Último movimiento (fiado o abono) de ese cliente — para saber si la deuda
    /// está viva o quedó dormida.
    pub last_movement: DateTime<Utc>,
}

/// Resumen de cuentas por cobrar del negocio: cuánto le deben en total y quién.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DebtorsReport {
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total_por_cobrar: Decimal,
    pub debtor_count: usize,
    /// Deudores ordenados por saldo (mayor primero).
    pub rows: Vec<DebtorRow>,
}

/// `POST /api/v1/customers/{id}/abono` body — registrar un pago del cliente
/// contra su deuda.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewAbono {
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub amount: Decimal,
    /// Caja abierta donde entra el abono en efectivo (para el arqueo). Opcional
    /// (abono por transferencia/tarjeta no toca la caja).
    #[serde(default)]
    pub cash_session: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

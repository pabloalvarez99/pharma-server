//! Libro de compras + resumen IVA (V3). Money serializes as JSON string.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Una línea del libro de compras: un documento de proveedor del período.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PurchaseBookRow {
    pub purchase_order: String,
    /// Tipo DTE del documento (33 factura afecta, 34 exenta, 61 NC, 56 ND).
    /// 33 por defecto cuando el operador no lo declaró.
    pub tipo: i32,
    /// Folio de la factura del proveedor; `null` si aún no se capturó.
    pub folio: Option<String>,
    pub supplier_name: String,
    pub supplier_rut: Option<String>,
    pub date: DateTime<Utc>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub neto: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub iva: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total: Decimal,
    /// `true` cuando neto/IVA vienen declarados en la factura; `false` cuando se
    /// derivaron del total (19% CL). El operador ve qué filas debe completar.
    pub declared: bool,
}

/// Libro de compras de un período (`YYYY-MM`) + totales.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PurchaseBook {
    pub period: String,
    pub rows: Vec<PurchaseBookRow>,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total_neto: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total_iva: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub total: Decimal,
    /// Filas cuyo neto/IVA se derivaron (falta capturar la factura real).
    pub pending_declaration: usize,
}

/// Resumen de IVA del período — la cifra que el dueño lleva al F29.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IvaSummary {
    pub period: String,
    /// IVA de las VENTAS del período (débito fiscal).
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub iva_debito: Decimal,
    /// IVA de las COMPRAS del período (crédito fiscal).
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub iva_credito: Decimal,
    /// `debito - credito`. Positivo = a pagar; negativo = remanente a favor.
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub iva_a_pagar: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub ventas_neto: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    #[schema(value_type = String)]
    pub compras_neto: Decimal,
}

/// `PATCH /api/v1/purchase-orders/{id}/factura` — capturar el documento del
/// proveedor. Todo opcional: se envía lo que el operador tenga a mano.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct InvoiceInput {
    pub folio: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub tipo: Option<i32>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub neto: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub iva: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    #[schema(value_type = String)]
    pub total: Option<Decimal>,
}

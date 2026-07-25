//! DTOs e inputs para stock por sucursal + transferencias.
//!
//! La sucursal viaja como `Option<String>` con el record-id (`branch:<key>`);
//! `None` = casa matriz / sitio único (ahí vive el stock histórico tras el
//! backfill de la migración 0041).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// --- stock por sucursal ----------------------------------------------------

/// On-hand de un producto en una sucursal. Una fila por (producto, sucursal)
/// que haya tenido movimiento alguna vez.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BranchStockDto {
    /// Producto (`product:<key>`).
    pub product: String,
    /// Nombre del producto, resuelto para que la UI no tenga que cruzar.
    pub product_name: Option<String>,
    /// Sucursal (`branch:<key>`), o `null` = casa matriz / sitio único.
    pub branch: Option<String>,
    /// Nombre de la sucursal para la UI; `null` en casa matriz.
    pub branch_name: Option<String>,
    /// On-hand en esa sucursal = `Σ stock_movement.delta` del bucket.
    pub stock: i64,
    pub updated_at: DateTime<Utc>,
}

/// Filtros de `GET /stock/branches`. Sin filtros = todo el stock por sucursal
/// del tenant.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct BranchStockFilters {
    /// Filtra por producto (`product:<key>`).
    pub product: Option<String>,
    /// Filtra por sucursal: un `branch:<key>`, o el literal `"none"` para la
    /// casa matriz.
    pub branch: Option<String>,
    /// Sólo filas con `stock != 0`. Default `false` (la UI de reposición quiere
    /// ver los ceros; el reporte de existencias normalmente no).
    #[serde(default)]
    pub non_zero: bool,
}

// --- reporte de stock por sucursal -----------------------------------------

/// Fila del reporte "stock por sucursal": un producto con su desglose por local
/// y el total, que por invariante V2 es `product.stock`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BranchStockReportRow {
    pub product: String,
    pub product_name: String,
    /// Desglose por sucursal (incluye la casa matriz como `branch = null`).
    pub by_branch: Vec<BranchStockSlice>,
    /// Suma del desglose. Igual a `product.stock` (invariante V2).
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BranchStockSlice {
    pub branch: Option<String>,
    pub branch_name: Option<String>,
    pub stock: i64,
}

// --- transferencia ---------------------------------------------------------

/// Solicitud de transferencia de stock entre dos sucursales del tenant.
///
/// `from_branch`/`to_branch`: `branch:<key>` o `null`/ausente = casa matriz.
/// Deben ser distintas. El producto debe ser un bien físico
/// (`physical_stock = true`); un servicio no tiene inventario que mover.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TransferInput {
    /// Producto a transferir (`product:<key>`).
    pub product: String,
    /// Sucursal origen (`branch:<key>`); ausente/`null` = casa matriz.
    #[serde(default)]
    pub from_branch: Option<String>,
    /// Sucursal destino (`branch:<key>`); ausente/`null` = casa matriz.
    #[serde(default)]
    pub to_branch: Option<String>,
    /// Cantidad a mover. Debe ser > 0 y ≤ stock disponible en el origen.
    pub qty: i64,
    /// Nota opcional (queda en `ref` de ambos movimientos de auditoría).
    #[serde(default)]
    pub notes: Option<String>,
}

/// Resultado de una transferencia aplicada: los saldos resultantes en ambas
/// puntas + los ids de los dos movimientos de auditoría emitidos.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TransferResult {
    pub product: String,
    pub product_name: String,
    pub from_branch: Option<String>,
    pub to_branch: Option<String>,
    pub qty: i64,
    /// Stock del producto en el origen DESPUÉS de la transferencia.
    pub from_stock: i64,
    /// Stock del producto en el destino DESPUÉS de la transferencia.
    pub to_stock: i64,
    /// `stock_movement` de salida (`reason = "transfer_out"`, delta `-qty`).
    pub movement_out: String,
    /// `stock_movement` de entrada (`reason = "transfer_in"`, delta `+qty`).
    pub movement_in: String,
}

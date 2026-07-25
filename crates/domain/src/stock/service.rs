//! Lógica de stock por sucursal + transferencias. Orquesta sobre
//! [`super::repo`] y reusa `branches::repo` (existencia de sucursal) e
//! `inventory::repo` (existencia / flag físico del producto).

use surrealdb::sql::Thing;

use crate::errors::{DomainError, DomainResult};
use crate::locks::tenant_stock_lock;

use super::model::*;
use super::repo;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

fn parse_thing(s: &str) -> DomainResult<Thing> {
    surrealdb::sql::thing(s).map_err(|_| DomainError::Invalid(format!("id inválido: {s}")))
}

/// Parsea una sucursal opcional. `None` / `""` / `"none"` = casa matriz (bucket
/// `NONE`). El literal `"none"` se acepta porque es el mismo token que usan los
/// filtros de lectura y el selector del cliente.
pub fn parse_branch(s: Option<&str>) -> DomainResult<Option<Thing>> {
    match s {
        None | Some("") | Some("none") => Ok(None),
        Some(v) => {
            let t = parse_thing(v)?;
            if t.tb != "branch" {
                return Err(DomainError::Invalid(format!(
                    "esperaba una sucursal, recibí {}",
                    t.tb
                )));
            }
            Ok(Some(t))
        }
    }
}

fn parse_admin(admin: Option<&str>) -> DomainResult<Option<Thing>> {
    match admin {
        None | Some("") => Ok(None),
        Some(s) => Ok(Some(parse_thing(s)?)),
    }
}

/// Igualdad de sucursales considerando la casa matriz (`None`).
fn same_branch(a: Option<&Thing>, b: Option<&Thing>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// Valida que una sucursal exista y sea del tenant. `None` (casa matriz) siempre
/// es válida — es el bucket implícito de todo negocio de un solo local.
pub async fn ensure_branch(
    db: &Db,
    tenant: &Thing,
    branch: Option<&Thing>,
    label: &str,
) -> DomainResult<()> {
    if let Some(b) = branch {
        if !crate::branches::repo::branch_exists(db, tenant, b).await? {
            return Err(DomainError::Invalid(format!("{label} no encontrada")));
        }
    }
    Ok(())
}

// --- reads -----------------------------------------------------------------

/// Lista el stock por sucursal del tenant (opcionalmente filtrado).
pub async fn list_branch_stock(
    db: &Db,
    tenant: &Thing,
    filters: BranchStockFilters,
) -> DomainResult<Vec<BranchStockDto>> {
    repo::list_branch_stock(db, tenant, &filters).await
}

/// Reporte de existencias por sucursal: producto → desglose por local + total.
pub async fn branch_stock_report(
    db: &Db,
    tenant: &Thing,
    filters: BranchStockFilters,
) -> DomainResult<Vec<BranchStockReportRow>> {
    repo::branch_stock_report(db, tenant, &filters).await
}

// --- transferencia ---------------------------------------------------------

/// Transfiere `qty` de un producto entre dos sucursales del tenant, atómico.
///
/// Validaciones (antes de la tx): producto del tenant y **físico**
/// (`physical_stock = true`); `qty > 0`; origen y destino distintos; ambas
/// sucursales (si no son casa matriz) existen y son del tenant; el origen tiene
/// stock suficiente.
///
/// El chequeo de suficiencia y la emisión de los movimientos corren bajo el
/// **mismo** lock por tenant que la venta y la devolución
/// ([`crate::locks::tenant_stock_lock`]). Sin eso, dos transferencias
/// concurrentes del mismo origen leerían el mismo saldo, pasarían las dos el
/// guard y sobre-girarían la sucursal; y una transferencia concurrente con una
/// venta del mismo producto reabriría el conflicto write-write de BUG-003/004.
pub async fn transfer(
    db: &Db,
    tenant: &Thing,
    admin: Option<&str>,
    input: TransferInput,
) -> DomainResult<TransferResult> {
    if input.qty <= 0 {
        return Err(DomainError::Invalid(
            "la cantidad a transferir debe ser mayor a 0".into(),
        ));
    }
    let pid = parse_thing(&input.product)?;
    if pid.tb != "product" {
        return Err(DomainError::Invalid(
            "product debe ser un id de producto".into(),
        ));
    }
    let from = parse_branch(input.from_branch.as_deref())?;
    let to = parse_branch(input.to_branch.as_deref())?;
    if same_branch(from.as_ref(), to.as_ref()) {
        return Err(DomainError::Invalid(
            "origen y destino deben ser sucursales distintas".into(),
        ));
    }

    // Producto del tenant + físico (un servicio no tiene inventario que mover).
    let product = crate::inventory::repo::product_for_movement(db, tenant, &pid)
        .await?
        .ok_or(DomainError::NotFound)?;
    if !product.physical_stock {
        return Err(DomainError::Invalid(
            "no se puede transferir un servicio: no tiene stock físico".into(),
        ));
    }

    ensure_branch(db, tenant, from.as_ref(), "la sucursal de origen").await?;
    ensure_branch(db, tenant, to.as_ref(), "la sucursal de destino").await?;

    let admin_thing = parse_admin(admin)?;

    // Sección crítica serializada por tenant: leer saldo del origen + aplicar.
    let lock = tenant_stock_lock(tenant);
    let _guard = lock.lock().await;

    let available = repo::branch_stock_qty(db, tenant, &pid, from.as_ref()).await?;
    if available < input.qty {
        return Err(DomainError::InsufficientStock);
    }

    let movements = repo::apply_transfer(
        db,
        tenant,
        &pid,
        from.as_ref(),
        to.as_ref(),
        input.qty,
        admin_thing.as_ref(),
        input.notes.as_deref(),
    )
    .await?;

    // Saldos resultantes (el evento ya los actualizó dentro de la tx).
    let from_stock = repo::branch_stock_qty(db, tenant, &pid, from.as_ref()).await?;
    let to_stock = repo::branch_stock_qty(db, tenant, &pid, to.as_ref()).await?;

    Ok(TransferResult {
        product: pid.to_string(),
        product_name: product.name,
        from_branch: from.map(|t| t.to_string()),
        to_branch: to.map(|t| t.to_string()),
        qty: input.qty,
        from_stock,
        to_stock,
        movement_out: movements.out_id,
        movement_in: movements.in_id,
    })
}

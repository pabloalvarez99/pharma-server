//! Inventory business logic. Thin orchestration over [`super::repo`].
//!
//! Two contracts other contexts depend on:
//!   * [`add_movement`] — the only sanctioned writer of `product.stock`.
//!     Catalog (`adjust_stock` retrofit), purchasing (Fase 5 `receive`) and
//!     sales (Fase 4 POS) MUST go through it.
//!   * [`plan_fefo`] — read-only batch allocation. Sales (Fase 4) calls it
//!     to plan POS-line consumption, then writes the decrements + the
//!     matching negative `stock_movement` in its own transaction.

use surrealdb::sql::Thing;

use crate::catalog::model::ProductDto;
use crate::errors::{DomainError, DomainResult};

use super::model::*;
use super::repo;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

fn parse_thing(s: &str) -> DomainResult<Thing> {
    surrealdb::sql::thing(s).map_err(|_| DomainError::Invalid(format!("id inválido: {s}")))
}

fn parse_admin(admin: Option<&str>) -> DomainResult<Option<Thing>> {
    match admin {
        None => Ok(None),
        Some("") => Ok(None),
        Some(s) => Ok(Some(parse_thing(s)?)),
    }
}

// --- stock movements -------------------------------------------------------

/// Apply a `delta` movement to `product`, materializing `product.stock` in
/// the same transaction. Validates tenant ownership, non-zero delta, and
/// non-negative resulting stock. `admin` is the JWT `sub` claim (record id).
pub async fn add_movement(
    db: &Db,
    tenant: &Thing,
    product_id: &str,
    delta: i64,
    reason: &str,
    admin: Option<&str>,
    reference: Option<String>,
) -> DomainResult<(StockMovementDto, ProductDto)> {
    if delta == 0 {
        return Err(DomainError::Invalid("delta no puede ser cero".into()));
    }
    let reason_norm = reason.trim();
    if reason_norm.is_empty() {
        return Err(DomainError::Invalid("razón requerida".into()));
    }
    let pid = parse_thing(product_id)?;
    if pid.tb != "product" {
        return Err(DomainError::Invalid(
            "product debe ser un id de producto".into(),
        ));
    }
    let current = repo::product_for_movement(db, tenant, &pid)
        .await?
        .ok_or(DomainError::NotFound)?;
    if current.stock + delta < 0 {
        return Err(DomainError::InsufficientStock);
    }
    let admin_thing = parse_admin(admin)?;
    repo::apply_movement(db, tenant, &pid, delta, reason_norm, admin_thing, reference).await
}

/// `set` or `delta` form (mirrors catalog `StockAdjust`); always emits an
/// audited movement with `reason = adj.reason`.
pub async fn adjust(
    db: &Db,
    tenant: &Thing,
    adj: AdjustMovement,
    admin: Option<&str>,
) -> DomainResult<(StockMovementDto, ProductDto)> {
    let pid = parse_thing(&adj.product)?;
    let current = repo::product_for_movement(db, tenant, &pid)
        .await?
        .ok_or(DomainError::NotFound)?;
    let delta = match (adj.set, adj.delta) {
        (Some(s), None) => s - current.stock,
        (None, Some(d)) => d,
        _ => {
            return Err(DomainError::Invalid(
                "indique exactamente uno de `set` o `delta`".into(),
            ))
        }
    };
    if delta == 0 {
        // No-op: return current row + a synthetic "no change" error?
        // Convention: reject so audit log isn't polluted with zero-deltas.
        return Err(DomainError::Invalid(
            "el ajuste no produce cambio de stock".into(),
        ));
    }
    add_movement(
        db,
        tenant,
        &adj.product,
        delta,
        &adj.reason,
        admin,
        adj.r#ref,
    )
    .await
}

pub async fn list_movements(
    db: &Db,
    tenant: &Thing,
    f: MovementFilters,
) -> DomainResult<Vec<StockMovementDto>> {
    repo::list_movements(db, tenant, &f).await
}

// --- batches ---------------------------------------------------------------

pub async fn create_batch(
    db: &Db,
    tenant: &Thing,
    input: NewBatch,
    admin: Option<&str>,
) -> DomainResult<BatchDto> {
    if input.batch_code.trim().is_empty() {
        return Err(DomainError::Invalid("batch_code requerido".into()));
    }
    if input.stock < 0 {
        return Err(DomainError::Invalid(
            "stock inicial no puede ser negativo".into(),
        ));
    }
    let pid = parse_thing(&input.product)?;
    // Ensure product belongs to this tenant.
    repo::product_for_movement(db, tenant, &pid)
        .await?
        .ok_or(DomainError::NotFound)?;
    let admin_thing = parse_admin(admin)?;
    let (batch, _updated) =
        repo::create_batch_atomic(db, tenant, &pid, &input, admin_thing).await?;
    Ok(batch)
}

pub async fn list_batches(db: &Db, tenant: &Thing, f: BatchFilters) -> DomainResult<Vec<BatchDto>> {
    repo::list_batches(db, tenant, &f).await
}

pub async fn get_batch(db: &Db, tenant: &Thing, id: &str) -> DomainResult<BatchDto> {
    let id = parse_thing(id)?;
    repo::get_batch(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)
}

pub async fn update_batch(
    db: &Db,
    tenant: &Thing,
    id: &str,
    patch: UpdateBatch,
) -> DomainResult<BatchDto> {
    let id = parse_thing(id)?;
    repo::update_batch(db, tenant, &id, &patch).await
}

pub async fn delete_batch(db: &Db, tenant: &Thing, id: &str) -> DomainResult<()> {
    let id = parse_thing(id)?;
    if repo::soft_delete_batch(db, tenant, &id).await? {
        Ok(())
    } else {
        Err(DomainError::NotFound)
    }
}

/// FEFO planner exposed for sales (Fase 4). Pure read; no writes.
pub async fn plan_fefo(
    db: &Db,
    tenant: &Thing,
    product_id: &str,
    qty: i64,
) -> DomainResult<Vec<FefoAllocation>> {
    let pid = parse_thing(product_id)?;
    repo::plan_fefo(db, tenant, &pid, qty).await
}

// --- faltas ----------------------------------------------------------------

pub async fn create_falta(db: &Db, tenant: &Thing, input: NewFalta) -> DomainResult<FaltaDto> {
    if input.name.trim().is_empty() {
        return Err(DomainError::Invalid("nombre requerido".into()));
    }
    if input.qty <= 0 {
        return Err(DomainError::Invalid("qty debe ser > 0".into()));
    }
    let product = match input.product.as_deref() {
        Some(s) if !s.is_empty() => Some(parse_thing(s)?),
        _ => None,
    };
    repo::create_falta(db, tenant, product, &input).await
}

pub async fn list_faltas(db: &Db, tenant: &Thing, f: FaltaFilters) -> DomainResult<Vec<FaltaDto>> {
    repo::list_faltas(db, tenant, &f).await
}

pub async fn update_falta(
    db: &Db,
    tenant: &Thing,
    id: &str,
    patch: UpdateFalta,
) -> DomainResult<FaltaDto> {
    let id = parse_thing(id)?;
    repo::update_falta(db, tenant, &id, &patch).await
}

// --- summary / abc / reorder ----------------------------------------------

pub async fn summary(db: &Db, tenant: &Thing) -> DomainResult<InventorySummary> {
    repo::inventory_summary(db, tenant, LOW_STOCK_DEFAULT, EXPIRING_SOON_DAYS).await
}

/// ABC classification. Until sales (Fase 4) lands the value axis is
/// `stock * cost_price` (fallback documented in the response `method`
/// field). When sales history exists this fn will switch to revenue 90d.
pub async fn abc(db: &Db, tenant: &Thing) -> DomainResult<AbcReport> {
    let raw = repo::abc_raw_rows(db, tenant).await?;
    let total: rust_decimal::Decimal = raw.iter().map(|r| r.value).sum();
    let mut cumulative = rust_decimal::Decimal::ZERO;
    let total_f = total.try_into().unwrap_or(0.0_f64);
    let mut rows = Vec::with_capacity(raw.len());
    for r in raw {
        cumulative += r.value;
        let pct = if total_f == 0.0 {
            0.0
        } else {
            let cum_f: f64 = cumulative.try_into().unwrap_or(0.0_f64);
            (cum_f / total_f) * 100.0
        };
        let class = if pct <= ABC_PCT_A {
            AbcClass::A
        } else if pct <= ABC_PCT_B {
            AbcClass::B
        } else {
            AbcClass::C
        };
        rows.push(AbcRow {
            product: r.product.to_string(),
            name: r.name,
            stock: r.stock,
            value: r.value,
            cumulative_pct: (pct * 100.0).round() / 100.0,
            class,
        });
    }
    Ok(AbcReport {
        method: "value_stock_fallback",
        rows,
    })
}

/// Reorder suggestions. Stub algorithm until sales history exists:
/// for every product with `stock <= LOW_STOCK_DEFAULT`, suggest enough
/// to reach `2 * LOW_STOCK_DEFAULT`. Documented in `method`.
pub async fn reorder_suggestions(db: &Db, tenant: &Thing) -> DomainResult<ReorderReport> {
    let low = LOW_STOCK_DEFAULT;
    let target = low * 2;
    let raw = repo::low_stock_rows(db, tenant, low).await?;
    let rows = raw
        .into_iter()
        .map(|r| ReorderRow {
            product: r.product.to_string(),
            name: r.name,
            stock: r.stock,
            low_threshold: low,
            suggested_qty: (target - r.stock).max(1),
            reason: "stock <= low_threshold",
        })
        .collect();
    Ok(ReorderReport {
        method: "low_stock_stub",
        rows,
    })
}

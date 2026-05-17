//! Sales service (Fase 4). Composes inventory/customer primitives into the
//! POS sale atomic flow + read endpoints + settings.
//!
//! FEFO batch consumption IS active for batch-tracked products: each line
//! is planned via [`crate::inventory::service::plan_fefo_optional`] and the
//! resulting `product_batch.stock` decrements run inside the same sale
//! BEGIN/COMMIT as the order/order_item/product.stock/stock_movement writes
//! ([`super::repo::apply_sale`]). Products with no active batches keep the
//! legacy `product.stock`-only path so adoption is opt-in per SKU.
//!
//! Prescription create from POS IS wired: each entry in
//! `PosSaleRequest.prescriptions` is persisted via
//! `prescriptions::service::create_prescription` after the sale tx commits.
//! `controlled` defaults to autodetection from `product.active_ingredient`
//! via [`super::controlled::is_controlled`] when the POS does not send the
//! flag explicitly. Failures of individual prescription creates surface as
//! `DomainError` so the caller sees them (the sale already committed; a
//! human can re-issue the receta from the order).
//!
//! Drug-interaction warnings are wired: every cart's `product.active_ingredient`
//! is tokenized against the full Beers + Vademécum CL ruleset
//! (`super::interactions::check`) and the matched pairs (severity-sorted)
//! surface in `PosSaleResponse.interaction_warnings`. Sale is never blocked —
//! pharmacist's call.
//!
//! Loyalty award IS active: if `customer` is set on the sale, points are
//! awarded via `repo::award_loyalty` (atomic tx — append `loyalty_transaction`
//! + bump `customer.loyalty_points`). Conversion rate honors
//!   `admin_setting.loyalty_points_per_clp` if set, else
//!   [`LOYALTY_CLP_PER_POINT_DEFAULT`].
//!
//! Public stable signatures so api router compiles today.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use surrealdb::sql::{thing, Thing};

use crate::errors::{DomainError, DomainResult};

use super::model::*;
use super::repo;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

fn parse_tenant_thing(s: &str, table: &str) -> DomainResult<Thing> {
    let t = thing(s).map_err(|_| DomainError::Invalid(format!("{table} id inválido: {s}")))?;
    if t.tb != table {
        return Err(DomainError::Invalid(format!(
            "esperaba {table}, recibí {}:{}",
            t.tb, t.id
        )));
    }
    Ok(t)
}

/// Validate + persist a POS sale atomically.
///
/// Caller threads `tenant` from JWT, `sold_by` from `claims.sub`, and the
/// `Idempotency-Key` header (optional). If the key is present and previously
/// resolved within the TTL, returns [`DomainError::IdempotencyReplay`] with
/// the cached payload (api crate maps it to the original response).
///
/// Returns the persisted [`PosSaleResponse`].
pub async fn post_sale(
    db: &Db,
    tenant: &Thing,
    sold_by: Option<&Thing>,
    sold_by_name: Option<&str>,
    idempotency_key: Option<&str>,
    req: PosSaleRequest,
) -> DomainResult<PosSaleResponse> {
    if req.items.is_empty() {
        return Err(DomainError::Invalid("items requeridos".into()));
    }
    if !POS_METHODS.contains(&req.payment_method.as_str()) {
        return Err(DomainError::Invalid(format!(
            "método de pago inválido: {}",
            req.payment_method
        )));
    }
    for it in &req.items {
        if it.quantity <= 0 {
            return Err(DomainError::Invalid(format!(
                "cantidad inválida para {}: {}",
                it.product_name, it.quantity
            )));
        }
        if it.unit_price.is_sign_negative() {
            return Err(DomainError::Invalid(format!(
                "precio negativo para {}",
                it.product_name
            )));
        }
    }

    // Idempotency replay: short-circuit if cached.
    if let Some(key) = idempotency_key {
        if let Some((cached, _status)) = repo::lookup_idempotency(db, tenant, key).await? {
            // The api crate decodes `cached` JSON back into PosSaleResponse
            // and returns it verbatim. We surface via a sentinel error so
            // the handler controls the response status.
            return Err(DomainError::Conflict(format!(
                "IDEMPOTENCY_CACHED:{cached}"
            )));
        }
    }

    // Stock pre-check (single SELECT IN $ids).
    let product_things: Vec<Thing> = req
        .items
        .iter()
        .map(|i| parse_tenant_thing(&i.product, "product"))
        .collect::<DomainResult<Vec<_>>>()?;
    let loaded = repo::load_products_for_sale(db, tenant, &product_things).await?;
    if loaded.len() != req.items.len() {
        return Err(DomainError::NotFound);
    }
    for (req_item, prod) in req.items.iter().zip(loaded.iter()) {
        if prod.stock < req_item.quantity {
            return Err(DomainError::InsufficientStock);
        }
    }

    // Money totals
    let subtotal: Decimal = req
        .items
        .iter()
        .map(|i| i.unit_price * Decimal::from(i.quantity))
        .sum();
    let discount_in = req.discount.unwrap_or_default();
    let discount = if discount_in > subtotal {
        subtotal
    } else if discount_in.is_sign_negative() {
        Decimal::ZERO
    } else {
        discount_in
    };
    let total = subtotal - discount;

    // Mixed payment cross-check
    if req.payment_method == "pos_mixed" {
        let cash = req.cash_amount.unwrap_or_default();
        let card = req.card_amount.unwrap_or_default();
        if cash + card < total {
            return Err(DomainError::Invalid(
                "monto efectivo + tarjeta < total en pago mixto".into(),
            ));
        }
    }

    let customer = req
        .customer
        .as_deref()
        .map(|s| parse_tenant_thing(s, "customer"))
        .transpose()?;

    // FEFO plan per line. `None` = product not batch-tracked (legacy
    // product.stock-only path); `Some(plan)` = batch-tracked, lots consumed
    // earliest-expiry-first inside the sale tx; `Err(InsufficientStock)` =
    // tracked but non-expired lots can't cover the line.
    let mut fefo_plans: Vec<Option<Vec<crate::inventory::model::FefoAllocation>>> =
        Vec::with_capacity(req.items.len());
    for it in &req.items {
        fefo_plans.push(
            crate::inventory::service::plan_fefo_optional(db, tenant, &it.product, it.quantity)
                .await?,
        );
    }

    let applied = repo::apply_sale(
        db,
        tenant,
        sold_by,
        sold_by_name,
        customer.as_ref(),
        &req,
        &fefo_plans,
        subtotal,
        discount,
        total,
    )
    .await?;

    // Loyalty: if customer set, award points based on total + setting.
    let mut loyalty_awarded = 0_i64;
    if let Some(c) = customer.as_ref() {
        let clp_per_point = resolve_loyalty_rate(db, tenant).await?;
        let points_dec = (total / Decimal::from(clp_per_point)).trunc();
        let points_i = points_dec.to_i64().unwrap_or(0).max(0);
        if points_i > 0 {
            repo::award_loyalty(db, tenant, c, points_i, "sale", &applied.order.id).await?;
            loyalty_awarded = points_i;
        }
    }

    // Prescriptions (Fase 4+: link receta a la venta). One row per input.
    // Controlled flag autodetected from product.active_ingredient when the
    // POS leaves it unset.
    let mut prescription_ids = Vec::with_capacity(req.prescriptions.len());
    for p in &req.prescriptions {
        let controlled = match p.controlled {
            Some(c) => c,
            None => detect_controlled(db, tenant, p.product.as_deref()).await?,
        };
        let new = crate::prescriptions::model::NewPrescription {
            product: p.product.clone(),
            customer: req.customer.clone(),
            patient_name: p.patient_name.clone(),
            patient_rut: p.patient_rut.clone(),
            doctor_name: p.doctor_name.clone(),
            doctor_rut: p.doctor_rut.clone(),
            controlled,
            folio: p.folio.clone(),
            dispensed_at: None,
        };
        let dto = crate::prescriptions::service::create_prescription(db, tenant, new).await?;
        prescription_ids.push(dto.id);
    }

    let resp = PosSaleResponse {
        order: applied.order,
        items: applied.items,
        stock_movements: applied.movement_ids,
        prescriptions: prescription_ids,
        loyalty_points_awarded: loyalty_awarded,
        interaction_warnings: super::interactions::check(
            &load_active_ingredients(db, tenant, &product_things).await?,
        ),
        low_stock_alerts: Vec::new(),
    };

    // Cache idempotent response.
    if let Some(key) = idempotency_key {
        let json = serde_json::to_string(&resp).map_err(|e| DomainError::Other(e.into()))?;
        repo::store_idempotency(db, tenant, key, &json, 200).await?;
    }

    Ok(resp)
}

/// Resolve loyalty conversion rate (CLP per point) — read tenant setting or
/// fall back to [`LOYALTY_CLP_PER_POINT_DEFAULT`].
/// Look up `product.active_ingredient` and check against the Decreto 404 set
/// (`super::controlled::is_controlled`). Returns `false` when no product id
/// was provided or the product row carries no active ingredient.
async fn detect_controlled(db: &Db, tenant: &Thing, product: Option<&str>) -> DomainResult<bool> {
    let Some(pid_s) = product else {
        return Ok(false);
    };
    if pid_s.is_empty() {
        return Ok(false);
    }
    let pid = parse_tenant_thing(pid_s, "product")?;
    #[derive(serde::Deserialize)]
    struct R {
        active_ingredient: Option<String>,
    }
    let mut r = db
        .query(
            "SELECT active_ingredient FROM product \
             WHERE id = $p AND tenant = $t LIMIT 1",
        )
        .bind(("p", pid))
        .bind(("t", tenant.clone()))
        .await?
        .check()?;
    let row: Option<R> = r.take(0)?;
    Ok(row
        .and_then(|r| r.active_ingredient)
        .map(|ai| super::controlled::is_controlled(Some(&ai)))
        .unwrap_or(false))
}

/// Load `active_ingredient` for the cart's products (tenant-scoped). Missing
/// rows or null fields are dropped silently — the interaction check tokenizes
/// whatever it gets and ignores unknown drugs.
pub async fn load_active_ingredients(
    db: &Db,
    tenant: &Thing,
    products: &[Thing],
) -> DomainResult<Vec<String>> {
    if products.is_empty() {
        return Ok(Vec::new());
    }
    #[derive(serde::Deserialize)]
    struct R {
        active_ingredient: Option<String>,
    }
    let rows: Vec<R> = db
        .query(
            "SELECT active_ingredient FROM product \
             WHERE tenant = $t AND id IN $ids",
        )
        .bind(("t", tenant.clone()))
        .bind(("ids", products.to_vec()))
        .await?
        .check()?
        .take(0)?;
    Ok(rows
        .into_iter()
        .filter_map(|r| r.active_ingredient)
        .collect())
}

async fn resolve_loyalty_rate(db: &Db, tenant: &Thing) -> DomainResult<i64> {
    let s = repo::get_setting(db, tenant, "loyalty_points_per_clp").await?;
    let rate = s
        .and_then(|s| s.value.parse::<i64>().ok())
        .filter(|r| *r > 0)
        .unwrap_or(LOYALTY_CLP_PER_POINT_DEFAULT);
    Ok(rate)
}

/// Cron-driven: drop `idempotency_key` rows past their TTL. Tenant-wide.
pub async fn purge_expired_idempotency(db: &Db) -> DomainResult<u64> {
    repo::purge_expired_idempotency(db).await
}

pub async fn list_orders(db: &Db, tenant: &Thing, f: OrderFilters) -> DomainResult<Vec<OrderDto>> {
    repo::list_orders(db, tenant, &f).await
}

pub async fn get_order(
    db: &Db,
    tenant: &Thing,
    id: &str,
) -> DomainResult<(OrderDto, Vec<OrderItemDto>)> {
    let id = parse_tenant_thing(id, "order")?;
    repo::get_order(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)
}

/// Validate + persist a refund/return atomically.
///
/// Rules enforced before the tx:
/// * at least one line, every line `quantity > 0`, `unit_price >= 0`;
/// * a line with `restock = true` MUST carry a `product` (you can't restock
///   an unidentified item — stock movements require a product);
/// * if `order` is set it must exist for this tenant, and per-product refunded
///   quantity may not exceed what that order actually sold (no over-refund).
pub async fn create_refund(
    db: &Db,
    tenant: &Thing,
    processed_by: Option<&Thing>,
    req: NewDevolucion,
) -> DomainResult<RefundResponse> {
    if req.items.is_empty() {
        return Err(DomainError::Invalid("items requeridos".into()));
    }
    if req.motivo.trim().is_empty() {
        return Err(DomainError::Invalid("motivo requerido".into()));
    }
    for it in &req.items {
        if it.quantity <= 0 {
            return Err(DomainError::Invalid(format!(
                "cantidad inválida para {}: {}",
                it.product_name, it.quantity
            )));
        }
        if it.unit_price.is_sign_negative() {
            return Err(DomainError::Invalid(format!(
                "precio negativo para {}",
                it.product_name
            )));
        }
        if it.restock && it.product.is_none() {
            return Err(DomainError::Invalid(format!(
                "restock requiere product en la línea '{}'",
                it.product_name
            )));
        }
    }

    let order_thing = req
        .order
        .as_deref()
        .map(|s| parse_tenant_thing(s, "order"))
        .transpose()?;

    if let Some(ord) = order_thing.as_ref() {
        let (_order, sold_items) = repo::get_order(db, tenant, ord)
            .await?
            .ok_or(DomainError::NotFound)?;
        let mut sold: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for si in &sold_items {
            if let Some(p) = &si.product {
                *sold.entry(p.clone()).or_default() += si.quantity;
            }
        }
        let mut refunding: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        for it in &req.items {
            if let Some(p) = &it.product {
                let acc = refunding.entry(p.clone()).or_default();
                *acc += it.quantity;
                let sold_qty = sold.get(p).copied().unwrap_or(0);
                if sold_qty == 0 {
                    return Err(DomainError::Invalid(format!(
                        "producto {p} no estaba en la orden"
                    )));
                }
                if *acc > sold_qty {
                    return Err(DomainError::Invalid(format!(
                        "devolución de {p} excede lo vendido ({acc} > {sold_qty})"
                    )));
                }
            }
        }
    }

    let total: Decimal = req
        .items
        .iter()
        .map(|i| i.unit_price * Decimal::from(i.quantity))
        .sum();

    let applied =
        repo::apply_refund(db, tenant, processed_by, order_thing.as_ref(), &req, total).await?;

    Ok(RefundResponse {
        devolucion: applied.devolucion,
        items: applied.items,
        stock_movements: applied.movement_ids,
        order_marked_refunded: applied.order_marked_refunded,
    })
}

pub async fn list_refunds(
    db: &Db,
    tenant: &Thing,
    f: DevolucionFilters,
) -> DomainResult<Vec<DevolucionDto>> {
    repo::list_devoluciones(db, tenant, &f).await
}

pub async fn get_setting(
    db: &Db,
    tenant: &Thing,
    key: &str,
) -> DomainResult<Option<AdminSettingDto>> {
    repo::get_setting(db, tenant, key).await
}

pub async fn set_setting(
    db: &Db,
    tenant: &Thing,
    key: &str,
    value: &str,
) -> DomainResult<AdminSettingDto> {
    if key.is_empty() {
        return Err(DomainError::Invalid("key vacío".into()));
    }
    repo::upsert_setting(db, tenant, key, value).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_payment_method() {
        // Smoke: validate the input check before any DB call.
        assert!(!POS_METHODS.contains(&"crypto"));
    }
}

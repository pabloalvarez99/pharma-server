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

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use rust_decimal::Decimal;
use surrealdb::sql::{thing, Thing};
use tokio::sync::Mutex as AsyncMutex;

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

/// Per-tenant serialization of the POS sale critical section (stock pre-check
/// + FEFO planning + [`repo::apply_sale`]).
///
/// SurrealKv's MVCC aborts a losing concurrent write-write transaction at
/// COMMIT with a *retryable* conflict and — worse — leaks partial writes from
/// the aborted multi-statement tx, corrupting the cached `product.stock`
/// counter (BUG-003 / BUG-004: 60 parallel sales → ~59 fail with DB_ERROR and
/// `product.stock` reads 199 after 2 commits from an initial 100). Serializing
/// sales per tenant removes the sale-vs-sale conflict entirely — the same
/// proven approach as `crates/dte/src/caf.rs::ASSIGN_LOCK` for folio
/// assignment. Different tenants never share a lock, so multi-tenant
/// throughput is unaffected; within a tenant the POS demand (a few cashiers)
/// is orders of magnitude below the serialized ceiling.
static SALE_LOCKS: OnceLock<std::sync::Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> =
    OnceLock::new();

fn tenant_sale_lock(tenant: &Thing) -> Arc<AsyncMutex<()>> {
    let locks = SALE_LOCKS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut guard = locks.lock().expect("SALE_LOCKS mutex poisoned");
    guard
        .entry(tenant.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

/// Max commit retries on a retryable MVCC conflict before surfacing the DB
/// error. Sale-vs-sale never conflicts (serialized above), so this only ever
/// fires against a concurrent writer on the same product (e.g. a refund); the
/// generous cap mirrors `caf.rs` and is effectively free (µs-scale backoff).
const MAX_SALE_COMMIT_RETRIES: usize = 256;

/// A losing SurrealKv transaction aborts at COMMIT with a retryable
/// write-write conflict. Mirrors `crates/dte/src/caf.rs::is_mvcc_conflict_str`.
fn is_retryable_conflict(e: &DomainError) -> bool {
    e.is_retryable_db_conflict()
}

/// Short linear backoff (max ~5ms). SurrealKv's conflict window is µs-scale;
/// longer backoff only wastes POS latency.
async fn conflict_backoff(attempt: usize) {
    let micros = std::cmp::min((attempt as u64 + 1) * 50, 5_000);
    tokio::time::sleep(std::time::Duration::from_micros(micros)).await;
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

    // Idempotency replay: short-circuit on (tenant, key) match. The body
    // fingerprint guards against BUG-002 — same key with a *different* body is
    // a 409 reuse-conflict, NOT a silent replay. Computed once here and reused
    // on `store_idempotency` below so the persisted row matches the request
    // bit-for-bit.
    let body_fp = if idempotency_key.is_some() {
        Some(repo::body_fingerprint(&req)?)
    } else {
        None
    };
    if let (Some(key), Some(fp)) = (idempotency_key, body_fp.as_deref()) {
        match repo::lookup_idempotency(db, tenant, key, fp).await? {
            repo::IdempotencyHit::Replay { response_json, .. } => {
                // The api crate decodes `response_json` back into PosSaleResponse
                // and returns it verbatim. We surface via a sentinel error so
                // the handler controls the response status.
                return Err(DomainError::Conflict(format!(
                    "IDEMPOTENCY_CACHED:{response_json}"
                )));
            }
            repo::IdempotencyHit::Conflict => {
                // BUG-002: caller reused the same key with a different body —
                // canonical "Idempotency-Key" semantics (RFC draft + Stripe).
                // The api crate maps `DomainError::Conflict` → 409 + code
                // `CONFLICT`.
                return Err(DomainError::Conflict(
                    "IDEMPOTENCY_KEY_REUSE_CONFLICT: la misma Idempotency-Key \
                     se reutilizó con un body distinto"
                        .to_string(),
                ));
            }
            repo::IdempotencyHit::None => {}
        }
    }

    // Product ids parsed once (pure). The stock pre-check itself runs inside
    // the serialized retry loop below so the check→commit window is atomic
    // w.r.t. concurrent sales (BUG-003/004).
    let product_things: Vec<Thing> = req
        .items
        .iter()
        .map(|i| parse_tenant_thing(&i.product, "product"))
        .collect::<DomainResult<Vec<_>>>()?;

    // Money totals — canonical formulas in `crate::invariants` (property-tested).
    let subtotal =
        crate::invariants::order_subtotal(req.items.iter().map(|i| (i.unit_price, i.quantity)));
    let discount_in = req.discount.unwrap_or_default();
    let discount = crate::invariants::clamp_discount(discount_in, subtotal);
    let total = crate::invariants::order_total(subtotal, discount);

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

    // === Serialized, retry-on-conflict critical section (BUG-003/004) ===
    // The per-tenant lock removes the sale-vs-sale SurrealKv write-write
    // conflict (and the partial-write corruption of `product.stock` it
    // causes). The pre-check, FEFO planning and the apply tx all live INSIDE
    // the loop so every attempt re-reads current stock and re-plans lots — a
    // retry happens only on a residual conflict with another writer (e.g. a
    // concurrent refund) on the same product, where the prior plan is stale.
    // The lock is dropped as soon as the tx commits; loyalty / prescriptions /
    // idempotency-store below run unlocked.
    let applied = {
        let sale_lock = tenant_sale_lock(tenant);
        let _guard = sale_lock.lock().await;
        let mut attempt = 0usize;
        loop {
            // Stock pre-check. `id IN $ids` doesn't preserve request order in
            // SurrealKv, so index by id and look each line up rather than zip
            // (clippy `mutable_key_type` rejects `Thing` as a key → string
            // keys). Re-read each attempt: a losing retry sees fresh stock.
            let loaded = repo::load_products_for_sale(db, tenant, &product_things).await?;
            if loaded.len() != req.items.len() {
                return Err(DomainError::NotFound);
            }
            // (stock, physical_stock) per product id. A service line
            // (`physical_stock = false`) skips the stock pre-check entirely — a
            // haircut never runs out of inventory.
            let by_id: HashMap<String, (i64, bool)> = loaded
                .iter()
                .map(|p| (p.id.to_string(), (p.stock, p.physical_stock)))
                .collect();
            let mut physical: Vec<bool> = Vec::with_capacity(req.items.len());
            for (req_item, pthing) in req.items.iter().zip(product_things.iter()) {
                let (stock, is_physical) = by_id
                    .get(&pthing.to_string())
                    .copied()
                    .ok_or(DomainError::NotFound)?;
                // Retail multi-SKU: the parent is not sellable when it has
                // active children — POS must scan the variant barcode / id
                // so stock decrements at variant level (migración 0034).
                // Stable ES contract for client matchers (`tiene variantes`,
                // `escanee el código`) — code remains INVALID_INPUT.
                if crate::catalog::repo::has_active_variants(db, tenant, pthing).await? {
                    return Err(DomainError::Invalid(format!(
                        "el producto '{}' tiene variantes; venda por talla/SKU o \
                         escanee el código de barras de la variante",
                        req_item.product_name
                    )));
                }
                if is_physical && stock < req_item.quantity {
                    return Err(DomainError::InsufficientStock);
                }
                physical.push(is_physical);
            }

            // FEFO plan per line. `None` = product not batch-tracked (legacy
            // product.stock-only path) OR a service (no inventory at all);
            // `Some(plan)` = batch-tracked, lots consumed earliest-expiry-first
            // inside the sale tx; `Err(InsufficientStock)` = tracked but
            // non-expired lots can't cover the line. Services skip FEFO planning
            // outright — they have no batches to consume.
            let mut fefo_plans: Vec<Option<Vec<crate::inventory::model::FefoAllocation>>> =
                Vec::with_capacity(req.items.len());
            for (it, is_physical) in req.items.iter().zip(physical.iter()) {
                if *is_physical {
                    fefo_plans.push(
                        crate::inventory::service::plan_fefo_optional(
                            db,
                            tenant,
                            &it.product,
                            it.quantity,
                        )
                        .await?,
                    );
                } else {
                    fefo_plans.push(None);
                }
            }

            match repo::apply_sale(
                db,
                tenant,
                sold_by,
                sold_by_name,
                customer.as_ref(),
                &req,
                &fefo_plans,
                &physical,
                subtotal,
                discount,
                total,
            )
            .await
            {
                Ok(applied) => break applied,
                Err(e) if attempt < MAX_SALE_COMMIT_RETRIES && is_retryable_conflict(&e) => {
                    attempt += 1;
                    conflict_backoff(attempt).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    };

    // Loyalty: if customer set, award points based on total + setting.
    let mut loyalty_awarded = 0_i64;
    if let Some(c) = customer.as_ref() {
        let clp_per_point = resolve_loyalty_rate(db, tenant).await?;
        let points_i = crate::invariants::loyalty_points(total, clp_per_point).max(0);
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

    // Cache idempotent response. `body_fp` was computed above (same struct =
    // same bytes) so the persisted row matches the request bit-for-bit, which
    // is what `lookup_idempotency` compares on replay.
    if let (Some(key), Some(fp)) = (idempotency_key, body_fp.as_deref()) {
        let json = serde_json::to_string(&resp).map_err(|e| DomainError::Other(e.into()))?;
        repo::store_idempotency(db, tenant, key, fp, &json, 200).await?;
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
            // Direct record-id fetch (`FROM $ids`) = O(cart); `FROM product
            // WHERE id IN $ids` full-scans the table per sale (BUG-perf-002).
            "SELECT active_ingredient FROM $ids \
             WHERE tenant = $t",
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

/// Build the printable receipt/boleta for an order. Read-only: composes the
/// order + its items + the tenant name + the loyalty points awarded into a
/// self-contained [`ReceiptDto`]. 404 (`DomainError::NotFound`) if the order
/// is missing or belongs to another tenant.
///
/// `change` = `cash_amount - total` for `pos_cash`, `(cash + card) - total` for
/// `pos_mixed`, else `None` (pure card). `line_total` = `qty * unit_price`.
pub async fn get_receipt(db: &Db, tenant: &Thing, id: &str) -> DomainResult<ReceiptDto> {
    let order_thing = parse_tenant_thing(id, "order")?;
    let (order, items) = repo::get_order(db, tenant, &order_thing)
        .await?
        .ok_or(DomainError::NotFound)?;

    let tenant_name = repo::tenant_name(db, tenant).await?.unwrap_or_default();
    let loyalty_points_awarded = repo::loyalty_awarded_for_order(db, tenant, &order_thing).await?;

    let receipt_items: Vec<ReceiptItem> = items
        .iter()
        .map(|it| ReceiptItem {
            name: it.product_name.clone(),
            qty: it.quantity,
            unit_price: it.unit_price,
            line_total: crate::invariants::line_total(it.unit_price, it.quantity),
        })
        .collect();

    // Vuelto: the amount tendered over the total. A pure-cash sale's vuelto is
    // `cash − total`; a MIXED sale's overpayment always falls on the cash side
    // (a card is never over-charged), so its vuelto is `(cash + card) − total`
    // — F-paul-pay-001: the cashier must see the vuelto on a mixed sale too, not
    // just on pos_cash. A pure-card sale settles exactly, so it has no vuelto.
    let change = match order.payment_method.as_str() {
        "pos_cash" => order
            .cash_amount
            .map(|cash| crate::invariants::cash_change(cash, order.total)),
        "pos_mixed" => {
            let tendered =
                order.cash_amount.unwrap_or_default() + order.card_amount.unwrap_or_default();
            Some(crate::invariants::cash_change(tendered, order.total))
        }
        _ => None,
    };

    // Prefer the SII folio when the boleta was issued; otherwise the local
    // record-id key is the human-facing ticket number.
    let folio_or_number = order
        .external_ref
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| order_thing.id.to_raw());

    Ok(ReceiptDto {
        order_id: order.id,
        folio_or_number,
        datetime: order.created_at,
        tenant_name,
        items: receipt_items,
        subtotal: order.subtotal,
        discount: order.discount,
        total: order.total,
        payment_method: order.payment_method,
        cash_amount: order.cash_amount,
        card_amount: order.card_amount,
        change,
        loyalty_points_awarded,
        cashier: order.sold_by_name,
        footer_note: RECEIPT_FOOTER_NOTE.to_string(),
    })
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

    // Per-line batch-restock plan, aligned with `req.items`. Filled below from
    // the originating order's consumed FEFO allocations so restock keeps
    // `product.stock == Σ product_batch.stock` (BUG-007). `None` = no batch
    // attribution (no order, line not restocked, or product not batch-tracked)
    // → `product.stock` bump only.
    let mut restock_plans: Vec<Option<Vec<crate::inventory::model::FefoAllocation>>> =
        vec![None; req.items.len()];

    // === Serialized, per-tenant critical section (refund integrity) ===
    // The cumulative over-refund guard below is a check-then-act TOCTOU: it
    // reads what was already refunded for this order (`sum_prior_refunds`) and
    // plans the restock into the remaining lot capacity, THEN `apply_refund`
    // writes. Two concurrent refunds of the same order both read the same
    // `prior`, both pass `refund_exceeds_sold`, and both COMMIT → cumulative
    // refund exceeds the sold qty (refund-fraud vector, BUG-005) and the FEFO
    // restock double-fills the same lots, breaking
    // `product.stock == Σ product_batch.stock`. Holding the SAME per-tenant lock
    // as `post_sale` (SALE_LOCKS) serializes refund-vs-refund (the guard now
    // holds) AND refund-vs-sale, so the `product.stock` UPDATE never races a
    // concurrent writer — no losing MVCC abort to surface as a 5xx. Dropped as
    // soon as `apply_refund` commits.
    let sale_lock = tenant_sale_lock(tenant);
    let _refund_guard = sale_lock.lock().await;

    if let Some(ord) = order_thing.as_ref() {
        let (_order, sold_items) = repo::get_order(db, tenant, ord)
            .await?
            .ok_or(DomainError::NotFound)?;
        let mut sold: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        // Consumed FEFO lots per product (earliest-expiry first, as recorded by
        // the sale), used to plan where returned units go back.
        let mut consumed: std::collections::HashMap<String, Vec<(String, i64)>> =
            std::collections::HashMap::new();
        for si in &sold_items {
            if let Some(p) = &si.product {
                *sold.entry(p.clone()).or_default() += si.quantity;
                if let Some(allocs) = &si.batches {
                    let lots = consumed.entry(p.clone()).or_default();
                    for a in allocs {
                        lots.push((a.batch.clone(), a.qty));
                    }
                }
            }
        }
        // Cumulative over-refund guard: count what was ALREADY refunded for
        // this order in prior `devolucion`s, not just the lines in this
        // request. Otherwise N sequential partial refunds can each pass the
        // within-request check yet sum past the sold qty (refund-fraud vector,
        // BUG-005). The running tally seeds from prior refunds and adds the
        // current request line-by-line.
        let prior = repo::sum_prior_refunds_by_product(db, tenant, ord).await?;
        // Reserve the lot capacity already consumed by prior refunds (FEFO
        // order) so the current restock fills only what is still outstanding —
        // keeps the batch sum exact across multiple sequential refunds.
        let mut lot_used: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for (p, mut already) in prior.clone() {
            if let Some(lots) = consumed.get(&p) {
                for (batch, cap) in lots {
                    if already == 0 {
                        break;
                    }
                    let take = already.min(*cap);
                    *lot_used.entry(batch.clone()).or_default() += take;
                    already -= take;
                }
            }
        }
        let mut refunding: std::collections::HashMap<String, i64> = prior;
        for (i, it) in req.items.iter().enumerate() {
            if let Some(p) = &it.product {
                let acc = refunding.entry(p.clone()).or_default();
                *acc += it.quantity;
                let sold_qty = sold.get(p).copied().unwrap_or(0);
                if sold_qty == 0 {
                    return Err(DomainError::Invalid(format!(
                        "producto {p} no estaba en la orden"
                    )));
                }
                if crate::invariants::refund_exceeds_sold(*acc - it.quantity, it.quantity, sold_qty)
                {
                    return Err(DomainError::Invalid(format!(
                        "devolución de {p} excede lo vendido ({acc} > {sold_qty})"
                    )));
                }
                // Plan the restock into the consumed lots with remaining
                // capacity (FEFO order). Only batch-tracked, restocked lines
                // get a plan; the rest stay `None` (product.stock-only).
                if it.restock {
                    if let Some(lots) = consumed.get(p) {
                        let mut remaining = it.quantity;
                        let mut plan = Vec::new();
                        for (batch, cap) in lots {
                            if remaining == 0 {
                                break;
                            }
                            let used = lot_used.entry(batch.clone()).or_default();
                            let free = (*cap - *used).max(0);
                            let take = remaining.min(free);
                            if take > 0 {
                                plan.push(crate::inventory::model::FefoAllocation {
                                    batch: batch.clone(),
                                    qty: take,
                                });
                                *used += take;
                                remaining -= take;
                            }
                        }
                        if !plan.is_empty() {
                            restock_plans[i] = Some(plan);
                        }
                    }
                }
            }
        }
    }

    let total: Decimal = req
        .items
        .iter()
        .map(|i| i.unit_price * Decimal::from(i.quantity))
        .sum();

    let applied = repo::apply_refund(
        db,
        tenant,
        processed_by,
        order_thing.as_ref(),
        &req,
        &restock_plans,
        total,
    )
    .await?;

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

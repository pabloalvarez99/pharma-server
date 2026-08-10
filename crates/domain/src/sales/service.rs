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
use std::sync::Arc;

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
///
/// El lock vive en [`crate::locks`] porque desde V2 (stock por sucursal) NO es
/// exclusivo de la venta: la transferencia entre sucursales escribe las mismas
/// filas (`product`, `stock_movement`, `product_branch_stock`) y debe
/// serializarse contra la venta, no en paralelo a ella.
fn tenant_sale_lock(tenant: &Thing) -> Arc<AsyncMutex<()>> {
    crate::locks::tenant_stock_lock(tenant)
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
    // Subtotal y descuento se normalizan a la moneda del tenant ANTES de restar,
    // así `subtotal − discount == total` sigue siendo exacto en la misma
    // granularidad en que se persiste y se cobra. En CLP (0 decimales) esto es
    // idéntico a lo que hacía el sistema; en USD conserva los centavos que un
    // `round_dp(0)` heredado de Chile habría borrado.
    let currency = crate::settings::currency(db, tenant).await?;
    let subtotal = currency.round(crate::invariants::order_subtotal(
        req.items.iter().map(|i| (i.unit_price, i.quantity)),
    ));
    let discount_in = req.discount.unwrap_or_default();
    let discount = currency.round(crate::invariants::clamp_discount(discount_in, subtotal));
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

    // Fiado (venta a cuenta) exige cliente: sin cliente no hay a quién cobrarle.
    if req.payment_method == "pos_fiado" && customer.is_none() {
        return Err(DomainError::Invalid(
            "para fiar debes elegir el cliente".into(),
        ));
    }

    // Sucursal de la venta (V2, migración 0041). Precedencia:
    //   1. `req.branch` — el POS manda la sucursal activa del selector, y el
    //      agente puede decir "vendé en el local 2".
    //   2. la sesión de caja abierta del cajero — "la venta descuenta de la
    //      sucursal de la caja" sin que el cliente tenga que mandar nada.
    //   3. casa matriz (`None`) — negocio de un solo local: se comporta igual
    //      que antes de V2, que es lo que hace este cambio compatible hacia
    //      atrás con todo instalado hoy.
    let branch = match req.branch.as_deref() {
        Some(b) => crate::stock::service::parse_branch(Some(b))?,
        None => match sold_by {
            Some(u) => crate::cash_register::service::open_session_branch(db, tenant, u).await?,
            None => None,
        },
    };
    crate::stock::service::ensure_branch(db, tenant, branch.as_ref(), "la sucursal").await?;

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
            // On-hand del BUCKET de la sucursal donde se vende (migración 0041),
            // en una sola query para no pagar N round-trips en el hot path. Un
            // producto sin fila cuenta como 0: esa sucursal nunca recibió nada.
            // Se re-lee en cada intento, igual que el stock global.
            let branch_stock = crate::stock::repo::branch_stock_qty_many(
                db,
                tenant,
                &product_things,
                branch.as_ref(),
            )
            .await?;
            // Acumulador por producto: una venta puede traer el mismo SKU en
            // dos líneas y la suma no puede pasarse del saldo de la sucursal.
            let mut taken: HashMap<String, i64> = HashMap::new();
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
                // Aislamiento por sucursal: el stock global puede alcanzar y aun
                // así NO se puede vender, porque las unidades están en otro
                // local. Sin este chequeo el bucket de la sucursal quedaría
                // negativo y "vender en A" consumiría lo de B.
                if is_physical {
                    let key = pthing.to_string();
                    let here = branch_stock.get(&key).copied().unwrap_or(0);
                    let acc = taken.entry(key).or_insert(0);
                    *acc += req_item.quantity;
                    if here < *acc {
                        return Err(DomainError::InsufficientStock);
                    }
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
                            branch.as_ref(),
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
                branch.as_ref(),
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

    // Fiado: la venta a cuenta genera un CARGO en el ledger del cliente (deuda).
    // Idempotente por orden (post_cargo verifica). NO toca caja (no es efectivo).
    if req.payment_method == "pos_fiado" {
        if let Some(c) = customer.as_ref() {
            let order_thing = parse_tenant_thing(&applied.order.id, "order")?;
            crate::credit::repo::post_cargo(db, tenant, c, &order_thing, total, sold_by).await?;
        }
    }

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

// --- web pickup orders (Free Web PR3, ADR-0020) ------------------------------

/// Marker prefix for "product exists but cannot be sold on the web" — the api
/// layer intercepts it (same scheme as `IDEMPOTENCY_CACHED`) and answers 422
/// `PRODUCT_NOT_AVAILABLE` instead of a generic 400.
pub const PRODUCT_NOT_AVAILABLE_MARKER: &str = "PRODUCT_NOT_AVAILABLE";

/// Derive a `RET-XXXX` pickup code from fresh UUID bytes over the unambiguous
/// alphabet (no 0/O/1/I/L).
fn mint_pickup_code() -> String {
    let bytes = *uuid::Uuid::new_v4().as_bytes();
    let mut code = String::from("RET-");
    for b in &bytes[..4] {
        code.push(PICKUP_CODE_ALPHABET[*b as usize % PICKUP_CODE_ALPHABET.len()] as char);
    }
    code
}

/// Create a web pickup order: availability check (`active && online_visible
/// && prescription 'direct'`), oversell guard on `stock - stock_reserved`,
/// reservation + order + items in ONE tx (per-tenant serialized, same lock as
/// POS sales). Prices come from the product row (`online_price ?? price`) —
/// client-supplied money is never trusted.
///
/// Idempotency mirrors `post_sale`: keys are namespaced `web:` so storefront
/// retries can never collide with POS keys; a replay surfaces as
/// `DomainError::Conflict("IDEMPOTENCY_CACHED:<json>")` for the handler to
/// unwrap into a 200.
pub async fn create_web_order(
    db: &Db,
    tenant: &Thing,
    idempotency_key: Option<&str>,
    req: WebPickupOrderRequest,
) -> DomainResult<WebPickupOrderResponse> {
    if req.customer.name.trim().is_empty() || req.customer.phone.trim().is_empty() {
        return Err(DomainError::Invalid("datos de cliente inválidos".into()));
    }
    if req.items.is_empty() || req.items.len() > WEB_ORDER_MAX_ITEMS {
        return Err(DomainError::Invalid(format!(
            "items debe tener entre 1 y {WEB_ORDER_MAX_ITEMS} líneas"
        )));
    }
    for it in &req.items {
        if it.qty < 1 || it.qty > WEB_ORDER_MAX_QTY {
            return Err(DomainError::Invalid(format!(
                "cantidad inválida para {}: {}",
                it.product_id, it.qty
            )));
        }
    }
    if let Some(f) = &req.fulfillment {
        if let Some(kind) = f.kind.as_deref() {
            if kind != "pickup" {
                return Err(DomainError::Invalid(
                    "solo retiro en tienda (pickup) está disponible".into(),
                ));
            }
        }
    }

    // Namespaced idempotency: web keys never collide with POS keys.
    let web_key = idempotency_key.map(|k| format!("web:{k}"));
    let body_fp = if web_key.is_some() {
        Some(
            serde_json::to_string(&req)
                .map_err(|e| DomainError::Other(anyhow::anyhow!("body fingerprint: {e}")))?,
        )
    } else {
        None
    };
    if let (Some(key), Some(fp)) = (web_key.as_deref(), body_fp.as_deref()) {
        match repo::lookup_idempotency(db, tenant, key, fp).await? {
            repo::IdempotencyHit::Replay { response_json, .. } => {
                return Err(DomainError::Conflict(format!(
                    "IDEMPOTENCY_CACHED:{response_json}"
                )));
            }
            repo::IdempotencyHit::Conflict => {
                return Err(DomainError::Conflict(
                    "IDEMPOTENCY_KEY_REUSE_CONFLICT: la misma Idempotency-Key \
                     se reutilizó con un body distinto"
                        .to_string(),
                ));
            }
            repo::IdempotencyHit::None => {}
        }
    }

    let product_things: Vec<Thing> = req
        .items
        .iter()
        .map(|i| parse_tenant_thing(&i.product_id, "product"))
        .collect::<DomainResult<Vec<_>>>()?;

    // === Serialized critical section (same tenant lock as POS sales) ===
    // Availability + oversell check re-runs each retry so a losing MVCC
    // conflict against a concurrent writer re-reads fresh counters.
    let order = {
        let sale_lock = tenant_sale_lock(tenant);
        let _guard = sale_lock.lock().await;
        let mut attempt = 0usize;
        loop {
            let loaded = repo::load_products_for_web_order(db, tenant, &product_things).await?;
            let by_id: HashMap<String, &repo::WebOrderProductRow> =
                loaded.iter().map(|p| (p.id.to_string(), p)).collect();

            let mut lines: Vec<repo::WebOrderLine> = Vec::with_capacity(req.items.len());
            let mut subtotal = Decimal::ZERO;
            for (it, pthing) in req.items.iter().zip(product_things.iter()) {
                // Missing, inactive, hidden and prescription-only are all the
                // same "not available" — the write seam never enumerates.
                let p = by_id.get(&pthing.to_string()).copied().ok_or_else(|| {
                    DomainError::Invalid(format!(
                        "{PRODUCT_NOT_AVAILABLE_MARKER}:{}",
                        it.product_id
                    ))
                })?;
                let direct = p.prescription_type.as_deref().unwrap_or("direct") == "direct";
                if !p.active || !p.online_visible || !direct {
                    return Err(DomainError::Invalid(format!(
                        "{PRODUCT_NOT_AVAILABLE_MARKER}:{}",
                        it.product_id
                    )));
                }
                if p.physical_stock && p.stock - p.stock_reserved < it.qty {
                    return Err(DomainError::InsufficientStock);
                }
                let unit_price = p.online_price.unwrap_or(p.price);
                let line_sub = crate::invariants::line_total(unit_price, it.qty);
                subtotal += line_sub;
                lines.push(repo::WebOrderLine {
                    product: pthing.clone(),
                    product_name: p.name.clone(),
                    quantity: it.qty,
                    unit_price,
                    subtotal: line_sub,
                });
            }

            let notes = req
                .fulfillment
                .as_ref()
                .and_then(|f| f.notes.as_deref())
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let expires_at = chrono::Utc::now() + chrono::Duration::hours(WEB_ORDER_RESERVE_HOURS);

            match repo::apply_web_order(
                db,
                tenant,
                &lines,
                req.customer.name.trim(),
                req.customer.phone.trim(),
                notes,
                &mint_pickup_code(),
                expires_at,
                subtotal,
                subtotal,
            )
            .await
            {
                Ok(order) => break order,
                Err(e) if attempt < MAX_SALE_COMMIT_RETRIES && is_retryable_conflict(&e) => {
                    attempt += 1;
                    conflict_backoff(attempt).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    };

    let resp = WebPickupOrderResponse {
        order_id: order.id.clone(),
        pickup_code: order.pickup_code.clone().unwrap_or_default(),
        status: order.status.clone(),
        currency: crate::settings::currency(db, tenant)
            .await?
            .code()
            .to_string(),
        total: order.total.to_string(),
        expires_at: order.expires_at.unwrap_or_default(),
    };

    if let (Some(key), Some(fp)) = (web_key.as_deref(), body_fp.as_deref()) {
        let json = serde_json::to_string(&resp).map_err(|e| DomainError::Other(e.into()))?;
        repo::store_idempotency(db, tenant, key, fp, &json, 201).await?;
    }

    Ok(resp)
}

/// Web pickup lifecycle: `reserved → preparing → ready_for_pickup →
/// completed`; anything pre-completed may go to `cancelled`. Cancel releases
/// the reservation; complete releases it AND decrements `stock` in the same
/// tx (simple decrement — the POS-paid handoff refines this later).
pub async fn transition_web_order(
    db: &Db,
    tenant: &Thing,
    order_id: &str,
    to: &str,
) -> DomainResult<OrderDto> {
    let order_thing = parse_tenant_thing(order_id, "order")?;

    let allowed = |from: &str, to: &str| -> bool {
        matches!(
            (from, to),
            ("reserved", "preparing")
                | ("preparing", "ready_for_pickup")
                | ("ready_for_pickup", "completed")
                | ("reserved", "cancelled")
                | ("preparing", "cancelled")
                | ("ready_for_pickup", "cancelled")
        )
    };

    // Same per-tenant lock as sales/refunds: the release/decrement UPDATE must
    // never race a concurrent stock writer.
    let sale_lock = tenant_sale_lock(tenant);
    let _guard = sale_lock.lock().await;

    let (order, items) = repo::get_web_order(db, tenant, &order_thing)
        .await?
        .ok_or(DomainError::NotFound)?;
    if !allowed(&order.status, to) {
        return Err(DomainError::Invalid(format!(
            "transición inválida de '{}' a '{to}'",
            order.status
        )));
    }
    let release = to == "cancelled" || to == "completed";
    let decrement = to == "completed";
    let set_ready = to == "ready_for_pickup";
    // El retiro descuenta del local donde se preparó el pedido: la sucursal
    // quedó estampada en la orden al reservarla. `None` = casa matriz.
    let branch = crate::stock::service::parse_branch(order.branch.as_deref())?;

    let mut attempt = 0usize;
    loop {
        match repo::apply_web_transition(
            db,
            tenant,
            &order_thing,
            to,
            &items,
            branch.as_ref(),
            release,
            decrement,
            set_ready,
        )
        .await
        {
            Ok(updated) => return Ok(updated),
            Err(e) if attempt < MAX_SALE_COMMIT_RETRIES && is_retryable_conflict(&e) => {
                attempt += 1;
                conflict_backoff(attempt).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
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
///
/// `footer_note` sale de [`crate::settings::receipt_footer_note`]: lo que este
/// negocio configuró, o un default armado con su propio nombre.
pub async fn get_receipt(db: &Db, tenant: &Thing, id: &str) -> DomainResult<ReceiptDto> {
    let order_thing = parse_tenant_thing(id, "order")?;
    let (order, items) = repo::get_order(db, tenant, &order_thing)
        .await?
        .ok_or(DomainError::NotFound)?;

    let tenant_name = repo::tenant_name(db, tenant).await?.unwrap_or_default();
    let footer_note = crate::settings::receipt_footer_note(db, tenant, &tenant_name).await?;
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
    //
    // El vuelto se redondea a la moneda del tenant: en CLP no existe medio peso
    // en el cajón, en USD sí existe el centavo. `round` con 0 decimales es
    // exactamente lo que hacía el sistema cuando la moneda era una constante.
    let currency = crate::settings::currency(db, tenant).await?;
    let change = match order.payment_method.as_str() {
        "pos_cash" => order
            .cash_amount
            .map(|cash| currency.round(crate::invariants::cash_change(cash, order.total))),
        "pos_mixed" => {
            let tendered =
                order.cash_amount.unwrap_or_default() + order.card_amount.unwrap_or_default();
            Some(currency.round(crate::invariants::cash_change(
                tendered,
                order.total,
            )))
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
        // El pie que configuró este negocio, o el que sale de su propio nombre.
        // `tenant_name` ya está leído acá arriba: el default no cuesta una
        // consulta más.
        footer_note,
    })
}

/// Caja abierta donde tiene que salir el efectivo de una devolución, con su
/// lock de mutación ya tomado (migración 0049).
///
/// El lock se sostiene desde ANTES de re-verificar el estado y hasta que la
/// transacción de la devolución commitea: es el mismo que toman
/// `cash_register::add_movement` y `close_session`, así que un `close_session`
/// concurrente no puede congelar el esperado justo antes de que aterrice el
/// retiro (faltante fantasma). Mismo patrón que
/// `expenses::service::create_expense`.
///
/// Sin caja abierta ⇒ `Conflict`, no un retiro al vacío: el reembolso en
/// efectivo es plata que sale y necesita asiento.
///
/// Con MÁS de una caja abierta ⇒ `Conflict` también. `NewDevolucion` no tiene
/// campo para elegir cuál, y adivinar sería mover plata del cajón equivocado.
/// Es el mismo agujero que tiene hoy el evento `cash_sales_running_maint`, que
/// le suma cada venta a TODAS las cajas abiertas del tenant (defecto conocido,
/// territorio del carril de sucursales); acá al menos falla ruidoso.
async fn open_session_for_refund(
    db: &Db,
    tenant: &Thing,
) -> DomainResult<(Thing, tokio::sync::OwnedMutexGuard<()>)> {
    let mut r = db
        .query("SELECT VALUE id FROM cash_register_session WHERE tenant = $t AND status = 'open'")
        .bind(("t", tenant.clone()))
        .await?
        .check()?;
    let open: Vec<Thing> = r.take(0)?;
    let session = match open.len() {
        0 => {
            return Err(DomainError::Conflict(
                "no hay caja abierta: abrí la caja antes de devolver en efectivo".into(),
            ))
        }
        1 => open.into_iter().next().unwrap(),
        n => {
            return Err(DomainError::Conflict(format!(
                "hay {n} cajas abiertas: no se puede saber de cuál sale el efectivo"
            )))
        }
    };
    let guard = crate::cash_register::service::session_mutation_lock(tenant, &session.to_string())
        .lock_owned()
        .await;
    // Re-chequeo BAJO el lock: entre el SELECT y el lock, un `close_session`
    // pudo cerrarla.
    let mut sr = db
        .query("SELECT status FROM cash_register_session WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", session.clone()))
        .bind(("t", tenant.clone()))
        .await?
        .check()?;
    let status: Option<String> = sr.take((0, "status"))?;
    if status.as_deref() != Some("open") {
        return Err(DomainError::Conflict(
            "la caja se cerró: abrí la caja antes de devolver en efectivo".into(),
        ));
    }
    Ok((session, guard))
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

    // Sucursal a la que vuelve la mercadería: la de la venta original. Una
    // devolución suelta (sin orden) repone en la casa matriz. Se resuelve dentro
    // del bloque de la orden más abajo.
    let mut refund_branch: Option<Thing> = None;

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

    // La venta original, si la hay: de acá salen el acumulado ya devuelto, el
    // total a cubrir y si fue fiada. Se necesita después del bloque para
    // derivar los efectos de plata (migración 0049).
    let mut sale: Option<crate::sales::model::OrderDto> = None;

    if let Some(ord) = order_thing.as_ref() {
        let (order_row, sold_items) = repo::get_order(db, tenant, ord)
            .await?
            .ok_or(DomainError::NotFound)?;
        sale = Some(order_row.clone());
        // La devolución repone EN LA SUCURSAL DONDE SE VENDIÓ: devolver en el
        // local A no puede inflar el stock del local B.
        refund_branch = crate::stock::service::parse_branch(order_row.branch.as_deref())?;
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

    // === Efectos de plata de la devolución (migración 0049) ==================
    //
    // Dos señales, las dos hechos y no adivinanzas:
    //   `metodo_reembolso == 'efectivo'` ⇒ la plata salió del cajón.
    //   la venta original fue `pos_fiado` ⇒ el cliente nunca la pagó, así que
    //   devolver mercadería baja la deuda.
    // Cualquier otra cosa (tarjeta, o método sin registrar) no toca plata: una
    // devolución con tarjeta vuelve por el procesador, y de un campo vacío no
    // se inventa un movimiento de caja.
    //
    // Ojo con la asimetría contra `invariants::cash_into_drawer`, que trata
    // `None` como "entró todo": ahí `cash_amount` es un campo OPCIONAL de una
    // venta que sí se cobró. Acá `metodo_reembolso` es la ÚNICA señal de que
    // hubo plata en movimiento; inventar un retiro desde el silencio fabrica
    // faltantes, que es justo la clase de bug que esto cierra.
    let refund_cash = req.metodo_reembolso.as_deref() == Some("efectivo");
    let mut effects = repo::RefundEffects::default();
    if let Some(s) = sale.as_ref() {
        let acc = s.refunded_total + total;
        effects.refunded_total = Some(acc);
        effects.mark_refunded = acc >= s.total;
        if !refund_cash && s.payment_method == "pos_fiado" && total > Decimal::ZERO {
            let cust = s.customer.as_deref().ok_or_else(|| {
                DomainError::Invalid("venta fiada sin cliente: no hay deuda que revertir".into())
            })?;
            effects.ledger_reversal = Some((parse_tenant_thing(cust, "customer")?, total));
        }
    }
    // El retiro necesita una caja abierta donde salga la plata. Sin caja no se
    // deja caer en el vacío: se rechaza y se dice por qué. Un reembolso en
    // efectivo sin asiento es plata que se va sin libro — el agujero exacto que
    // este carril cierra.
    let mut drawer_guard = None;
    if refund_cash && total > Decimal::ZERO {
        let (session, guard) = open_session_for_refund(db, tenant).await?;
        drawer_guard = Some(guard);
        effects.cash_retiro = Some((session, total));
    }

    let applied = repo::apply_refund(
        db,
        tenant,
        processed_by,
        order_thing.as_ref(),
        refund_branch.as_ref(),
        &req,
        &restock_plans,
        total,
        &effects,
    )
    .await?;
    drop(drawer_guard);

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
    // El k/v es genérico a propósito, pero las keys de plata se validan acá:
    // es el único punto por el que entra el `PUT /api/v1/settings/{key}`, y una
    // moneda mal escrita no se puede descubrir recién en la próxima venta.
    let value = match key {
        crate::money::CURRENCY_SETTING_KEY => {
            crate::money::Currency::parse(value)?.code().to_string()
        }
        crate::money::TAX_RATE_SETTING_KEY => crate::money::parse_tax_percent(value)?
            .normalize()
            .to_string(),
        _ => value.to_string(),
    };
    repo::upsert_setting(db, tenant, key, &value).await
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

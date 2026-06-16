//! Purchasing business logic: validation, compare best supplier, mapping.
//! Thin orchestration over [`super::repo`].

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use rust_decimal::Decimal;
use surrealdb::sql::Thing;
use tokio::sync::Mutex as AsyncMutex;

use crate::errors::{DomainError, DomainResult};
use crate::money::CURRENCY_CLP;

use super::model::*;
use super::repo;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Per-(tenant,purchase_order) serialization of every PO *state mutation*
/// (send / receive / receive-lines / payment / cancel). Each of those is a
/// check-then-act TOCTOU on the PO status + AP ledger: the handler reads the
/// status (and, for payments, the running `paid`; for receipts, the current
/// `product.stock`/WAC base) and then writes WITHOUT a compare-and-swap guard
/// on the `WHERE id=$po` UPDATE. Concurrent callers on the SAME PO each pass
/// the pre-check and each apply, breaking money/stock integrity: two receipts
/// add stock twice + recompute WAC against a stale base + duplicate the
/// `stock_movement` (phantom inventory); a receive racing a cancel moves stock
/// on a PO that also flips to `cancelled` (status/stock disagree); two payments
/// both pass the `Σ payments ≤ total` check and overpay; a payment racing a
/// cancel posts money against a void doc, breaking `cancelled ⇒ Σ payments = 0`.
/// Serializing per PO removes the whole family (same proven approach as
/// `sales::service::SALE_LOCKS` / `cash_register::service` OPEN/SESSION locks).
/// Different POs never share a lock, so throughput is unaffected.
static PO_LOCKS: OnceLock<std::sync::Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();

fn purchase_order_lock(tenant: &Thing, po: &Thing) -> Arc<AsyncMutex<()>> {
    let locks = PO_LOCKS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut guard = locks.lock().expect("PO_LOCKS mutex poisoned");
    guard
        .entry(format!("{tenant}|{po}"))
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

fn parse_thing(s: &str) -> DomainResult<Thing> {
    surrealdb::sql::thing(s).map_err(|_| DomainError::Invalid(format!("id inválido: {s}")))
}

fn parse_typed(s: &str, table: &str) -> DomainResult<Thing> {
    let t = parse_thing(s)?;
    if t.tb != table {
        return Err(DomainError::Invalid(format!(
            "id debe pertenecer a `{table}`"
        )));
    }
    Ok(t)
}

// --- suppliers -------------------------------------------------------------

pub async fn create_supplier(
    db: &Db,
    tenant: &Thing,
    input: NewSupplier,
) -> DomainResult<SupplierDto> {
    if input.name.trim().is_empty() {
        return Err(DomainError::Invalid("nombre requerido".into()));
    }
    repo::create_supplier(db, tenant, &input).await
}

pub async fn list_suppliers(
    db: &Db,
    tenant: &Thing,
    filters: SupplierFilters,
) -> DomainResult<Vec<SupplierDto>> {
    repo::list_suppliers(db, tenant, &filters).await
}

pub async fn get_supplier(db: &Db, tenant: &Thing, id: &str) -> DomainResult<SupplierDto> {
    let id = parse_typed(id, "supplier")?;
    repo::get_supplier(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)
}

pub async fn update_supplier(
    db: &Db,
    tenant: &Thing,
    id: &str,
    patch: UpdateSupplier,
) -> DomainResult<SupplierDto> {
    let id = parse_typed(id, "supplier")?;
    repo::update_supplier(db, tenant, &id, &patch).await
}

pub async fn delete_supplier(db: &Db, tenant: &Thing, id: &str) -> DomainResult<()> {
    let id = parse_typed(id, "supplier")?;
    if repo::soft_delete_supplier(db, tenant, &id).await? {
        Ok(())
    } else {
        Err(DomainError::NotFound)
    }
}

// --- mapping ---------------------------------------------------------------

async fn resolve_supplier(db: &Db, tenant: &Thing, id: &str) -> DomainResult<Thing> {
    let t = parse_typed(id, "supplier")?;
    if !repo::supplier_belongs(db, tenant, &t).await? {
        return Err(DomainError::Invalid(
            "el proveedor no existe en este tenant".into(),
        ));
    }
    Ok(t)
}

async fn resolve_product(db: &Db, tenant: &Thing, id: &str) -> DomainResult<Thing> {
    let t = parse_typed(id, "product")?;
    if !repo::product_belongs(db, tenant, &t).await? {
        return Err(DomainError::Invalid(
            "el producto no existe en este tenant".into(),
        ));
    }
    Ok(t)
}

pub async fn map_product(
    db: &Db,
    tenant: &Thing,
    supplier_id: &str,
    input: MapSupplierProduct,
) -> DomainResult<SupplierProductMappingDto> {
    if input.supplier_code.trim().is_empty() {
        return Err(DomainError::Invalid("supplier_code requerido".into()));
    }
    let supplier = resolve_supplier(db, tenant, supplier_id).await?;
    let product = resolve_product(db, tenant, &input.product).await?;
    repo::create_mapping(db, tenant, &supplier, &product, input.supplier_code.trim())
        .await
        .map_err(|e| match e {
            // Unique-index violation surfaces as a generic Db error; map to
            // CONFLICT so the caller gets a 409 instead of 500.
            DomainError::Db(boxed) => {
                let msg = boxed.to_string().to_lowercase();
                if msg.contains("already") || msg.contains("index") || msg.contains("unique") {
                    DomainError::Conflict(
                        "ya existe un mapping para este (proveedor, supplier_code)".into(),
                    )
                } else {
                    DomainError::Db(boxed)
                }
            }
            other => other,
        })
}

// --- prices ----------------------------------------------------------------

pub async fn create_price(
    db: &Db,
    tenant: &Thing,
    input: NewSupplierPrice,
) -> DomainResult<SupplierPriceDto> {
    if input.unit_cost < Decimal::ZERO {
        return Err(DomainError::Invalid(
            "unit_cost no puede ser negativo".into(),
        ));
    }
    let supplier = resolve_supplier(db, tenant, &input.supplier).await?;
    let product = match input.product.as_deref() {
        Some(s) if !s.is_empty() => Some(resolve_product(db, tenant, s).await?),
        _ => None,
    };
    let currency = input.currency.as_deref().unwrap_or(CURRENCY_CLP);
    repo::create_price(
        db,
        tenant,
        &supplier,
        product,
        input.supplier_code.as_deref(),
        input.description.as_deref(),
        input.unit_cost,
        currency,
        input.valid_from,
    )
    .await
}

pub async fn list_prices(
    db: &Db,
    tenant: &Thing,
    filters: SupplierPriceFilters,
) -> DomainResult<Vec<SupplierPriceDto>> {
    repo::list_prices(db, tenant, &filters).await
}

/// Pick cheapest supplier for each requested item. If `product` is given,
/// `savings = product.cost_price − best.unit_cost`. If only `supplier_code`
/// is given, savings is `None`.
pub async fn compare(
    db: &Db,
    tenant: &Thing,
    req: CompareRequest,
) -> DomainResult<CompareResponse> {
    let mut out = Vec::with_capacity(req.items.len());
    for item in req.items {
        let (best, current_cost) = if let Some(pid) = item.product.as_deref() {
            let p = resolve_product(db, tenant, pid).await?;
            let best = repo::best_price_for_product(db, tenant, &p).await?;
            let cost = repo::product_cost_price(db, tenant, &p).await?;
            (best, cost)
        } else if let Some(code) = item.supplier_code.as_deref() {
            (repo::best_price_for_code(db, tenant, code).await?, None)
        } else {
            return Err(DomainError::Invalid(
                "cada item requiere `product` o `supplier_code`".into(),
            ));
        };
        let best = best.map(|(dto, supplier_name)| CompareBest {
            supplier: dto.supplier.clone(),
            supplier_name,
            unit_cost: dto.unit_cost,
            price_id: dto.id,
            valid_from: dto.valid_from,
        });
        let savings = match (&best, current_cost) {
            (Some(b), Some(c)) => Some(c - b.unit_cost),
            _ => None,
        };
        out.push(CompareResult {
            product: item.product,
            supplier_code: item.supplier_code,
            best,
            current_cost,
            savings,
        });
    }
    Ok(CompareResponse { items: out })
}

// --- purchase orders (Fase 5-full, BACKLOG #8 slice 1) ---------------------

/// Create a `draft` purchase order. Validates the supplier is in this tenant,
/// every line (qty > 0, unit_cost ≥ 0, product_name present, product — if
/// given — is in this tenant), computes per-line `subtotal` and the header
/// `total`, and persists header + lines atomically.
///
/// Receipt (stock + weighted average cost recompute) and accounts-payable
/// stay deferred to later BACKLOG #8 slices — this only creates the document.
pub async fn create_purchase_order(
    db: &Db,
    tenant: &Thing,
    input: NewPurchaseOrder,
) -> DomainResult<PurchaseOrderDto> {
    if input.items.is_empty() {
        return Err(DomainError::Invalid(
            "la orden de compra requiere al menos un ítem".into(),
        ));
    }
    let supplier = resolve_supplier(db, tenant, &input.supplier).await?;
    let currency = input.currency.as_deref().unwrap_or(CURRENCY_CLP);

    let mut lines = Vec::with_capacity(input.items.len());
    let mut total = Decimal::ZERO;
    for it in input.items {
        if it.product_name.trim().is_empty() {
            return Err(DomainError::Invalid("product_name requerido".into()));
        }
        if it.quantity <= 0 {
            return Err(DomainError::Invalid("quantity debe ser mayor a 0".into()));
        }
        if it.unit_cost < Decimal::ZERO {
            return Err(DomainError::Invalid(
                "unit_cost no puede ser negativo".into(),
            ));
        }
        let product = match it.product.as_deref() {
            Some(s) if !s.is_empty() => Some(resolve_product(db, tenant, s).await?),
            _ => None,
        };
        let subtotal = it.unit_cost * Decimal::from(it.quantity);
        total += subtotal;
        lines.push(repo::PoLine {
            product,
            product_name: it.product_name.trim().to_string(),
            quantity: it.quantity,
            unit_cost: it.unit_cost,
            subtotal,
        });
    }

    repo::create_purchase_order(
        db,
        tenant,
        &supplier,
        currency,
        input.notes.as_deref(),
        input.external_ref.as_deref(),
        total,
        &lines,
    )
    .await
}

pub async fn list_purchase_orders(
    db: &Db,
    tenant: &Thing,
    filters: PurchaseOrderFilters,
) -> DomainResult<Vec<PurchaseOrderDto>> {
    repo::list_purchase_orders(db, tenant, &filters).await
}

/// Issue a `draft` purchase order to the supplier (`draft → sent`). This is
/// the lifecycle transition that makes goods receipt reachable end-to-end:
/// `create_purchase_order` leaves the PO in `draft`, but
/// `receive_purchase_order_lines` (the only receive path wired to HTTP) only
/// accepts `sent`/`approved`/`partially_received`. Without a way to leave
/// `draft` the operator could create an OC and never receive against it
/// (BUG-bob-002). Refuses any status other than `draft` with `Conflict` (a PO
/// already sent/received/cancelled can't be re-issued).
pub async fn send_purchase_order(
    db: &Db,
    tenant: &Thing,
    id: &str,
) -> DomainResult<PurchaseOrderDto> {
    let po = parse_typed(id, "purchase_order")?;
    let po_lock = purchase_order_lock(tenant, &po);
    let _po_guard = po_lock.lock().await;
    let (status, _total, _currency) = repo::purchase_order_belongs(db, tenant, &po)
        .await?
        .ok_or(DomainError::NotFound)?;
    if status != "draft" {
        return Err(DomainError::Conflict(format!(
            "orden en estado '{status}' no puede pasar a 'sent' (solo desde 'draft')"
        )));
    }
    repo::set_purchase_order_status(db, tenant, &po, "sent").await?;
    repo::get_purchase_order(db, tenant, &po)
        .await?
        .ok_or(DomainError::NotFound)
}

pub async fn get_purchase_order(
    db: &Db,
    tenant: &Thing,
    id: &str,
) -> DomainResult<PurchaseOrderDto> {
    let id = parse_typed(id, "purchase_order")?;
    repo::get_purchase_order(db, tenant, &id)
        .await?
        .ok_or(DomainError::NotFound)
}

/// Receive a `draft` purchase order (BACKLOG #8 slice 2): for each catalogued
/// line bump `product.stock` by the line `quantity`, recompute
/// `product.cost_price` as the weighted average cost
/// `(old_stock·old_cost + Σqty·unit_cost) / (old_stock + Σqty)`, append an
/// audit `stock_movement(reason='purchase_receipt', ref=po_id)`, and flip
/// the PO to `received` — all in one `BEGIN/COMMIT` so a crash can't bump
/// stock without the movement (mirrors `sales::repo::apply_sale`; preserves
/// the `product.stock = SUM(stock_movement.delta)` invariant).
///
/// Free-text lines (no `product`) are accounting-only — they don't move
/// stock. Multiple lines on the same product are aggregated before the WAC
/// computation. WAC base: if `old_cost is None`, treat it as the line
/// `unit_cost` so a first receipt seeds `cost_price` without diluting it
/// with a phantom zero.
///
/// Receipt is one-shot per PO (status must be `draft`). Partial receipt and
/// receipt of an inactive product are out of scope for this slice;
/// accounts-payable (`purchase_payment`) stays deferred to slice 3.
pub async fn receive_purchase_order(
    db: &Db,
    tenant: &Thing,
    id: &str,
    admin: Option<&str>,
) -> DomainResult<PurchaseOrderDto> {
    let po = parse_typed(id, "purchase_order")?;
    let po_lock = purchase_order_lock(tenant, &po);
    let _po_guard = po_lock.lock().await;
    let current = repo::get_purchase_order(db, tenant, &po)
        .await?
        .ok_or(DomainError::NotFound)?;
    if current.status != "draft" {
        return Err(DomainError::Conflict(format!(
            "orden en estado '{}' no puede pasar a 'received' (solo desde 'draft')",
            current.status
        )));
    }

    // Aggregate catalogued lines per product so a PO with two lines on the
    // same product still produces one WAC recompute + one stock_movement.
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<String, (Thing, i64, Decimal)> = BTreeMap::new();
    for line in &current.items {
        let Some(pid_str) = line.product.as_deref() else {
            continue;
        };
        let pid = parse_typed(pid_str, "product")?;
        let entry = buckets
            .entry(pid_str.to_string())
            .or_insert_with(|| (pid, 0i64, Decimal::ZERO));
        entry.1 += line.quantity;
        // Σ(qty · unit_cost), accumulated for the weighted average.
        entry.2 += line.unit_cost * Decimal::from(line.quantity);
    }

    let mut effects = Vec::with_capacity(buckets.len());
    for (_pid_str, (pid, add_qty, cost_sum)) in buckets {
        let (old_stock, old_cost_opt) = repo::product_stock_cost(db, tenant, &pid)
            .await?
            .ok_or_else(|| {
                DomainError::Invalid(format!("producto no existe en este tenant: {pid}"))
            })?;
        let line_avg_cost = cost_sum / Decimal::from(add_qty);
        let new_cost = match old_cost_opt {
            // No prior cost → first receipt seeds the cost with the line
            // average; otherwise old_cost weight would dilute to zero.
            None => line_avg_cost,
            Some(_) if old_stock <= 0 => line_avg_cost,
            Some(old_cost) => {
                let total_qty = Decimal::from(old_stock + add_qty);
                (Decimal::from(old_stock) * old_cost + cost_sum) / total_qty
            }
        };
        effects.push(repo::ReceiveEffect {
            product: pid,
            add_qty,
            new_cost,
        });
    }

    let admin_thing = match admin {
        Some(a) if !a.is_empty() => Some(parse_thing(a)?),
        _ => None,
    };
    repo::receive_purchase_order(db, tenant, &po, admin_thing, &effects)
        .await?
        .ok_or(DomainError::NotFound)
}

/// Line-level goods receipt (recepción de mercadería). Receiving is only
/// legal from `sent`/`approved` (a `draft` hasn't been issued to the supplier
/// yet; `received`/`cancelled` are terminal) — any other status is
/// `Conflict`. Partial receipts are allowed and additive: each line carries a
/// cumulative `qty_received` that a receipt bumps, capped at the ordered
/// `quantity` (over-receipt is `Conflict`).
///
/// For each received line on a catalogued product the receipt — in one
/// `BEGIN/COMMIT` (`repo::receive_purchase_order_lines`) — bumps
/// `product.stock`, recomputes the weighted-average `cost_price`
/// `(old_stock·old_cost + Σqty·unit_cost) / (old_stock + Σqty)`, appends an
/// audit `stock_movement(reason='purchase_receive')`, bumps the line
/// `qty_received`, and (when a `lot`+`expiry_date` is given) creates or tops
/// up a `product_batch`. Free-text lines (no `product`) move no stock — they
/// only advance their `qty_received` so the PO can still reach `received`.
///
/// Final status: `received` when every line is fully received after this
/// receipt, else `partially_received`. WAC base mirrors the one-shot path: a
/// first receipt (no prior `cost`) seeds `cost_price` with the line average
/// rather than diluting against a phantom zero.
pub async fn receive_purchase_order_lines(
    db: &Db,
    tenant: &Thing,
    id: &str,
    input: ReceivePurchaseOrder,
    admin: Option<&str>,
) -> DomainResult<PurchaseOrderDto> {
    let po = parse_typed(id, "purchase_order")?;
    let po_lock = purchase_order_lock(tenant, &po);
    let _po_guard = po_lock.lock().await;
    let current = repo::get_purchase_order(db, tenant, &po)
        .await?
        .ok_or(DomainError::NotFound)?;
    // Multi-receipt path: `partially_received` must accept further receipts
    // until the PO completes to `received`.
    if !matches!(
        current.status.as_str(),
        "sent" | "approved" | "partially_received"
    ) {
        return Err(DomainError::Conflict(format!(
            "orden en estado '{}' no puede recibir mercadería (solo desde 'sent', 'approved' o 'partially_received')",
            current.status
        )));
    }
    if input.lines.is_empty() {
        return Err(DomainError::Invalid(
            "la recepción requiere al menos una línea".into(),
        ));
    }

    use std::collections::BTreeMap;
    // Index the PO's lines by id for O(1) lookup + over-receipt checks.
    let mut by_id: BTreeMap<&str, &PurchaseOrderItemDto> = BTreeMap::new();
    for it in &current.items {
        by_id.insert(it.id.as_str(), it);
    }
    // Track running received-qty per line across the receipt body so two
    // receive-lines targeting the same po_line_id can't jointly over-receive.
    let mut received_in_req: BTreeMap<String, i64> = BTreeMap::new();
    // Per-product aggregation for the WAC recompute: (Thing, Σqty, Σ(qty·cost)).
    let mut prod_buckets: BTreeMap<String, (Thing, i64, Decimal)> = BTreeMap::new();
    let mut line_effects: Vec<repo::ReceiveLineQtyEffect> = Vec::new();
    let mut batch_effects: Vec<repo::ReceiveBatchEffect> = Vec::new();

    for rl in &input.lines {
        if rl.qty_received <= 0 {
            return Err(DomainError::Invalid(
                "qty_received debe ser mayor a 0".into(),
            ));
        }
        let line_thing = parse_typed(&rl.po_line_id, "purchase_order_item")?;
        let item = by_id.get(rl.po_line_id.as_str()).copied().ok_or_else(|| {
            DomainError::Invalid(format!(
                "la línea {} no pertenece a esta orden de compra",
                rl.po_line_id
            ))
        })?;
        let prior = item.qty_received + received_in_req.get(&rl.po_line_id).copied().unwrap_or(0);
        if prior + rl.qty_received > item.quantity {
            return Err(DomainError::Conflict(format!(
                "recepción excede lo pedido en la línea {}: pedido={}, ya recibido={}, intento={}",
                rl.po_line_id, item.quantity, prior, rl.qty_received
            )));
        }
        *received_in_req.entry(rl.po_line_id.clone()).or_insert(0) += rl.qty_received;

        line_effects.push(repo::ReceiveLineQtyEffect {
            line: line_thing,
            add_qty: rl.qty_received,
        });

        // Free-text lines carry no product → no stock/WAC/batch effect.
        let Some(pid_str) = item.product.as_deref() else {
            if rl.lot.is_some() {
                return Err(DomainError::Invalid(format!(
                    "la línea {} no tiene producto catalogado; no admite lote",
                    rl.po_line_id
                )));
            }
            continue;
        };
        let pid = parse_typed(pid_str, "product")?;
        let entry = prod_buckets
            .entry(pid_str.to_string())
            .or_insert_with(|| (pid.clone(), 0i64, Decimal::ZERO));
        entry.1 += rl.qty_received;
        entry.2 += item.unit_cost * Decimal::from(rl.qty_received);

        if let Some(lot) = rl.lot.as_deref() {
            let lot = lot.trim();
            if lot.is_empty() {
                return Err(DomainError::Invalid("lot no puede ser vacío".into()));
            }
            let expiry = rl.expiry_date.ok_or_else(|| {
                DomainError::Invalid(format!(
                    "la línea {} indica lote sin expiry_date",
                    rl.po_line_id
                ))
            })?;
            let existing = repo::find_batch(db, tenant, &pid, lot).await?;
            batch_effects.push(repo::ReceiveBatchEffect {
                batch: existing,
                product: pid.clone(),
                batch_code: lot.to_string(),
                expiry_date: expiry,
                add_qty: rl.qty_received,
                cost: item.unit_cost,
            });
        }
    }

    // WAC per product (same base rule as the one-shot receive path).
    let mut product_effects = Vec::with_capacity(prod_buckets.len());
    for (_k, (pid, add_qty, cost_sum)) in prod_buckets {
        let (old_stock, old_cost_opt) = repo::product_stock_cost(db, tenant, &pid)
            .await?
            .ok_or_else(|| {
                DomainError::Invalid(format!("producto no existe en este tenant: {pid}"))
            })?;
        let line_avg_cost = cost_sum / Decimal::from(add_qty);
        let new_cost = match old_cost_opt {
            None => line_avg_cost,
            Some(_) if old_stock <= 0 => line_avg_cost,
            Some(old_cost) => {
                let total_qty = Decimal::from(old_stock + add_qty);
                (Decimal::from(old_stock) * old_cost + cost_sum) / total_qty
            }
        };
        product_effects.push(repo::ReceiveLineProductEffect {
            product: pid,
            add_qty,
            new_cost,
        });
    }

    // Final status: fully received iff every line's cumulative qty_received
    // (prior + this receipt) equals its ordered quantity.
    let fully_received = current.items.iter().all(|it| {
        let added = received_in_req.get(&it.id).copied().unwrap_or(0);
        it.qty_received + added >= it.quantity
    });
    let final_status = if fully_received {
        "received"
    } else {
        "partially_received"
    };

    let admin_thing = match admin {
        Some(a) if !a.is_empty() => Some(parse_thing(a)?),
        _ => None,
    };
    repo::receive_purchase_order_lines(
        db,
        tenant,
        &po,
        admin_thing,
        final_status,
        &product_effects,
        &line_effects,
        &batch_effects,
    )
    .await?
    .ok_or(DomainError::NotFound)
}

// --- accounts payable (Fase 5-full, BACKLOG #8 slice 3) --------------------

const ALLOWED_PAYMENT_METHODS: &[&str] = &["cash", "bank", "card", "transfer"];

fn validate_payment_method(method: &str) -> DomainResult<&'static str> {
    ALLOWED_PAYMENT_METHODS
        .iter()
        .find(|m| **m == method)
        .copied()
        .ok_or_else(|| {
            DomainError::Invalid(format!(
                "payment_method inválido: {method} (use cash|bank|card|transfer)"
            ))
        })
}

/// Record a payment against a purchase order. Validates the PO is in this
/// tenant (any status except `cancelled` — payments to a cancelled PO are
/// rejected so the AP ledger doesn't accept money against a void document),
/// amount > 0, payment_method ∈ {cash,bank,card,transfer}, and that the
/// running `paid + amount ≤ total` so accidental overpayment is caught up-
/// front (operator can split a payment but not double-pay). Cash payments
/// optionally bind a `cash_register_session` (validated tenant-scoped) so
/// arqueo can include them later.
pub async fn create_purchase_payment(
    db: &Db,
    tenant: &Thing,
    po_id: &str,
    input: NewPurchasePayment,
    created_by: Option<&str>,
) -> DomainResult<PurchasePaymentDto> {
    let po = parse_typed(po_id, "purchase_order")?;
    let po_lock = purchase_order_lock(tenant, &po);
    let _po_guard = po_lock.lock().await;
    let (status, total, po_currency) = repo::purchase_order_belongs(db, tenant, &po)
        .await?
        .ok_or(DomainError::NotFound)?;
    if status == "cancelled" {
        return Err(DomainError::Conflict(
            "no se puede pagar una orden de compra cancelada".into(),
        ));
    }
    if input.amount <= Decimal::ZERO {
        return Err(DomainError::Invalid("amount debe ser mayor a 0".into()));
    }
    let method = validate_payment_method(input.payment_method.as_deref().unwrap_or("cash"))?;
    let currency = input
        .currency
        .as_deref()
        .unwrap_or(&po_currency)
        .to_string();

    let cash_session = match input.cash_session.as_deref() {
        Some(s) if !s.is_empty() => {
            let t = parse_typed(s, "cash_register_session")?;
            if !repo::cash_session_belongs(db, tenant, &t).await? {
                return Err(DomainError::Invalid(
                    "la sesión de caja no existe en este tenant".into(),
                ));
            }
            Some(t)
        }
        _ => None,
    };

    let already_paid = repo::sum_payments(db, tenant, &po).await?;
    if already_paid + input.amount > total {
        return Err(DomainError::Conflict(format!(
            "el pago excede el saldo: total={total}, pagado={already_paid}, intento={}",
            input.amount
        )));
    }

    let created_by_thing = match created_by {
        Some(s) if !s.is_empty() => Some(parse_thing(s)?),
        _ => None,
    };
    repo::create_purchase_payment(
        db,
        tenant,
        &po,
        input.amount,
        &currency,
        method,
        cash_session,
        input.reference.as_deref(),
        input.note.as_deref(),
        input.paid_at,
        created_by_thing,
    )
    .await
}

/// Read the AP state of a PO: header `total` + sum of payments + remaining
/// `balance` + `fully_paid` flag + the full payment ledger (chronological).
pub async fn get_purchase_payment_summary(
    db: &Db,
    tenant: &Thing,
    po_id: &str,
) -> DomainResult<PurchasePaymentSummary> {
    let po = parse_typed(po_id, "purchase_order")?;
    let (status, total, _currency) = repo::purchase_order_belongs(db, tenant, &po)
        .await?
        .ok_or(DomainError::NotFound)?;
    let payments = repo::list_payments_for(db, tenant, &po).await?;
    let paid: Decimal = payments.iter().map(|p| p.amount).sum();
    let balance = total - paid;
    Ok(PurchasePaymentSummary {
        purchase_order: po.to_string(),
        status,
        total,
        paid,
        balance,
        fully_paid: balance <= Decimal::ZERO,
        payments,
    })
}

/// Cancel a `draft` purchase order (BACKLOG #8 Fase 5-full slice 4 — closes
/// the lifecycle: `draft → received` (slice 2) and `draft → cancelled` (here)
/// are the two valid transitions from `draft`). Refuses:
/// - status != 'draft' (received already moved stock and recomputed WAC; that
///   path needs a reversal, not a cancel — out of scope);
/// - PO has any `purchase_payment` (operator must reverse the payment first
///   so a cancelled doc never carries paid money against it — keeps the AP
///   ledger and PO status invariant consistent: `cancelled ⇒ Σ payments = 0`).
pub async fn cancel_purchase_order(
    db: &Db,
    tenant: &Thing,
    id: &str,
) -> DomainResult<PurchaseOrderDto> {
    let po = parse_typed(id, "purchase_order")?;
    let po_lock = purchase_order_lock(tenant, &po);
    let _po_guard = po_lock.lock().await;
    let (status, _total, _currency) = repo::purchase_order_belongs(db, tenant, &po)
        .await?
        .ok_or(DomainError::NotFound)?;
    if status != "draft" {
        return Err(DomainError::Conflict(format!(
            "orden en estado '{status}' no puede pasar a 'cancelled' (solo desde 'draft')"
        )));
    }
    let already_paid = repo::sum_payments(db, tenant, &po).await?;
    if already_paid > Decimal::ZERO {
        return Err(DomainError::Conflict(format!(
            "no se puede cancelar una OC con pagos asociados (pagado={already_paid}); reverse el pago primero"
        )));
    }
    repo::set_purchase_order_status(db, tenant, &po, "cancelled").await?;
    repo::get_purchase_order(db, tenant, &po)
        .await?
        .ok_or(DomainError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_typed_rejects_wrong_table() {
        let err = parse_typed("product:abc", "supplier").unwrap_err();
        assert_eq!(err.code(), "INVALID_INPUT");
    }
}

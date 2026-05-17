//! Tenant-scoped reads + decision transitions for inbound `agent_order`s.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use surrealdb::sql::{thing, Thing};

use crate::errors::{DomainError, DomainResult};

use super::model::*;

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

#[derive(Debug, Deserialize)]
struct Row {
    id: Thing,
    peer_did: String,
    status: String,
    total: f64,
    currency: String,
    price_adjusted: bool,
    buyer_note: Option<String>,
    lines_json: String,
    #[serde(default)]
    fulfillment_batches_json: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<Row> for AgentOrderDto {
    fn from(r: Row) -> Self {
        let lines = serde_json::from_str(&r.lines_json).unwrap_or(serde_json::Value::Null);
        // Migration 0014: silently None if absent or malformed — older rows
        // never carried this field, and fulfillments of non-batch-tracked
        // products skip writing it.
        let fulfillment_batches = r
            .fulfillment_batches_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<AgentOrderFulfillmentLine>>(s).ok());
        Self {
            id: r.id.to_string(),
            peer_did: r.peer_did,
            status: r.status,
            total: r.total,
            currency: r.currency,
            price_adjusted: r.price_adjusted,
            buyer_note: r.buyer_note,
            lines,
            fulfillment_batches,
            created_at: r.created_at,
        }
    }
}

const SELECT_COLS: &str = "id, peer_did, status, <float> total AS total, currency, \
     price_adjusted, buyer_note, lines_json, fulfillment_batches_json, created_at";

pub async fn list(
    db: &Db,
    tenant: &Thing,
    f: AgentOrderFilters,
) -> DomainResult<Vec<AgentOrderDto>> {
    let mut conds = vec!["tenant = $t".to_string()];
    if f.status.is_some() {
        conds.push("status = $s".to_string());
    }
    let limit = f.limit.unwrap_or(100).clamp(1, 500);
    let offset = f.offset.unwrap_or(0).max(0);
    let sql = format!(
        "SELECT {SELECT_COLS} FROM agent_order WHERE {} \
         ORDER BY created_at DESC LIMIT {} START {}",
        conds.join(" AND "),
        limit,
        offset
    );
    let mut qb = db.query(sql).bind(("t", tenant.clone()));
    if let Some(s) = f.status {
        qb = qb.bind(("s", s));
    }
    let rows: Vec<Row> = qb.await?.check()?.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn get(db: &Db, tenant: &Thing, id: &str) -> DomainResult<AgentOrderDto> {
    let oid =
        thing(id).map_err(|_| DomainError::Invalid(format!("agent_order id inválido: {id}")))?;
    if oid.tb != "agent_order" {
        return Err(DomainError::Invalid("id no es un agent_order".into()));
    }
    let mut r = db
        .query(format!(
            "SELECT {SELECT_COLS} FROM agent_order WHERE id = $i AND tenant = $t LIMIT 1"
        ))
        .bind(("i", oid))
        .bind(("t", tenant.clone()))
        .await?
        .check()?;
    let row: Option<Row> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

/// Move an inbound order to `accepted` or `rejected`. Only legal from
/// `received` — re-deciding a settled order is a conflict, not idempotent.
pub async fn decide(
    db: &Db,
    tenant: &Thing,
    id: &str,
    new_status: &str,
) -> DomainResult<AgentOrderDto> {
    if new_status != "accepted" && new_status != "rejected" {
        return Err(DomainError::Invalid(format!(
            "transición inválida: {new_status} (esperaba accepted|rejected)"
        )));
    }
    let current = get(db, tenant, id).await?;
    if current.status != "received" {
        return Err(DomainError::Conflict(format!(
            "orden en estado '{}' no puede pasar a '{new_status}' (solo desde 'received')",
            current.status
        )));
    }
    let oid =
        thing(id).map_err(|_| DomainError::Invalid(format!("agent_order id inválido: {id}")))?;
    db.query("UPDATE agent_order SET status = $st WHERE id = $i AND tenant = $t")
        .bind(("st", new_status.to_string()))
        .bind(("i", oid))
        .bind(("t", tenant.clone()))
        .await?
        .check()?;
    get(db, tenant, id).await
}

/// Fulfill an `accepted` order: decrement the supplier tenant's stock for
/// every catalogued line and append an audit `stock_movement(reason=
/// 'agent_fulfill')`, then flip the order to `fulfilled` — all in one
/// `BEGIN/COMMIT` so a crash can't ship stock without recording it (the
/// `product.stock = SUM(stock_movement.delta)` invariant holds).
///
/// Only legal from `accepted` (a rejected/received/fulfilled order cannot be
/// shipped). Stock is validated up-front: if any line lacks a catalogued
/// product or has insufficient stock the whole fulfillment is refused and the
/// order stays `accepted`.
///
/// FEFO multi-lot split (BACKLOG #2 remainder, migration `0014`): when the
/// product is batch-tracked, every consumed lot is decremented in earliest-
/// expiry order and the per-line breakdown is persisted to
/// `agent_order.fulfillment_batches_json`. Non-batch-tracked products keep
/// the legacy `product.stock`-only path (no breakdown written). Sibling of
/// `order_item.batches_json` on the sales path (migration `0013`).
pub async fn fulfill(db: &Db, tenant: &Thing, id: &str) -> DomainResult<AgentOrderDto> {
    let current = get(db, tenant, id).await?;
    if current.status != "accepted" {
        return Err(DomainError::Conflict(format!(
            "orden en estado '{}' no puede pasar a 'fulfilled' (solo desde 'accepted')",
            current.status
        )));
    }

    let lines = current
        .lines
        .as_array()
        .cloned()
        .ok_or_else(|| DomainError::Invalid("lines_json no es un array".into()))?;

    #[derive(Deserialize)]
    struct Prod {
        id: Thing,
        stock: i64,
    }
    // Resolve + stock-check every catalogued line before any write, and
    // simultaneously build the FEFO plan for batch-tracked products. The
    // FEFO planner itself returns `InsufficientStock` if active non-expired
    // lots can't cover the qty (even when `product.stock` would) — that is
    // the correct semantics for federated fulfillment.
    let mut plan: Vec<(
        Thing,
        i64,
        Option<Vec<crate::inventory::model::FefoAllocation>>,
    )> = Vec::new();
    for l in &lines {
        if l.get("found").and_then(|v| v.as_bool()) == Some(false) {
            continue;
        }
        let barcode = l.get("barcode").and_then(|v| v.as_str()).unwrap_or("");
        let qty = l.get("qty").and_then(|v| v.as_i64()).unwrap_or(0);
        if barcode.is_empty() || qty <= 0 {
            continue;
        }
        let mut rb = db
            .query(
                "SELECT VALUE product FROM product_barcode \
                 WHERE tenant = $t AND barcode = $b LIMIT 1",
            )
            .bind(("t", tenant.clone()))
            .bind(("b", barcode.to_string()))
            .await?
            .check()?;
        let pids: Vec<Thing> = rb.take(0)?;
        let pid = pids.into_iter().next().ok_or_else(|| {
            DomainError::Invalid(format!("producto no catalogado para barcode {barcode}"))
        })?;
        let mut rp = db
            .query(
                "SELECT id, stock FROM product \
                 WHERE id = $p AND tenant = $t AND active = true LIMIT 1",
            )
            .bind(("p", pid))
            .bind(("t", tenant.clone()))
            .await?
            .check()?;
        let prod: Option<Prod> = rp.take(0)?;
        let prod =
            prod.ok_or_else(|| DomainError::Invalid(format!("producto inactivo: {barcode}")))?;
        if prod.stock < qty {
            return Err(DomainError::InsufficientStock);
        }
        let fefo =
            crate::inventory::service::plan_fefo_optional(db, tenant, &prod.id.to_string(), qty)
                .await?;
        plan.push((prod.id, qty, fefo));
    }

    let oid =
        thing(id).map_err(|_| DomainError::Invalid(format!("agent_order id inválido: {id}")))?;
    let mut q = String::from("BEGIN; ");
    for i in 0..plan.len() {
        q.push_str(&format!(
            "UPDATE product SET stock = stock - $q{i} WHERE id = $p{i} AND tenant = $t; \
             CREATE stock_movement SET tenant=$t, product=$p{i}, delta = 0 - $q{i}, \
                reason='agent_fulfill', ref=$ref; ",
        ));
    }
    // Tail: FEFO `product_batch.stock` decrements, grouped after the per-line
    // statements so their indices stay fixed. Mirrors the layout used in
    // `sales::repo::apply_sale`. The `ASSERT >= 0` on `product_batch.stock`
    // is the final oversell guard.
    let mut alloc_idx = 0usize;
    for (_, _, fefo) in plan.iter() {
        if let Some(allocs) = fefo {
            for _ in allocs {
                q.push_str(&format!(
                    "UPDATE product_batch SET stock = stock - $ba{alloc_idx} \
                     WHERE id = $bid{alloc_idx} AND tenant = $t; ",
                ));
                alloc_idx += 1;
            }
        }
    }
    // Build the per-line fulfillment breakdown (skipped when no FEFO plan).
    let breakdown: Vec<crate::agent_orders::model::AgentOrderFulfillmentLine> = plan
        .iter()
        .filter_map(|(pid, _qty, fefo)| {
            fefo.as_ref().map(
                |allocs| crate::agent_orders::model::AgentOrderFulfillmentLine {
                    product: pid.to_string(),
                    allocations: allocs
                        .iter()
                        .map(
                            |a| crate::agent_orders::model::AgentOrderFulfillmentAllocation {
                                batch: a.batch.clone(),
                                qty: a.qty,
                            },
                        )
                        .collect(),
                },
            )
        })
        .collect();
    let fbj_value: surrealdb::sql::Value = if breakdown.is_empty() {
        surrealdb::sql::Value::None
    } else {
        serde_json::to_string(&breakdown)
            .map_err(|e| DomainError::Other(anyhow::anyhow!("fulfillment_batches_json: {e}")))?
            .into()
    };
    q.push_str(
        "UPDATE agent_order SET status='fulfilled', fulfillment_batches_json=$fbj \
         WHERE id=$o AND tenant=$t; COMMIT;",
    );

    let mut qb = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("o", oid))
        .bind(("ref", id.to_string()))
        .bind(("fbj", fbj_value));
    for (i, (pid, qty, _)) in plan.iter().enumerate() {
        qb = qb
            .bind((format!("p{i}"), pid.clone()))
            .bind((format!("q{i}"), *qty));
    }
    // Bind FEFO tail params in the same order they were emitted.
    let mut alloc_idx = 0usize;
    for (_, _, fefo) in plan.iter() {
        if let Some(allocs) = fefo {
            for a in allocs {
                let bid = thing(&a.batch)
                    .map_err(|_| DomainError::Invalid(format!("batch id inválido: {}", a.batch)))?;
                qb = qb
                    .bind((format!("ba{alloc_idx}"), a.qty))
                    .bind((format!("bid{alloc_idx}"), bid));
                alloc_idx += 1;
            }
        }
    }
    qb.await?.check()?;

    get(db, tenant, id).await
}

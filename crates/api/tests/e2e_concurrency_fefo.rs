//! E2E scenario 2 — concurrent POS sales against a single batch-tracked SKU.
//!
//! Fires N parallel `domain::sales::service::post_sale` calls (each 1 unit) at
//! the in-process sale path and asserts the FEFO/stock invariants hold under
//! contention. The `product_batch.stock ASSERT >= 0` (migration 0004) is the
//! oversell guard; `product.stock` itself carries NO assert (migration 0003),
//! so this is the canonical hunt for the SurrealKv write-write race documented
//! in project memory (`crates/dte/src/caf.rs::ASSIGN_LOCK`).
//!
//! Why the domain layer and not HTTP: `POST /api/v1/pos/sale` is role-gated and
//! currently 500s (BUG-001). The race we are probing lives entirely in
//! `post_sale` / `apply_sale`, so driving that fn directly is both faithful and
//! unaffected by BUG-001. The same calls flow through the HTTP route verbatim
//! once BUG-001 is fixed.
//!
//! ## What this run discovered (BUG-003, high)
//! Under heavy contention the SurrealKv MVCC engine detects the write-write
//! conflict on `product_batch.stock` / `product.stock` and ABORTS the losing
//! transactions with a *retryable* error
//! (`Failed to commit transaction due to a read or write conflict`). The sale
//! path does NOT retry and does NOT serialize (no global `tokio::Mutex` like
//! `crates/dte/src/caf.rs::ASSIGN_LOCK`), so ~59/60 concurrent sales fail with
//! `DomainError::Db` → HTTP 500 instead of succeeding or returning a clean
//! `INSUFFICIENT_STOCK`. The two strict tests below assert the CORRECT behavior
//! and are `#[ignore]`d under BUG-003.
//!
//! Worse, the cached `product.stock` counter is itself CORRUPTED under the race
//! (observed 199 after 2 commits from an initial 100) — concurrent
//! `UPDATE product SET stock = stock - 1` statements lose updates / leak partial
//! writes from aborted multi-statement transactions. That is tracked separately
//! as BUG-004.
//!
//! Invariants:
//! * (BUG-003, ignored) exactly `min(stock,N)` succeed, rest InsufficientStock,
//!   none fail with an internal/DB error;
//! * (BUG-004, ignored) `product.stock == initial - committed sales`;
//! * (green) the AUDIT LEDGER stays consistent with the committed sales: one
//!   `stock_movement` per commit, `sum(delta) == -committed`, distinct order ids,
//!   and never more commits than stock allows.

mod e2e_common;

use std::collections::HashSet;
use std::sync::Arc;

use domain::sales::model::PosSaleResponse;
use domain::DomainError;
use e2e_common::*;
use surrealdb::sql::Thing;
use tokio::task::JoinSet;

/// Seed one tenant + one batch-tracked product whose `product.stock` equals
/// the batch stock (`batch_stock`). The product is created with stock 0 because
/// `create_batch` below ATOMICALLY bumps `product.stock` by the batch quantity
/// (`UPDATE product SET stock = stock + $stock`, goods-receipt semantics) — so
/// seeding the product at `batch_stock` too would double-count to `2*N` and
/// the per-sale invariant `product.stock == Σ product_batch.stock` would start
/// out violated. Returns (db, tenant_thing, product_id, TestDb guard).
async fn setup(batch_stock: i64) -> (Arc<db::Db>, Thing, String, TestDb) {
    let tdb = spawn_db().await;
    let (tid, _uid, _roles) = seed_tenant_admin(&tdb.db, "farmacia-conc", "admin@conc.cl").await;
    let tenant = tid_thing(&tid);
    let db = tdb.db.clone();
    let pid = seed_product(&db, &tenant, "Amoxicilina 500", "2500", "1500", 0, None).await;
    let expiry = (chrono::Utc::now() + chrono::Duration::days(365)).to_rfc3339();
    seed_batch(
        &db,
        &tenant,
        &pid,
        "LOTE-CONC-1",
        &expiry,
        batch_stock,
        "1500",
    )
    .await;
    (db, tenant, pid, tdb)
}

/// Fire `n` concurrent single-unit sales of `pid`; return each task's result.
async fn fire_concurrent(
    db: &Arc<db::Db>,
    tenant: &Thing,
    pid: &str,
    n: usize,
) -> Vec<Result<PosSaleResponse, DomainError>> {
    let mut set: JoinSet<Result<PosSaleResponse, DomainError>> = JoinSet::new();
    for _ in 0..n {
        let db = db.clone();
        let tenant = tenant.clone();
        let pid = pid.to_string();
        set.spawn(async move {
            let lines = [SaleLine {
                product: &pid,
                name: "Amoxicilina 500",
                qty: 1,
                unit_price: "2500",
            }];
            seed_sale_raw(&db, &tenant, "pos_cash", Some("2500"), None, &lines).await
        });
    }
    let mut out = Vec::with_capacity(n);
    while let Some(r) = set.join_next().await {
        out.push(r.expect("task join"));
    }
    out
}

/// Like `seed_sale` but returns the `Result` (we WANT to observe failures).
async fn seed_sale_raw(
    db: &db::Db,
    tenant: &Thing,
    payment_method: &str,
    cash: Option<&str>,
    card: Option<&str>,
    lines: &[SaleLine<'_>],
) -> Result<PosSaleResponse, DomainError> {
    use domain::money::Decimal;
    use domain::sales::model::{PosSaleItem, PosSaleRequest};
    let items = lines
        .iter()
        .map(|l| PosSaleItem {
            product: l.product.to_string(),
            product_name: l.name.to_string(),
            quantity: l.qty,
            unit_price: l.unit_price.parse::<Decimal>().unwrap(),
        })
        .collect();
    let req = PosSaleRequest {
        items,
        payment_method: payment_method.to_string(),
        cash_amount: cash.map(|c| c.parse::<Decimal>().unwrap()),
        card_amount: card.map(|c| c.parse::<Decimal>().unwrap()),
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: Vec::new(),
        branch: None,
    };
    domain::sales::service::post_sale(db, tenant, None, None, None, req).await
}

async fn product_stock(db: &db::Db, pid: &str) -> i64 {
    #[derive(serde::Deserialize)]
    struct R {
        stock: i64,
    }
    let pthing = surrealdb::sql::thing(pid).unwrap();
    let mut q = db
        .query("SELECT stock FROM product WHERE id = $p LIMIT 1")
        .bind(("p", pthing))
        .await
        .unwrap();
    let r: Option<R> = q.take(0).unwrap();
    r.unwrap().stock
}

async fn movement_stats(db: &db::Db, pid: &str) -> (i64, i64) {
    #[derive(serde::Deserialize)]
    struct R {
        c: i64,
        s: i64,
    }
    let pthing = surrealdb::sql::thing(pid).unwrap();
    let mut q = db
        .query(
            "SELECT count() AS c, math::sum(delta) AS s FROM stock_movement \
             WHERE product = $p AND reason = 'sale' GROUP ALL",
        )
        .bind(("p", pthing))
        .await
        .unwrap();
    let r: Option<R> = q.take(0).unwrap();
    match r {
        Some(r) => (r.c, r.s),
        None => (0, 0),
    }
}

async fn batch_stock(db: &db::Db, code: &str) -> i64 {
    #[derive(serde::Deserialize)]
    struct B {
        stock: i64,
    }
    let mut q = db
        .query("SELECT stock FROM product_batch WHERE batch_code = $c LIMIT 1")
        .bind(("c", code.to_string()))
        .await
        .unwrap();
    let b: Option<B> = q.take(0).unwrap();
    b.unwrap().stock
}

/// Classify a failure as "expected stock exhaustion" vs "internal/DB error"
/// (the latter is the cryptic-500 equivalent the task asks us to rule out).
fn is_internal(e: &DomainError) -> bool {
    matches!(e, DomainError::Db(_) | DomainError::Other(_))
}

/// GREEN: the LEDGER invariants that still hold under contention even though
/// the cached `product.stock` counter is corrupted by the race (see
/// BUG-003 / BUG-004). For every *committed* sale there is exactly one
/// `stock_movement(reason='sale', delta=-1)` and a distinct order row — the
/// audit trail never invents or drops rows, and no more sales commit than the
/// stock allows. (We deliberately do NOT assert the value of `product.stock`
/// here; that counter diverges under the race — that divergence is BUG-004,
/// asserted in its own ignored test.)
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_sales_ledger_is_consistent_with_committed_sales() {
    let (db, tenant, pid, _tdb) = setup(100).await;

    let results = fire_concurrent(&db, &tenant, &pid, 60).await;
    let oks: Vec<&PosSaleResponse> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    let successes = oks.len() as i64;

    assert!(
        successes <= 100,
        "cannot commit more sales than stock allows"
    );
    assert!(successes >= 1, "at least one sale should commit");

    // No two committed sales share an order id (no double-spend at the order level).
    let order_ids: HashSet<String> = oks.iter().map(|r| r.order.id.clone()).collect();
    assert_eq!(
        order_ids.len(),
        oks.len(),
        "every committed sale has a distinct order"
    );

    // Audit ledger matches the committed sales exactly (no orphan/dup rows).
    let (count, sum) = movement_stats(&db, &pid).await;
    assert_eq!(count, successes, "one 'sale' movement per committed sale");
    assert_eq!(sum, -successes, "sum of sale deltas == -(committed sales)");
}

/// BUG-004 (high): under contention `product.stock` does NOT equal
/// `initial - committed sales`. Observed: 2 sales commit but `product.stock`
/// reads 199 (initial 100) — the concurrent `UPDATE product SET stock=stock-1`
/// statements lose updates / leak partial writes from aborted multi-statement
/// transactions in kv-surrealkv, corrupting the cached counter. Correct
/// behavior asserted here; ignored until the sale path serializes or retries.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_sales_keep_product_stock_consistent() {
    let (db, tenant, pid, _tdb) = setup(100).await;
    let results = fire_concurrent(&db, &tenant, &pid, 60).await;
    let successes = results.iter().filter(|r| r.is_ok()).count() as i64;

    let stock = product_stock(&db, &pid).await;
    assert!(
        stock >= 0,
        "product.stock must never go negative, got {stock}"
    );
    assert_eq!(
        stock,
        100 - successes,
        "product.stock ({stock}) must equal 100 - committed sales ({successes})"
    );
    assert_eq!(
        batch_stock(&db, "LOTE-CONC-1").await,
        100 - successes,
        "batch stock must equal 100 - committed sales"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn sixty_parallel_sales_one_batch_no_oversell() {
    let (db, tenant, pid, _tdb) = setup(100).await;

    let results = fire_concurrent(&db, &tenant, &pid, 60).await;

    let oks: Vec<&PosSaleResponse> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    let internal: Vec<&DomainError> = results
        .iter()
        .filter_map(|r| r.as_ref().err())
        .filter(|e| is_internal(e))
        .collect();

    assert!(
        internal.is_empty(),
        "no sale should fail with an internal/DB error; got {}: {:?}",
        internal.len(),
        internal
    );
    assert_eq!(
        oks.len(),
        60,
        "all 60 sales (stock 100) must succeed; got {}",
        oks.len()
    );

    assert_eq!(
        product_stock(&db, &pid).await,
        40,
        "final product.stock must be 100-60=40"
    );
    let (count, sum) = movement_stats(&db, &pid).await;
    assert_eq!(count, 60, "exactly 60 'sale' stock_movements");
    assert_eq!(sum, -60, "sum of sale deltas must be -60");

    let order_ids: HashSet<String> = oks.iter().map(|r| r.order.id.clone()).collect();
    assert_eq!(
        order_ids.len(),
        60,
        "every successful sale has a distinct order"
    );

    assert_eq!(
        batch_stock(&db, "LOTE-CONC-1").await,
        40,
        "batch stock must be 100-60=40 (no lot double-consumed)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn thirty_parallel_sales_exhaust_stock_ten() {
    let (db, tenant, pid, _tdb) = setup(10).await;

    let results = fire_concurrent(&db, &tenant, &pid, 30).await;

    let ok = results.iter().filter(|r| r.is_ok()).count();
    let insufficient = results
        .iter()
        .filter(|r| matches!(r, Err(DomainError::InsufficientStock)))
        .count();
    let internal = results
        .iter()
        .filter_map(|r| r.as_ref().err())
        .filter(|e| is_internal(e))
        .count();

    assert_eq!(
        internal, 0,
        "stock exhaustion must not surface as internal/DB error"
    );
    assert_eq!(
        ok, 10,
        "exactly 10 of 30 sales succeed (stock=10), got {ok}"
    );
    assert_eq!(
        insufficient, 20,
        "the other 20 fail with InsufficientStock, got {insufficient}"
    );

    assert_eq!(
        product_stock(&db, &pid).await,
        0,
        "stock fully drained to 0"
    );
    let (count, sum) = movement_stats(&db, &pid).await;
    assert_eq!(count, 10, "exactly 10 'sale' movements");
    assert_eq!(sum, -10, "sum of deltas = -10");
    assert_eq!(
        batch_stock(&db, "LOTE-CONC-1").await,
        0,
        "batch drained to 0"
    );
}

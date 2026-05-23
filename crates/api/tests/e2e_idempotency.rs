//! E2E scenario 4 — `Idempotency-Key` semantics on the POS sale path.
//!
//! Driven through `domain::sales::service::post_sale` (the HTTP route
//! `POST /api/v1/pos/sale` is role-gated and currently 500s — BUG-001; the
//! idempotency logic lives entirely in the domain layer + repo). The domain
//! layer keys the cache on `(tenant, key)` only and signals a replay via
//! `DomainError::Conflict("IDEMPOTENCY_CACHED:<json>")`, which the HTTP handler
//! turns into a 200 with the cached payload.
//!
//! Confirmed behaviors:
//! * first call with K1 → real sale + decrement;
//! * replay K1 with the SAME body → IDEMPOTENCY_CACHED replay, NO second decrement;
//! * new key K2 with the same body → fresh sale + real decrement.
//!
//! The same-key/different-body case is a known gap (no body-hash compare) — see
//! the `#[ignore]`d test at the bottom (BUG-002).

mod e2e_common;

use std::sync::Arc;

use domain::money::Decimal;
use domain::sales::model::{PosSaleItem, PosSaleRequest};
use domain::DomainError;
use e2e_common::*;
use surrealdb::sql::Thing;

async fn setup() -> (Arc<db::Db>, Thing, String, TestDb) {
    let tdb = spawn_db().await;
    let (tid, _uid, _roles) = seed_tenant_admin(&tdb.db, "farmacia-idem", "admin@idem.cl").await;
    let tenant = tid_thing(&tid);
    let db = tdb.db.clone();
    let pid = seed_product(&db, &tenant, "Ibuprofeno 400", "1200", "700", 10, None).await;
    (db, tenant, pid, tdb)
}

fn req(pid: &str, qty: i64) -> PosSaleRequest {
    PosSaleRequest {
        items: vec![PosSaleItem {
            product: pid.to_string(),
            product_name: "Ibuprofeno 400".to_string(),
            quantity: qty,
            unit_price: "1200".parse::<Decimal>().unwrap(),
        }],
        payment_method: "pos_cash".to_string(),
        cash_amount: Some("5000".parse::<Decimal>().unwrap()),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: Vec::new(),
    }
}

async fn do_sale(
    db: &db::Db,
    tenant: &Thing,
    pid: &str,
    key: Option<&str>,
    qty: i64,
) -> Result<domain::sales::model::PosSaleResponse, DomainError> {
    domain::sales::service::post_sale(db, tenant, None, None, key, req(pid, qty)).await
}

async fn stock(db: &db::Db, pid: &str) -> i64 {
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

/// Extract the cached order id from the `IDEMPOTENCY_CACHED:<json>` sentinel
/// (this is exactly what the HTTP handler decodes + replays as 200).
fn cached_order_id(e: &DomainError) -> Option<String> {
    if let DomainError::Conflict(msg) = e {
        let json = msg.strip_prefix("IDEMPOTENCY_CACHED:")?;
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        return v["order"]["id"].as_str().map(str::to_string);
    }
    None
}

#[tokio::test]
async fn replay_same_key_same_body_returns_cached_sale() {
    let (db, tenant, pid, _tdb) = setup().await;

    // 1) First sale of 2 units with key K1.
    let r1 = do_sale(&db, &tenant, &pid, Some("K1"), 2)
        .await
        .expect("first sale");
    let order1 = r1.order.id.clone();
    assert_eq!(stock(&db, &pid).await, 8, "stock 10-2=8 after first sale");

    // 2) Replay K1 with the SAME body → IDEMPOTENCY_CACHED sentinel, same order.
    let r2 = do_sale(&db, &tenant, &pid, Some("K1"), 2).await;
    let cached = r2.expect_err("replay must short-circuit via IDEMPOTENCY_CACHED");
    assert_eq!(
        cached_order_id(&cached).as_deref(),
        Some(order1.as_str()),
        "replay returns the same order id (cached payload), got {cached:?}"
    );
    assert_eq!(
        stock(&db, &pid).await,
        8,
        "replay must NOT decrement again (still 8)"
    );

    // 3) A different key with the same body → fresh sale + real decrement.
    let r3 = do_sale(&db, &tenant, &pid, Some("K2"), 2)
        .await
        .expect("new key sale");
    assert_ne!(r3.order.id, order1, "new key yields a different order id");
    assert_eq!(stock(&db, &pid).await, 6, "stock 8-2=6 after K2 sale");
}

#[tokio::test]
async fn no_key_each_sale_is_distinct() {
    let (db, tenant, pid, _tdb) = setup().await;
    let r1 = do_sale(&db, &tenant, &pid, None, 1).await.expect("sale 1");
    let r2 = do_sale(&db, &tenant, &pid, None, 1).await.expect("sale 2");
    assert_ne!(
        r1.order.id, r2.order.id,
        "without an idempotency key, each sale is a new order"
    );
    assert_eq!(
        stock(&db, &pid).await,
        8,
        "two un-keyed sales of 1 → 10-2=8"
    );
}

/// BUG-002: an `Idempotency-Key` reused with a DIFFERENT body should be a 409
/// conflict (canonical "key reuse" semantics — RFC draft + Stripe). Today
/// `lookup_idempotency` matches on `(tenant, key)` only and never compares the
/// request body, so the second call silently REPLAYS the first sale's payload.
/// That hides a real divergence (the caller believes the *new* cart was sold).
/// Correct behavior asserted here; ignored until the cache stores + compares a
/// body hash and the handler maps the mismatch to 409
/// `IDEMPOTENCY_KEY_REUSE_CONFLICT`.
#[tokio::test]
#[ignore = "BUG-002: idempotency cache ignores body; reuse with different body silently replays instead of 409"]
async fn replay_same_key_different_body_should_conflict() {
    let (db, tenant, pid, _tdb) = setup().await;

    let _ = do_sale(&db, &tenant, &pid, Some("K1"), 2)
        .await
        .expect("first sale");
    assert_eq!(stock(&db, &pid).await, 8);

    // Same key, body now sells 5 units instead of 2. CORRECT = a distinct
    // "reuse conflict" error, NOT a silent replay of the 2-unit sale.
    let r2 = do_sale(&db, &tenant, &pid, Some("K1"), 5).await;
    match r2 {
        Err(DomainError::Conflict(msg)) if msg.starts_with("IDEMPOTENCY_CACHED:") => {
            panic!("BUG-002: key reused with a different body silently replayed the cached sale");
        }
        Err(DomainError::Conflict(_)) => { /* a real reuse-conflict — acceptable */ }
        other => panic!("expected a reuse conflict, got {other:?}"),
    }
    assert_eq!(stock(&db, &pid).await, 8, "no extra decrement on conflict");
}

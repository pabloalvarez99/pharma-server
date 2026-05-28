//! E2E scenario 6 — refund accumulation + over-refund guard.
//!
//! A sale of qty=5 is refunded. The domain guard
//! (`sales::service::create_refund`) compares refunded qty per product against
//! what the linked order sold and rejects an over-refund.
//!
//! Driven through `domain::sales::service` (the HTTP routes `POST /pos/sale` +
//! `POST /pos/returns` are role-gated and currently 500 — BUG-001; the guard
//! logic lives in the domain layer).
//!
//! ## What this run discovered (BUG-005, high)
//! The guard only sums the items WITHIN THE CURRENT request against the sold
//! qty; it never looks at prior `devolucion` rows for the same order. So two
//! sequential partial refunds of 3 (total 6 > 5 sold) BOTH succeed — an
//! over-refund / refund-fraud vector. The cross-request test below asserts the
//! correct behavior and is `#[ignore]`d under BUG-005. The single-request guard
//! (refund 6 in one call) DOES work — that is the green test.
//!
//! NOTE on the contract: the (working) single-request rejection is a
//! `DomainError::Invalid` → HTTP 400 `INVALID_INPUT` with a Spanish "excede lo
//! vendido" message (NOT a 409/`OVER_REFUND`). Asserted as-is; the status-code
//! choice is a spec discrepancy noted in the report, not a correctness bug.

mod e2e_common;

use std::sync::Arc;

use domain::money::Decimal;
use domain::sales::model::{NewDevolucion, NewDevolucionItem};
use domain::DomainError;
use e2e_common::*;
use surrealdb::sql::Thing;

async fn setup() -> (Arc<db::Db>, Thing, String, String, TestDb) {
    let tdb = spawn_db().await;
    let (tid, _uid, _roles) = seed_tenant_admin(&tdb.db, "farmacia-ret", "admin@ret.cl").await;
    let tenant = tid_thing(&tid);
    let db = tdb.db.clone();
    let pid = seed_product(&db, &tenant, "Paracetamol 500", "990", "500", 20, None).await;

    // Sell 5 units (stock 20 → 15).
    let lines = [SaleLine {
        product: &pid,
        name: "Paracetamol 500",
        qty: 5,
        unit_price: "990",
    }];
    let sale = seed_sale(
        &db,
        &tenant,
        None,
        "pos_cash",
        Some("5000"),
        None,
        None,
        &lines,
    )
    .await;
    (db, tenant, pid, sale.order.id, tdb)
}

fn refund(order: &str, pid: &str, qty: i64) -> NewDevolucion {
    NewDevolucion {
        order: Some(order.to_string()),
        tipo: "venta".to_string(),
        motivo: "cliente devuelve".to_string(),
        notas: None,
        items: vec![NewDevolucionItem {
            product: Some(pid.to_string()),
            product_name: "Paracetamol 500".to_string(),
            quantity: qty,
            unit_price: "990".parse::<Decimal>().unwrap(),
            restock: true,
        }],
        metodo_reembolso: Some("efectivo".to_string()),
    }
}

async fn do_refund(
    db: &db::Db,
    tenant: &Thing,
    order: &str,
    pid: &str,
    qty: i64,
) -> Result<domain::sales::model::RefundResponse, DomainError> {
    domain::sales::service::create_refund(db, tenant, None, refund(order, pid, qty)).await
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

async fn return_movement_total(db: &db::Db, pid: &str) -> i64 {
    #[derive(serde::Deserialize)]
    struct R {
        s: i64,
    }
    let pthing = surrealdb::sql::thing(pid).unwrap();
    let mut q = db
        .query(
            "SELECT math::sum(delta) AS s FROM stock_movement \
             WHERE product = $p AND reason = 'return' GROUP ALL",
        )
        .bind(("p", pthing))
        .await
        .unwrap();
    let r: Option<R> = q.take(0).unwrap();
    r.map(|r| r.s).unwrap_or(0)
}

fn assert_over_refund(e: &DomainError) {
    match e {
        DomainError::Invalid(msg) => assert!(
            msg.contains("excede lo vendido"),
            "over-refund message must name the excess, got: {msg}"
        ),
        other => {
            panic!("over-refund must be DomainError::Invalid (→400 INVALID_INPUT), got {other:?}")
        }
    }
}

/// GREEN: the WITHIN-REQUEST over-refund guard works. A single refund of 6
/// against a sale of 5 is rejected, and a valid refund of 3 restocks correctly.
#[tokio::test]
async fn single_request_over_refund_is_blocked() {
    let (db, tenant, pid, order, _tdb) = setup().await;
    assert_eq!(stock(&db, &pid).await, 15, "after selling 5 of 20");

    // One request asking for 6 > 5 sold → rejected, stock untouched.
    let e = do_refund(&db, &tenant, &order, &pid, 6)
        .await
        .expect_err("refund of 6 > 5 sold must be rejected in one shot");
    assert_over_refund(&e);
    assert_eq!(
        stock(&db, &pid).await,
        15,
        "rejected refund left stock unchanged"
    );

    // A valid refund of 3 succeeds and restocks (+3 → 18).
    do_refund(&db, &tenant, &order, &pid, 3)
        .await
        .expect("valid refund of 3");
    assert_eq!(stock(&db, &pid).await, 18, "restock +3");
    assert_eq!(
        return_movement_total(&db, &pid).await,
        3,
        "exactly +3 returned via stock_movement reason='return'"
    );
}

/// BUG-005 (high): the over-refund guard ignores PRIOR refunds for the same
/// order. Returning 3 then 3 again (total 6 > 5 sold) BOTH succeed today — an
/// over-refund / refund-fraud vector. Correct behavior asserted; ignored until
/// the guard accumulates already-refunded quantities (sum existing
/// `devolucion_item.quantity` for the order's products).
#[tokio::test]
async fn cross_request_over_refund_should_be_blocked() {
    let (db, tenant, pid, order, _tdb) = setup().await;

    // Return 3 → ok. stock 15+3 = 18.
    do_refund(&db, &tenant, &order, &pid, 3)
        .await
        .expect("first return of 3");
    assert_eq!(stock(&db, &pid).await, 18, "restock +3");

    // Return another 3 (cumulative 6 > 5 sold) → MUST be rejected.
    let e = do_refund(&db, &tenant, &order, &pid, 3)
        .await
        .expect_err("cumulative 3+3 > 5 must be rejected (counting prior refunds)");
    assert_over_refund(&e);
    assert_eq!(
        stock(&db, &pid).await,
        18,
        "rejected refund must not restock again"
    );

    // The ledger must never show more than the 5 actually sellable units returned.
    assert!(
        return_movement_total(&db, &pid).await <= 5,
        "cumulative returns must never exceed the 5 units sold"
    );
}

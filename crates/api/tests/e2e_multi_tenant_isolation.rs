//! E2E scenario 3 — multi-tenant isolation.
//!
//! Two tenants T1, T2 each catalog a "PARACETAMOL-500" SKU at distinct prices
//! and ring up sales. T1's JWT must see ONLY T1 data across every read surface,
//! and must never be able to act on a T2 record. READS are driven over HTTP
//! (tenant comes from the JWT claim); the cross-tenant POS write is checked at
//! the domain layer because `/pos/sale` is role-gated and 500s (BUG-001).
//!
//! Asserted:
//! * `GET /inventory` / `GET /products` return only the caller's tenant rows
//! * `GET /orders` returns only the caller's tenant orders
//! * `GET /products/<id_T2>` with T1's token → 404
//! * a POS sale for T1 referencing a T2 `product_id` → NotFound (never succeeds)
//! * `GET /reports/sales-daily` is tenant-scoped
//! * (BUG-006) the license reload endpoint does NOT bind the license to the
//!   caller's tenant — asserted + `#[ignore]`d.

mod e2e_common;

use std::sync::Arc;

use axum::http::StatusCode;
use domain::DomainError;
use e2e_common::*;
use surrealdb::sql::Thing;

struct Tenants {
    db: Arc<db::Db>,
    app: axum::Router,
    t1: Thing,
    t1_token: String,
    t1_user: Thing,
    p1: String,
    p2: String,
    _tdb: TestDb,
}

async fn setup() -> Tenants {
    let tdb = spawn_db().await;
    let (t1id, u1, r1) = seed_tenant_admin(&tdb.db, "tenant-uno", "admin@uno.cl").await;
    let (t2id, u2, _r2) = seed_tenant_admin(&tdb.db, "tenant-dos", "admin@dos.cl").await;
    let t1 = tid_thing(&t1id);
    let t2 = tid_thing(&t2id);
    let t1_user = surrealdb::sql::thing(&u1).unwrap();
    let t2_user = surrealdb::sql::thing(&u2).unwrap();
    let db = tdb.db.clone();
    let t1_token = token_for(&u1, &t1id, r1);
    let app = api::build_router(state_free(db.clone()));

    // Same code, different prices.
    let p1 = seed_product(&db, &t1, "PARACETAMOL-500", "990", "500", 50, None).await;
    let p2 = seed_product(&db, &t2, "PARACETAMOL-500", "1290", "600", 50, None).await;

    // T1 sells 5, T2 sells 3 (seed via domain — sale route is gated).
    let l1 = [SaleLine {
        product: &p1,
        name: "PARACETAMOL-500",
        qty: 5,
        unit_price: "990",
    }];
    seed_sale(
        &db,
        &t1,
        Some(&t1_user),
        "pos_cash",
        Some("4950"),
        None,
        None,
        &l1,
    )
    .await;
    let l2 = [SaleLine {
        product: &p2,
        name: "PARACETAMOL-500",
        qty: 3,
        unit_price: "1290",
    }];
    seed_sale(
        &db,
        &t2,
        Some(&t2_user),
        "pos_cash",
        Some("3870"),
        None,
        None,
        &l2,
    )
    .await;

    Tenants {
        db,
        app,
        t1,
        t1_token,
        t1_user,
        p1,
        p2,
        _tdb: tdb,
    }
}

#[tokio::test]
async fn t1_inventory_and_products_show_only_t1() {
    let t = setup().await;

    // GET /products → only T1's product.
    let (st, body) = req_json(
        &t.app,
        "GET",
        "/api/v1/products",
        Some(&t.t1_token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let prods = body.as_array().unwrap();
    assert_eq!(prods.len(), 1, "T1 sees exactly its own SKU");
    assert_eq!(prods[0]["id"].as_str().unwrap(), t.p1);
    assert_eq!(
        prods[0]["price"], "990",
        "T1 sees its own price, not T2's 1290"
    );

    // GET /inventory summary → 1 active SKU (T1 only).
    let (st, inv) = req_json(
        &t.app,
        "GET",
        "/api/v1/inventory",
        Some(&t.t1_token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{inv}");
    assert_eq!(inv["total_skus"], 1, "inventory summary scoped to T1");
}

#[tokio::test]
async fn t1_cannot_read_t2_product_by_id() {
    let t = setup().await;
    let (st, body) = req_json(
        &t.app,
        "GET",
        &format!("/api/v1/products/{}", t.p2),
        Some(&t.t1_token),
        None,
        &[],
    )
    .await;
    assert_eq!(
        st,
        StatusCode::NOT_FOUND,
        "T1 fetching a T2 product id must 404, got {st}: {body}"
    );
}

#[tokio::test]
async fn t1_orders_show_only_t1() {
    let t = setup().await;
    let (st, body) = req_json(
        &t.app,
        "GET",
        "/api/v1/orders",
        Some(&t.t1_token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let orders = body.as_array().unwrap();
    assert_eq!(orders.len(), 1, "T1 sees only its own order (not T2's)");
    // Its single line sold 5 units at 990 → total 4950.
    assert_eq!(orders[0]["total"], "4950");
}

#[tokio::test]
async fn t1_sales_daily_is_tenant_scoped() {
    let t = setup().await;
    let (st, body) = req_json(
        &t.app,
        "GET",
        "/api/v1/reports/sales-daily",
        Some(&t.t1_token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let rows = body.as_array().unwrap();
    assert_eq!(rows.len(), 1, "one date bucket (today)");
    // T1 revenue = 4950 (its sale only); T2's 3870 must not leak in.
    assert_eq!(rows[0]["revenue"], "4950", "sales-daily revenue is T1-only");
}

#[tokio::test]
async fn pos_sale_for_t1_with_t2_product_is_not_found() {
    let t = setup().await;
    // Drive at the domain layer (POS HTTP route is gated — BUG-001). T1 tenant
    // + a T2 product id must resolve to NotFound (the pre-check filters by
    // tenant), never a successful cross-tenant sale.
    use domain::money::Decimal;
    use domain::sales::model::{PosSaleItem, PosSaleRequest};
    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: t.p2.clone(), // T2's product, T1's tenant
            product_name: "PARACETAMOL-500".into(),
            quantity: 1,
            unit_price: "990".parse::<Decimal>().unwrap(),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some("990".parse::<Decimal>().unwrap()),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: Vec::new(),
    };
    let res =
        domain::sales::service::post_sale(&t.db, &t.t1, Some(&t.t1_user), None, None, req).await;
    assert!(
        matches!(res, Err(DomainError::NotFound)),
        "a T1 sale referencing a T2 product must be NotFound, got {res:?}"
    );

    // And T2's product/stock is untouched by the rejected attempt.
    #[derive(serde::Deserialize)]
    struct P {
        stock: i64,
    }
    let p2 = surrealdb::sql::thing(&t.p2).unwrap();
    let mut q =
        t.db.query("SELECT stock FROM product WHERE id = $p LIMIT 1")
            .bind(("p", p2))
            .await
            .unwrap();
    let p: Option<P> = q.take(0).unwrap();
    assert_eq!(
        p.unwrap().stock,
        47,
        "T2 stock unchanged (50-3 from its own sale)"
    );
}

/// BUG-006 (medium): `POST /api/v1/admin/license/reload` verifies the on-disk
/// license signature but does NOT check that `license.tenant_id` matches the
/// reloading operator's tenant. A T2-issued license dropped into T1's data dir
/// would be accepted and activated for T1. (We cannot forge a *validly signed*
/// foreign-tenant license without the licenser private key, so this test
/// documents the missing binding via the public reload contract and is
/// `#[ignore]`d; the gap is also visible by inspection of
/// `api::v1::license::reload_license`, which never compares tenants.)
#[tokio::test]
#[ignore = "BUG-006: license reload does not bind license.tenant_id to the caller's tenant"]
async fn license_reload_should_reject_foreign_tenant() {
    // Intentionally unimplemented end-to-end: requires the licenser private key
    // to mint a signed license for a different tenant_id. Kept as a documented
    // expectation so a future signing-capable harness can complete it.
    panic!(
        "BUG-006: reload must reject a license whose tenant_id != caller tenant (binding absent)"
    );
}

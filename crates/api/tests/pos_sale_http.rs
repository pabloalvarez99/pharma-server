//! HTTP smoke for `POST /api/v1/pos/sale` (multi-tenant nube isolation).
//!
//! Pins that the route is role-gated cleanly (admin/cashier → not 500) and
//! that a same-tenant body returns 201.

mod e2e_common;

use axum::http::StatusCode;
use e2e_common::*;

#[tokio::test]
async fn pos_sale_http_same_tenant_returns_201() {
    let tdb = spawn_db().await;
    let (tid, uid, roles) = seed_tenant_admin(&tdb.db, "pos-sale-http", "admin@pos.cl").await;
    let token = token_for(&uid, &tid, roles);
    let tenant = tid_thing(&tid);
    let user = surrealdb::sql::thing(&uid).unwrap();
    let db = tdb.db.clone();
    let app = api::build_router(state_free(db.clone()));

    let pid = seed_product(&db, &tenant, "Tomate", "2000", "1000", 20, None).await;
    // Optional: open caja so sold_by has a session (not required for 201).
    let _ = seed_cash_session(&db, &tenant, &user, "Caja 1", "0").await;

    let body = serde_json::json!({
        "items": [{
            "product": pid,
            "product_name": "Tomate",
            "quantity": 1,
            "unit_price": "2000"
        }],
        "payment_method": "pos_cash",
        "cash_amount": "2000"
    });
    let (st, resp) = req_json(
        &app,
        "POST",
        "/api/v1/pos/sale",
        Some(&token),
        Some(body),
        &[],
    )
    .await;
    assert_eq!(
        st,
        StatusCode::CREATED,
        "same-tenant POS sale must be 201, got {st}: {resp}"
    );
    assert!(
        resp["order"]["id"].as_str().is_some(),
        "response must include order.id: {resp}"
    );
}

#[tokio::test]
async fn pos_sale_http_cross_tenant_product_returns_404() {
    let tdb = spawn_db().await;
    let (t1id, u1, r1) = seed_tenant_admin(&tdb.db, "pos-t1", "a@t1.cl").await;
    let (t2id, _u2, _r2) = seed_tenant_admin(&tdb.db, "pos-t2", "a@t2.cl").await;
    let t1 = tid_thing(&t1id);
    let t2 = tid_thing(&t2id);
    let db = tdb.db.clone();
    let token = token_for(&u1, &t1id, r1);
    let app = api::build_router(state_free(db.clone()));

    let _p1 = seed_product(&db, &t1, "PARACETAMOL-500", "990", "500", 50, None).await;
    let p2 = seed_product(&db, &t2, "PARACETAMOL-500", "1290", "600", 50, None).await;

    let body = serde_json::json!({
        "items": [{
            "product": p2,
            "product_name": "PARACETAMOL-500",
            "quantity": 1,
            "unit_price": "990"
        }],
        "payment_method": "pos_cash",
        "cash_amount": "990"
    });
    let (st, resp) = req_json(
        &app,
        "POST",
        "/api/v1/pos/sale",
        Some(&token),
        Some(body),
        &[],
    )
    .await;
    assert!(
        st == StatusCode::NOT_FOUND || st == StatusCode::UNPROCESSABLE_ENTITY,
        "T1 selling T2 product must be 404/422, never 201/500; got {st}: {resp}"
    );
    assert_ne!(st, StatusCode::CREATED);
    assert_ne!(st, StatusCode::INTERNAL_SERVER_ERROR);
}

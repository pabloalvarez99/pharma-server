//! E2E scenario 5 — federated purchase-order roundtrip between two nodes.
//!
//! Buyer node A (an `agent::Identity`, DID_A) places a signed `po.create` on
//! supplier node B (a full in-process app with its own DID_B + DB). B records
//! an `agent_order`; B's human operator accepts then fulfills it (decrementing
//! B's stock + writing an `agent_fulfill` audit movement); A polls `po.status`
//! and sees the lifecycle `received → accepted → fulfilled`.
//!
//! The federation transport (`POST /agent/inbox`) is signature-authenticated,
//! NOT JWT/role-gated, so po.create / po.status run over HTTP. The operator
//! transitions (`/api/v1/agent-orders/<id>/accept|fulfill`) ARE role-gated and
//! 500 today (BUG-001), so they are driven via `domain::agent_orders::service`.
//!
//! Bug-hunt:
//! * po.create naming a tenant that did NOT opt into federation → 403;
//! * a tampered (invalid-signature) envelope → 401.

mod e2e_common;

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use e2e_common::*;
use http_body_util::BodyExt;
use surrealdb::sql::Thing;
use tower::ServiceExt;

/// Build supplier node B: full app + node identity (returns its DID), a
/// federation-enabled tenant with one catalogued+barcoded product.
async fn supplier_node() -> (axum::Router, String, Arc<db::Db>, Thing, TestDb) {
    let tdb = spawn_db().await;
    let db = tdb.db.clone();
    let node = agent::Identity::generate();
    let did = node.did();
    let state = api::AppState {
        started_at: chrono::Utc::now(),
        jwt: jwt_cfg(),
        db: Some(db.clone()),
        metrics_token: None,
        node_identity: Some(Arc::new(node)),
        data_dir: None,
        license: Arc::new(arc_swap::ArcSwap::from_pointee(
            license::License::free_default(uuid::Uuid::nil()),
        )),
        license_path: None,
        rate_limit: None,
        docs_enabled: true,
        public_catalog: pharma_core::config::PublicCatalogConfig::default(),
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
        stock_webhook: Arc::new(api::stock_webhook::StockWebhookConfig::default()),
        provisioning_key: None,
    };
    let app = api::build_router(state);

    // Supplier tenant + federation opt-in + a catalogued, barcoded product.
    let (tid, _u, _r) = seed_tenant_admin(&db, "drogueria-b", "op@b.cl").await;
    let tenant = tid_thing(&tid);
    db.query("CREATE admin_setting SET tenant=$t, key='federation_enabled', value='true'")
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let pid = seed_product(&db, &tenant, "Amoxicilina 500", "1490", "900", 100, None).await;
    db.query("CREATE product_barcode SET tenant=$t, product=$p, barcode='7800000000017'")
        .bind(("t", tenant.clone()))
        .bind(("p", surrealdb::sql::thing(&pid).unwrap()))
        .await
        .unwrap();

    (app, did, db, tenant, tdb)
}

async fn post_inbox(app: &axum::Router, body: String) -> (StatusCode, String) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/agent/inbox")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn full_po_lifecycle_received_accepted_fulfilled() {
    let (app, node_did, db, tenant, _tdb) = supplier_node().await;
    let buyer = agent::Identity::generate();

    // 1) Buyer A → B: signed po.create.
    let create = agent::Envelope::create(
        &buyer,
        node_did.clone(),
        "po-rt-1",
        "po.create",
        serde_json::json!({
            "tenant": "drogueria-b",
            "lines": [{ "barcode": "7800000000017", "qty": 10, "unit_price": 1490 }],
            "buyer_note": "pedido de prueba",
        }),
    )
    .expect("po.create envelope");
    let (st, body) = post_inbox(&app, create.to_json().unwrap()).await;
    assert_eq!(st, StatusCode::OK, "po.create must be 200: {body}");
    let ack = agent::Envelope::from_json(&body).unwrap();
    assert_eq!(ack.topic, "po.ack");
    ack.verify().expect("B-signed ack verifies");
    assert_eq!(ack.body["status"], "received");
    assert_eq!(ack.body["total"], 14900.0, "10 × canonical 1490");
    let order_id = ack.body["order_id"].as_str().unwrap().to_string();
    assert!(order_id.starts_with("agent_order:"));

    // The order is persisted in B, status=received, carrying the buyer DID.
    #[derive(serde::Deserialize)]
    struct O {
        peer_did: String,
        status: String,
    }
    let mut q = db
        .query("SELECT peer_did, status FROM agent_order WHERE tenant=$t")
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let rows: Vec<O> = q.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].peer_did, buyer.did());
    assert_eq!(rows[0].status, "received");

    // 2) Operator B accepts (domain — the HTTP route is role-gated, BUG-001).
    let accepted = domain::agent_orders::service::decide(&db, &tenant, &order_id, "accepted")
        .await
        .expect("operator accept");
    assert_eq!(accepted.status, "accepted");

    // 3) Buyer A polls status → accepted.
    let q1 = agent::Envelope::create(
        &buyer,
        node_did.clone(),
        "po-rt-2",
        "po.status",
        serde_json::json!({ "order_id": order_id }),
    )
    .expect("po.status q1 envelope");
    let (st, body) = post_inbox(&app, q1.to_json().unwrap()).await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let r = agent::Envelope::from_json(&body).unwrap();
    assert_eq!(r.topic, "po.status.result");
    assert_eq!(r.body["status"], "accepted");

    // 4) Operator B fulfills → stock decrements + audit movement.
    let fulfilled = domain::agent_orders::service::fulfill(&db, &tenant, &order_id)
        .await
        .expect("operator fulfill");
    assert_eq!(fulfilled.status, "fulfilled");

    #[derive(serde::Deserialize)]
    struct P {
        stock: i64,
    }
    let mut q = db
        .query("SELECT stock FROM product WHERE tenant=$t LIMIT 1")
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let p: Option<P> = q.take(0).unwrap();
    assert_eq!(p.unwrap().stock, 90, "B stock 100-10=90 after fulfillment");

    #[derive(serde::Deserialize)]
    struct M {
        c: i64,
        s: i64,
    }
    let mut q = db
        .query(
            "SELECT count() AS c, math::sum(delta) AS s FROM stock_movement \
             WHERE tenant=$t AND reason='agent_fulfill' GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let m: Option<M> = q.take(0).unwrap();
    let m = m.expect("agent_fulfill movement");
    assert_eq!(m.c, 1, "one agent_fulfill movement");
    assert_eq!(m.s, -10, "delta -10 (audit trail)");

    // 5) Buyer A polls again → fulfilled.
    let q2 = agent::Envelope::create(
        &buyer,
        node_did,
        "po-rt-3",
        "po.status",
        serde_json::json!({ "order_id": order_id }),
    )
    .expect("po.status q2 envelope");
    let (st, body) = post_inbox(&app, q2.to_json().unwrap()).await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let r = agent::Envelope::from_json(&body).unwrap();
    assert_eq!(r.body["status"], "fulfilled", "buyer sees the final state");
    assert_eq!(r.body["currency"], "CLP");
}

#[tokio::test]
async fn po_create_for_non_federated_tenant_is_forbidden() {
    let (app, node_did, db, _tenant, _tdb) = supplier_node().await;

    // A second tenant on the same node that did NOT enable federation.
    let (_t2, _u2, _r2) = seed_tenant_admin(&db, "privada-b", "op2@b.cl").await;

    let buyer = agent::Identity::generate();
    let env = agent::Envelope::create(
        &buyer,
        node_did,
        "po-priv",
        "po.create",
        serde_json::json!({
            "tenant": "privada-b",
            "lines": [{ "barcode": "7800000000017", "qty": 1, "unit_price": 1 }],
        }),
    )
    .expect("po.create privada-b envelope");
    let (st, body) = post_inbox(&app, env.to_json().unwrap()).await;
    assert_eq!(
        st,
        StatusCode::FORBIDDEN,
        "po.create against a non-federated tenant must be 403, got {st}: {body}"
    );
}

#[tokio::test]
async fn tampered_envelope_is_rejected_401() {
    let (app, node_did, _db, _tenant, _tdb) = supplier_node().await;
    let buyer = agent::Identity::generate();
    let mut env = agent::Envelope::create(
        &buyer,
        node_did,
        "po-evil",
        "po.create",
        serde_json::json!({ "tenant": "drogueria-b", "lines": [] }),
    )
    .expect("po-evil envelope");
    // Mutate the body after signing → signature no longer matches.
    env.body =
        serde_json::json!({ "tenant": "drogueria-b", "lines": [{ "barcode": "x", "qty": 99 }] });
    let (st, _body) = post_inbox(&app, env.to_json().unwrap()).await;
    assert_eq!(
        st,
        StatusCode::UNAUTHORIZED,
        "an envelope tampered after signing must be 401"
    );
}

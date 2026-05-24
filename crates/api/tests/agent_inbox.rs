//! Agent federation transport integration tests â€” exercises POST /agent/inbox
//! end-to-end: a peer identity signs an Envelope, the node verifies it,
//! dispatches the topic, and replies with its own signed Envelope. Confirms
//! the cross-node trust handshake works without JWT/tenant context.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use pharma_core::config::{DbConfig, JwtConfig};
use tempfile::TempDir;
use tower::ServiceExt;

const MIGRATIONS_DIR: &str = "../../migrations";

struct TestDb {
    db: Arc<db::Db>,
    _dir: TempDir,
}

async fn spawn_test_db() -> TestDb {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = DbConfig {
        path: dir.path().to_string_lossy().into_owned(),
        namespace: "pharma_test".into(),
        database: "main".into(),
    };
    let handle = db::connect(&cfg).await.expect("db connect");
    db::run_migrations(&handle, MIGRATIONS_DIR)
        .await
        .expect("migrations");
    TestDb {
        db: Arc::new(handle),
        _dir: dir,
    }
}

fn node_state(db: Arc<db::Db>) -> (api::AppState, String) {
    let node = agent::Identity::generate();
    let did = node.did();
    let state = api::AppState {
        started_at: chrono::Utc::now(),
        jwt: JwtConfig {
            secret: "x".into(),
            issuer: "x".into(),
            ttl_seconds: 60,
        },
        db: Some(db),
        metrics_token: None,
        node_identity: Some(Arc::new(node)),
        data_dir: None,
        license: Arc::new(arc_swap::ArcSwap::from_pointee(
            license::License::free_default(uuid::Uuid::nil()),
        )),
        license_path: None,
        rate_limit: None,
        public_catalog: pharma_core::config::PublicCatalogConfig::default(),
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
    };
    (state, did)
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
async fn ping_returns_signed_pong() {
    let tdb = spawn_test_db().await;
    let (state, node_did) = node_state(tdb.db.clone());
    let app = api::build_router(state);

    let peer = agent::Identity::generate();
    let env = agent::Envelope::create(
        &peer,
        node_did.clone(),
        "msg-ping-1",
        "ping",
        serde_json::json!({}),
    );
    let (status, body) = post_inbox(&app, env.to_json().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "body={body}");

    let reply = agent::Envelope::from_json(&body).expect("reply is an envelope");
    assert_eq!(reply.topic, "pong");
    assert_eq!(reply.from, node_did);
    assert_eq!(reply.to, peer.did());
    assert_eq!(reply.body["echo"], "msg-ping-1");
    reply.verify().expect("node-signed reply verifies");
}

#[tokio::test]
async fn tampered_envelope_is_rejected_401() {
    let tdb = spawn_test_db().await;
    let (state, node_did) = node_state(tdb.db.clone());
    let app = api::build_router(state);

    let peer = agent::Identity::generate();
    let mut env =
        agent::Envelope::create(&peer, node_did, "m", "ping", serde_json::json!({ "x": 1 }));
    // Tamper after signing.
    env.body = serde_json::json!({ "x": 999 });
    let (status, _) = post_inbox(&app, env.to_json().unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn catalog_lookup_matches_global_barcode_catalog() {
    let tdb = spawn_test_db().await;
    // Seed the GLOBAL (no-tenant) barcode_catalog.
    tdb.db
        .query(
            "CREATE barcode_catalog SET barcode='7800001112223', external_id='LAB-AAA'; \
             CREATE barcode_catalog SET barcode='7800009998887', external_id='LAB-BBB';",
        )
        .await
        .expect("seed catalog");

    let (state, node_did) = node_state(tdb.db.clone());
    let app = api::build_router(state);

    let peer = agent::Identity::generate();
    let env = agent::Envelope::create(
        &peer,
        node_did.clone(),
        "lookup-1",
        "catalog.lookup",
        serde_json::json!({ "barcodes": ["7800001112223", "0000000000000"] }),
    );
    let (status, body) = post_inbox(&app, env.to_json().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "body={body}");

    let reply = agent::Envelope::from_json(&body).unwrap();
    assert_eq!(reply.topic, "catalog.match");
    reply.verify().expect("signed reply");
    let matches = reply.body["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1, "only the known barcode matches");
    assert_eq!(matches[0]["barcode"], "7800001112223");
    assert_eq!(matches[0]["external_id"], "LAB-AAA");

    // Interaction recorded in the local trust graph.
    let mut r = tdb
        .db
        .query("SELECT count() FROM agent_interaction WHERE topic='catalog.lookup' GROUP ALL")
        .await
        .unwrap();
    let c: Option<serde_json::Value> = r.take(0).unwrap();
    assert!(c.is_some(), "agent_interaction row written");
}

async fn seed_supplier_tenant(db: &db::Db, slug: &str, federation: bool) -> String {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let mut t = db
        .query("CREATE tenant SET name=$n, slug=$s RETURN AFTER")
        .bind(("n", format!("Sup {slug}")))
        .bind(("s", slug.to_string()))
        .await
        .unwrap();
    let tid = t.take::<Option<Row>>(0).unwrap().unwrap().id;
    if federation {
        db.query("CREATE admin_setting SET tenant=$t, key='federation_enabled', value='true'")
            .bind(("t", tid.clone()))
            .await
            .unwrap();
    }
    // Product + barcode mapping.
    let mut p = db
        .query(
            "CREATE product SET tenant=$t, name='Paracetamol 500', slug='para-500', \
             price=1490, stock=120, active=true RETURN AFTER",
        )
        .bind(("t", tid.clone()))
        .await
        .unwrap();
    let pid = p.take::<Option<Row>>(0).unwrap().unwrap().id;
    db.query("CREATE product_barcode SET tenant=$t, product=$p, barcode='7801234567890'")
        .bind(("t", tid.clone()))
        .bind(("p", pid))
        .await
        .unwrap();
    tid.to_string()
}

#[tokio::test]
async fn quote_request_returns_priced_lines_when_federation_enabled() {
    let tdb = spawn_test_db().await;
    seed_supplier_tenant(&tdb.db, "drogueria-x", true).await;
    let (state, node_did) = node_state(tdb.db.clone());
    let app = api::build_router(state);

    let peer = agent::Identity::generate();
    let env = agent::Envelope::create(
        &peer,
        node_did,
        "q1",
        "quote.request",
        serde_json::json!({
            "tenant": "drogueria-x",
            "items": [{ "barcode": "7801234567890", "qty": 10 }]
        }),
    );
    let (status, body) = post_inbox(&app, env.to_json().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    let reply = agent::Envelope::from_json(&body).unwrap();
    assert_eq!(reply.topic, "quote.response");
    reply.verify().expect("signed");
    let lines = reply.body["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["product_name"], "Paracetamol 500");
    assert_eq!(lines[0]["unit_price"], 1490.0);
    assert_eq!(lines[0]["qty"], 10);
    assert_eq!(lines[0]["in_stock"], true);
    assert_eq!(reply.body["total"], 14900.0);
}

#[tokio::test]
async fn quote_request_blocked_when_federation_disabled() {
    let tdb = spawn_test_db().await;
    seed_supplier_tenant(&tdb.db, "private-co", false).await;
    let (state, node_did) = node_state(tdb.db.clone());
    let app = api::build_router(state);

    let peer = agent::Identity::generate();
    let env = agent::Envelope::create(
        &peer,
        node_did,
        "q2",
        "quote.request",
        serde_json::json!({
            "tenant": "private-co",
            "items": [{ "barcode": "7801234567890", "qty": 1 }]
        }),
    );
    let (status, _) = post_inbox(&app, env.to_json().unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "opt-out tenant must reject");
}

#[tokio::test]
async fn po_create_records_order_and_acks() {
    let tdb = spawn_test_db().await;
    seed_supplier_tenant(&tdb.db, "drogueria-y", true).await;
    let (state, node_did) = node_state(tdb.db.clone());
    let app = api::build_router(state);

    let peer = agent::Identity::generate();
    let env = agent::Envelope::create(
        &peer,
        node_did,
        "po1",
        "po.create",
        serde_json::json!({
            "tenant": "drogueria-y",
            "lines": [{ "barcode": "7801234567890", "qty": 24, "unit_price": 1490 }],
            "buyer_note": "urgente fin de semana"
        }),
    );
    let (status, body) = post_inbox(&app, env.to_json().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    let reply = agent::Envelope::from_json(&body).unwrap();
    assert_eq!(reply.topic, "po.ack");
    reply.verify().expect("signed");
    assert_eq!(reply.body["status"], "received");
    assert_eq!(reply.body["total"], 35760.0);
    assert_eq!(
        reply.body["price_adjusted"], false,
        "buyer sent canonical price â†’ no adjustment"
    );
    let l0 = &reply.body["lines"][0];
    assert_eq!(l0["unit_price_canonical"], 1490.0);
    assert_eq!(l0["unit_price_sent"], 1490.0);
    assert!(reply.body["order_id"]
        .as_str()
        .unwrap()
        .starts_with("agent_order:"));

    // Order persisted, tenant-scoped, carrying buyer DID.
    let mut r = tdb
        .db
        .query("SELECT peer_did, status, total FROM agent_order")
        .await
        .unwrap();
    #[derive(serde::Deserialize)]
    struct O {
        peer_did: String,
        status: String,
    }
    let rows: Vec<O> = r.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].peer_did, peer.did());
    assert_eq!(rows[0].status, "received");
}

#[tokio::test]
async fn po_create_rejects_buyer_supplied_price_and_uses_canonical() {
    // SECURITY: a malicious peer sends unit_price=1 for a product the supplier
    // catalogs at 1490. The supplier MUST re-quote against its own catalog;
    // the persisted total + ack must reflect 1490, and price_adjusted=true.
    let tdb = spawn_test_db().await;
    seed_supplier_tenant(&tdb.db, "drogueria-z", true).await;
    let (state, node_did) = node_state(tdb.db.clone());
    let app = api::build_router(state);

    let peer = agent::Identity::generate();
    let env = agent::Envelope::create(
        &peer,
        node_did,
        "po-evil",
        "po.create",
        serde_json::json!({
            "tenant": "drogueria-z",
            "lines": [{ "barcode": "7801234567890", "qty": 10, "unit_price": 1 }]
        }),
    );
    let (status, body) = post_inbox(&app, env.to_json().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    let reply = agent::Envelope::from_json(&body).unwrap();
    assert_eq!(reply.topic, "po.ack");
    reply.verify().expect("signed");
    assert_eq!(
        reply.body["total"], 14900.0,
        "supplier must price with canonical 1490, not buyer-supplied 1"
    );
    assert_eq!(
        reply.body["price_adjusted"], true,
        "buyer price diverged from canonical â†’ flag must be set"
    );
    let l0 = &reply.body["lines"][0];
    assert_eq!(l0["unit_price_canonical"], 1490.0);
    assert_eq!(l0["unit_price_sent"], 1.0);

    // Persisted agent_order total = canonical, not buyer-supplied.
    #[derive(serde::Deserialize)]
    struct O {
        total: f64,
    }
    let mut r = tdb
        .db
        .query("SELECT <float> total AS total FROM agent_order")
        .await
        .unwrap();
    let rows: Vec<O> = r.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert!((rows[0].total - 14900.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn po_create_marks_adjusted_when_product_unknown() {
    let tdb = spawn_test_db().await;
    seed_supplier_tenant(&tdb.db, "drogueria-w", true).await;
    let (state, node_did) = node_state(tdb.db.clone());
    let app = api::build_router(state);

    let peer = agent::Identity::generate();
    let env = agent::Envelope::create(
        &peer,
        node_did,
        "po-unknown",
        "po.create",
        serde_json::json!({
            "tenant": "drogueria-w",
            "lines": [{ "barcode": "0000000000000", "qty": 5, "unit_price": 100 }]
        }),
    );
    let (status, body) = post_inbox(&app, env.to_json().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    let reply = agent::Envelope::from_json(&body).unwrap();
    assert_eq!(reply.body["price_adjusted"], true);
    assert_eq!(
        reply.body["total"], 0.0,
        "unknown product contributes 0 to total"
    );
    assert_eq!(reply.body["lines"][0]["found"], false);
}

#[tokio::test]
async fn po_status_returns_persisted_state_scoped_to_buyer_did() {
    let tdb = spawn_test_db().await;
    seed_supplier_tenant(&tdb.db, "drogueria-s", true).await;
    let (state, node_did) = node_state(tdb.db.clone());
    let app = api::build_router(state);

    // Buyer places an order with a bad price â†’ supplier reprices.
    let buyer = agent::Identity::generate();
    let create = agent::Envelope::create(
        &buyer,
        node_did.clone(),
        "po-s1",
        "po.create",
        serde_json::json!({
            "tenant": "drogueria-s",
            "lines": [{ "barcode": "7801234567890", "qty": 3, "unit_price": 1 }]
        }),
    );
    let (status, body) = post_inbox(&app, create.to_json().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    let ack = agent::Envelope::from_json(&body).unwrap();
    let order_id = ack.body["order_id"].as_str().unwrap().to_string();
    assert_eq!(ack.body["price_adjusted"], true);

    // Same buyer polls status: durable price_adjusted + canonical total.
    let q = agent::Envelope::create(
        &buyer,
        node_did.clone(),
        "po-s2",
        "po.status",
        serde_json::json!({ "order_id": order_id }),
    );
    let (status, body) = post_inbox(&app, q.to_json().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "body={body}");
    let reply = agent::Envelope::from_json(&body).unwrap();
    assert_eq!(reply.topic, "po.status.result");
    reply.verify().expect("signed");
    assert_eq!(reply.body["status"], "received");
    assert_eq!(reply.body["price_adjusted"], true);
    assert_eq!(reply.body["total"], 4470.0); // 3 Ã— canonical 1490
    assert_eq!(reply.body["currency"], "CLP");

    // A different peer cannot read this order (DID-scoped).
    let intruder = agent::Identity::generate();
    let evil = agent::Envelope::create(
        &intruder,
        node_did,
        "po-s3",
        "po.status",
        serde_json::json!({ "order_id": order_id }),
    );
    let (status, _) = post_inbox(&app, evil.to_json().unwrap()).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "another buyer's DID must not resolve the order"
    );
}

#[tokio::test]
async fn po_status_unknown_order_is_not_found() {
    let tdb = spawn_test_db().await;
    let (state, node_did) = node_state(tdb.db.clone());
    let app = api::build_router(state);

    let peer = agent::Identity::generate();
    let env = agent::Envelope::create(
        &peer,
        node_did,
        "po-s4",
        "po.status",
        serde_json::json!({ "order_id": "agent_order:doesnotexist" }),
    );
    let (status, _) = post_inbox(&app, env.to_json().unwrap()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_topic_rejected_400() {
    let tdb = spawn_test_db().await;
    let (state, node_did) = node_state(tdb.db.clone());
    let app = api::build_router(state);

    let peer = agent::Identity::generate();
    let env = agent::Envelope::create(
        &peer,
        node_did,
        "m",
        "totally.unknown",
        serde_json::json!({}),
    );
    let (status, _) = post_inbox(&app, env.to_json().unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

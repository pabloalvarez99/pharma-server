//! Agent federation transport integration tests — exercises POST /agent/inbox
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

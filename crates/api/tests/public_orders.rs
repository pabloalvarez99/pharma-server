//! Integration tests for `POST /api/v1/public/orders/web` (ADR-0012 pattern 2).
//!
//! Covers the HMAC auth surface end-to-end: valid signed ingest, idempotent
//! replay, wrong/missing signature, stale timestamp, disabled config (404),
//! unknown tenant (404), and the 64 KiB body cap (413).

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use hmac::{Hmac, Mac};
use http_body_util::BodyExt;
use pharma_core::config::{DbConfig, JwtConfig, PublicOrdersConfig};
use sha2::Sha256;
use tempfile::TempDir;
use tower::ServiceExt;

type HmacSha256 = Hmac<Sha256>;

const MIGRATIONS_DIR: &str = "../../migrations";
const SECRET: &str = "test-hmac-secret-do-not-ship";

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    }
}

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
        .expect("run migrations");
    TestDb {
        db: Arc::new(handle),
        _dir: dir,
    }
}

async fn seed_tenant(db: &db::Db, slug: &str) {
    db.query("CREATE tenant SET name = $name, slug = $slug")
        .bind(("name", format!("Tenant {slug}")))
        .bind(("slug", slug.to_string()))
        .await
        .expect("create tenant");
}

/// Seed an active product with the given commercial `external_id` so the
/// domain ingest can resolve the order line.
async fn seed_product(db: &db::Db, tenant_slug: &str, external_id: &str) {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let mut q = db
        .query("SELECT id FROM tenant WHERE slug = $slug LIMIT 1")
        .bind(("slug", tenant_slug.to_string()))
        .await
        .expect("find tenant");
    let tenant: Option<Row> = q.take(0).expect("decode tenant");
    let tenant_id = tenant.expect("tenant row").id;

    db.query(
        "CREATE product SET tenant = $t, name = $name, slug = $pslug, \
            price = 1000, external_id = $ext, active = true",
    )
    .bind(("t", tenant_id))
    .bind(("name", format!("Producto {external_id}")))
    .bind(("pslug", format!("prod-{external_id}")))
    .bind(("ext", external_id.to_string()))
    .await
    .expect("create product");
}

fn state(db: Arc<db::Db>, enabled: bool, secret: &str) -> api::AppState {
    api::AppState {
        started_at: chrono::Utc::now(),
        jwt: jwt_cfg(),
        db: Some(db),
        metrics_token: None,
        node_identity: None,
        data_dir: None,
        license: Arc::new(arc_swap::ArcSwap::from_pointee(
            license::License::free_default(uuid::Uuid::nil()),
        )),
        license_path: None,
        rate_limit: None,
        docs_enabled: true,
        public_catalog: pharma_core::config::PublicCatalogConfig::default(),
        public_orders: PublicOrdersConfig {
            enabled,
            hmac_secret: secret.into(),
            require_api_key: false,
        },
        stock_webhook: std::sync::Arc::new(pharma_core::config::StockWebhookConfig::default()),
        provisioning_key: None,
    }
}

fn sign(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

fn order_body(external_order_id: &str, sku: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "external_order_id": external_order_id,
        "customer": { "name": "Juan Pérez", "phone": "+56912345678" },
        "items": [ { "external_id": sku, "quantity": 2, "unit_price": "1000" } ],
        "payment_method": "webpay",
        "total": "2000"
    }))
    .unwrap()
}

/// Build a fully-signed request with a fresh timestamp.
fn signed_request(tenant: &str, secret: &str, body: Vec<u8>) -> Request<Body> {
    let sig = sign(secret, &body);
    Request::builder()
        .method("POST")
        .uri("/api/v1/public/orders/web")
        .header("content-type", "application/json")
        .header("x-pharma-signature", sig)
        .header("x-pharma-timestamp", chrono::Utc::now().to_rfc3339())
        .header("x-pharma-tenant", tenant)
        .body(Body::from(body))
        .unwrap()
}

async fn json_body(res: axum::response::Response) -> serde_json::Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

#[tokio::test]
async fn valid_signed_order_is_created() {
    let t = spawn_test_db().await;
    seed_tenant(&t.db, "acme").await;
    seed_product(&t.db, "acme", "SKU-1").await;
    let app = api::build_router(state(t.db.clone(), true, SECRET));

    let res = app
        .oneshot(signed_request("acme", SECRET, order_body("ord-1", "SKU-1")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let json = json_body(res).await;
    assert_eq!(json["status"], "created");
    assert_eq!(json["external_order_id"], "ord-1");
    assert!(json["order_id"].as_str().unwrap().starts_with("order:"));
}

#[tokio::test]
async fn replay_same_external_id_is_idempotent() {
    let t = spawn_test_db().await;
    seed_tenant(&t.db, "acme").await;
    seed_product(&t.db, "acme", "SKU-1").await;
    let app = api::build_router(state(t.db.clone(), true, SECRET));

    let first = app
        .clone()
        .oneshot(signed_request("acme", SECRET, order_body("dup-1", "SKU-1")))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_id = json_body(first).await["order_id"]
        .as_str()
        .unwrap()
        .to_string();

    let second = app
        .oneshot(signed_request("acme", SECRET, order_body("dup-1", "SKU-1")))
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let json = json_body(second).await;
    assert_eq!(json["status"], "idempotent_replay");
    assert_eq!(json["order_id"].as_str().unwrap(), first_id);
}

#[tokio::test]
async fn wrong_signature_is_rejected() {
    let t = spawn_test_db().await;
    seed_tenant(&t.db, "acme").await;
    seed_product(&t.db, "acme", "SKU-1").await;
    let app = api::build_router(state(t.db.clone(), true, SECRET));

    // Sign with a different secret than the server holds.
    let res = app
        .oneshot(signed_request(
            "acme",
            "attacker-secret",
            order_body("ord-2", "SKU-1"),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(res).await["error"]["code"], "SIGNATURE_INVALID");
}

#[tokio::test]
async fn missing_signature_is_rejected() {
    let t = spawn_test_db().await;
    seed_tenant(&t.db, "acme").await;
    let app = api::build_router(state(t.db.clone(), true, SECRET));

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/public/orders/web")
        .header("content-type", "application/json")
        .header("x-pharma-timestamp", chrono::Utc::now().to_rfc3339())
        .header("x-pharma-tenant", "acme")
        .body(Body::from(order_body("ord-3", "SKU-1")))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(json_body(res).await["error"]["code"], "SIGNATURE_MISSING");
}

#[tokio::test]
async fn stale_timestamp_is_rejected() {
    let t = spawn_test_db().await;
    seed_tenant(&t.db, "acme").await;
    seed_product(&t.db, "acme", "SKU-1").await;
    let app = api::build_router(state(t.db.clone(), true, SECRET));

    let body = order_body("ord-4", "SKU-1");
    let sig = sign(SECRET, &body);
    let stale = (chrono::Utc::now() - chrono::Duration::seconds(600)).to_rfc3339();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/public/orders/web")
        .header("content-type", "application/json")
        .header("x-pharma-signature", sig)
        .header("x-pharma-timestamp", stale)
        .header("x-pharma-tenant", "acme")
        .body(Body::from(body))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        json_body(res).await["error"]["code"],
        "SIGNATURE_REPLAY_REJECTED"
    );
}

#[tokio::test]
async fn disabled_config_returns_404() {
    let t = spawn_test_db().await;
    seed_tenant(&t.db, "acme").await;
    // enabled = false → uniform 404 even with a perfectly valid signature.
    let app = api::build_router(state(t.db.clone(), false, SECRET));

    let res = app
        .oneshot(signed_request("acme", SECRET, order_body("ord-5", "SKU-1")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_tenant_returns_404() {
    let t = spawn_test_db().await;
    seed_tenant(&t.db, "acme").await;
    let app = api::build_router(state(t.db.clone(), true, SECRET));

    // Valid signature, but the tenant slug does not exist.
    let res = app
        .oneshot(signed_request(
            "ghost",
            SECRET,
            order_body("ord-6", "SKU-1"),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn oversize_body_returns_413() {
    let t = spawn_test_db().await;
    seed_tenant(&t.db, "acme").await;
    let app = api::build_router(state(t.db.clone(), true, SECRET));

    // 80 KiB body exceeds the 64 KiB cap; rejected before the handler runs.
    let big = "x".repeat(80 * 1024);
    let body = order_body(&big, "SKU-1");
    let res = app
        .oneshot(signed_request("acme", SECRET, body))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

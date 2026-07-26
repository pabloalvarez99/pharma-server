//! Integration tests for the scoped-API-key gate on the public DSS seam
//! (ADR-0014 L1). Exercises `GET /api/v1/public/catalog` with
//! `public_catalog.require_api_key = true` end-to-end: the route is wired to
//! `api::api_key::guard`, so this proves the fail-closed behavior over real HTTP
//! (no key → 401, wrong scope → 403, valid scoped key → 200) plus that the
//! default (require_api_key = false) keeps the slug-only read working.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use pharma_core::config::{DbConfig, JwtConfig, PublicCatalogConfig};
use tempfile::TempDir;
use tower::ServiceExt;

const MIGRATIONS_DIR: &str = "../../migrations";

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

async fn tenant_id(db: &db::Db, slug: &str) -> surrealdb::sql::Thing {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let mut q = db
        .query("SELECT id FROM tenant WHERE slug = $slug LIMIT 1")
        .bind(("slug", slug.to_string()))
        .await
        .expect("find tenant");
    let row: Option<Row> = q.take(0).expect("decode tenant");
    row.expect("tenant row").id
}

async fn seed_tenant_with_product(db: &db::Db, slug: &str) {
    db.query("CREATE tenant SET name = $name, slug = $slug")
        .bind(("name", format!("Tenant {slug}")))
        .bind(("slug", slug.to_string()))
        .await
        .expect("create tenant");
    let t = tenant_id(db, slug).await;
    db.query(
        "CREATE product SET tenant = $t, name = 'Paracetamol', slug = 'para', \
            price = $price, external_id = 'PARA-500', active = true, stock = 5",
    )
    .bind(("t", t))
    .bind(("price", rust_decimal::Decimal::new(1000, 0)))
    .await
    .expect("create product");
}

/// Insert an active API key for `slug` with the given scopes; returns the
/// plaintext secret. Stores only the SHA-256 hash (same as the verifier).
async fn mint_key(db: &db::Db, slug: &str, scopes: &[&str], secret: &str) {
    let t = tenant_id(db, slug).await;
    let scopes: Vec<String> = scopes.iter().map(|s| s.to_string()).collect();
    db.query("CREATE api_key SET tenant = $t, key_hash = $h, scopes = $s, active = true")
        .bind(("t", t))
        .bind(("h", api::api_key::hash_key(secret)))
        .bind(("s", scopes))
        .await
        .expect("create api_key");
}

fn state(db: Arc<db::Db>, require_api_key: bool) -> api::AppState {
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
        public_catalog: PublicCatalogConfig {
            enabled: true,
            cors_origins: vec![],
            require_api_key,
        },
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
        stock_webhook: Arc::new(pharma_core::config::StockWebhookConfig::default()),
        provisioning_key: None,
    }
}

fn get(uri: &str, key: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method("GET").uri(uri);
    if let Some(k) = key {
        b = b.header("x-api-key", k);
    }
    b.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn no_key_required_keeps_slug_only_read() {
    let t = spawn_test_db().await;
    seed_tenant_with_product(&t.db, "acme").await;
    let app = api::build_router(state(t.db.clone(), false));
    let res = app
        .oneshot(get("/api/v1/public/catalog?tenant=acme", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn required_but_missing_key_is_401() {
    let t = spawn_test_db().await;
    seed_tenant_with_product(&t.db, "acme").await;
    let app = api::build_router(state(t.db.clone(), true));
    let res = app
        .oneshot(get("/api/v1/public/catalog?tenant=acme", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wrong_scope_is_403() {
    let t = spawn_test_db().await;
    seed_tenant_with_product(&t.db, "acme").await;
    mint_key(&t.db, "acme", &["orders:write"], "pk_only_orders").await;
    let app = api::build_router(state(t.db.clone(), true));
    let res = app
        .oneshot(get(
            "/api/v1/public/catalog?tenant=acme",
            Some("pk_only_orders"),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn valid_scoped_key_is_200() {
    let t = spawn_test_db().await;
    seed_tenant_with_product(&t.db, "acme").await;
    mint_key(&t.db, "acme", &["catalog:read"], "pk_good").await;
    let app = api::build_router(state(t.db.clone(), true));
    let res = app
        .oneshot(get("/api/v1/public/catalog?tenant=acme", Some("pk_good")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn key_from_other_tenant_is_401() {
    let t = spawn_test_db().await;
    seed_tenant_with_product(&t.db, "acme").await;
    seed_tenant_with_product(&t.db, "other").await;
    // Valid catalog:read key, but minted for `other` — must not read `acme`.
    mint_key(&t.db, "other", &["catalog:read"], "pk_other").await;
    let app = api::build_router(state(t.db.clone(), true));
    let res = app
        .oneshot(get("/api/v1/public/catalog?tenant=acme", Some("pk_other")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

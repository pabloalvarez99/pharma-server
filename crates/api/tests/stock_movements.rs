//! Integration tests for `GET /api/v1/stock-movements` (paginated audit trail).
//!
//! Tests that do NOT need a DB (auth/role gate) use `db: None` — the handler
//! returns 503 when authenticated+authorized but no DB is wired, which is
//! different from 401/403.  Only the shape test needs a live DB and is marked
//! `#[ignore]` so it doesn't block CI on machines without SurrealKv.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use jsonwebtoken::{encode, EncodingKey, Header};
use pharma_core::config::JwtConfig;
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Helpers (mirrors roles_granular.rs)
// ---------------------------------------------------------------------------

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret-sm".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    }
}

fn state_no_db() -> api::AppState {
    api::AppState {
        started_at: chrono::Utc::now(),
        jwt: jwt_cfg(),
        db: None,
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
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
        stock_webhook: std::sync::Arc::new(pharma_core::config::StockWebhookConfig::default()),
        provisioning_key: None,
    }
}

fn token(roles: &[&str]) -> String {
    let cfg = jwt_cfg();
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "sub": "user:test",
        "tenant_id": "tenant:1",
        "roles": roles,
        "iss": cfg.issuer,
        "iat": now,
        "exp": now + 3600_i64,
    });
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(cfg.secret.as_bytes()),
    )
    .expect("issue jwt")
}

async fn get_movements(token_val: Option<&str>, query: &str) -> StatusCode {
    let app = api::build_router(state_no_db());
    let uri = if query.is_empty() {
        "/api/v1/stock-movements".to_string()
    } else {
        format!("/api/v1/stock-movements?{query}")
    };
    let mut builder = Request::builder().method("GET").uri(&uri);
    if let Some(t) = token_val {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    let req = builder.body(Body::empty()).unwrap();
    app.oneshot(req).await.unwrap().status()
}

// ---------------------------------------------------------------------------
// 1. Unauthenticated → 401
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unauthenticated_returns_401() {
    let status = get_movements(None, "").await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "sin token debe rechazar con 401"
    );
}

// ---------------------------------------------------------------------------
// 2. Role below cashier → 403
//    There is no role below cashier in the ladder (cashier is the floor).
//    An empty roles array (authenticated but no role) must be rejected.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_roles_returns_403() {
    let tok = token(&[]);
    let status = get_movements(Some(&tok), "").await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "roles vacíos deben ser rechazados con 403"
    );
}

// ---------------------------------------------------------------------------
// 3. Cashier (minimum qualifying role) reaches handler → NOT 401/403.
//    Without a DB the handler returns 503 Service Unavailable, which means
//    the auth + role gate passed.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cashier_passes_gate_reaches_handler() {
    let tok = token(&["cashier"]);
    let status = get_movements(Some(&tok), "").await;
    assert_ne!(
        status,
        StatusCode::UNAUTHORIZED,
        "cashier no debe recibir 401"
    );
    assert_ne!(status, StatusCode::FORBIDDEN, "cashier no debe recibir 403");
    // Without DB the handler returns 503.
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "sin DB debe retornar 503 (gate pasó)"
    );
}

// ---------------------------------------------------------------------------
// 4. Pharmacist also passes gate (cashier_plus ladder).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pharmacist_passes_gate() {
    let tok = token(&["pharmacist"]);
    let status = get_movements(Some(&tok), "").await;
    assert_ne!(status, StatusCode::FORBIDDEN);
    assert_ne!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// 5. Admin passes gate.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn admin_passes_gate() {
    let tok = token(&["admin"]);
    let status = get_movements(Some(&tok), "").await;
    assert_ne!(status, StatusCode::FORBIDDEN);
    assert_ne!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// 6. product_id filter query param — gate still passes for cashier.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cashier_with_product_id_filter_passes_gate() {
    let tok = token(&["cashier"]);
    let status = get_movements(Some(&tok), "product_id=product%3Axyz").await;
    assert_ne!(
        status,
        StatusCode::FORBIDDEN,
        "cashier con filtro product_id no debe recibir 403"
    );
    assert_ne!(status, StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// 7. Paginated shape test — requires live in-memory SurrealDB.
//    Marked #[ignore] so CI doesn't try to spin a DB.  Run locally with:
//      cargo test -p api stock_movements::paginated_response_shape -- --ignored
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requiere SurrealDB en memoria; ejecutar con --ignored en dev local"]
async fn paginated_response_shape() {
    use http_body_util::BodyExt;

    // Spin up a temp-dir SurrealKv DB, run migrations, create a tenant +
    // product + stock_movement, then verify the response envelope.
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = pharma_core::config::DbConfig {
        path: dir.path().to_string_lossy().into_owned(),
        namespace: "pharma_test".into(),
        database: "main".into(),
    };
    let handle = db::connect(&cfg).await.expect("db connect");
    db::run_migrations(&handle, "../../migrations")
        .await
        .expect("run migrations");
    let db = std::sync::Arc::new(handle);

    // Create a tenant.
    let _: Option<serde_json::Value> = db
        .query("CREATE tenant:test_sm SET name = 'test_sm'")
        .await
        .expect("create tenant")
        .take(0)
        .expect("take tenant");

    // Create a product.
    let _: Option<serde_json::Value> = db
        .query(
            "CREATE product:p1 SET tenant = tenant:test_sm, \
             sku = 'SKU1', name = 'Producto Test', price = 100, stock = 10",
        )
        .await
        .expect("create product")
        .take(0)
        .expect("take product");

    // Insert a stock_movement directly.
    let _: Option<serde_json::Value> = db
        .query(
            "CREATE stock_movement SET tenant = tenant:test_sm, \
             product = product:p1, delta = 5, reason = 'test_receipt'",
        )
        .await
        .expect("create movement")
        .take(0)
        .expect("take movement");

    let app_state = api::AppState {
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
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
        stock_webhook: std::sync::Arc::new(pharma_core::config::StockWebhookConfig::default()),
        provisioning_key: None,
    };

    let tok = {
        let cfg = jwt_cfg();
        let now = chrono::Utc::now().timestamp();
        let claims = serde_json::json!({
            "sub": "user:test_sm",
            "tenant_id": "tenant:test_sm",
            "roles": ["admin"],
            "iss": cfg.issuer,
            "iat": now,
            "exp": now + 3600_i64,
        });
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(cfg.secret.as_bytes()),
        )
        .expect("issue jwt")
    };

    let app = api::build_router(app_state);
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/stock-movements?page=1&limit=10")
        .header("authorization", format!("Bearer {tok}"))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).expect("valid json");

    // Verify envelope shape.
    assert!(
        json.get("data").is_some(),
        "respuesta debe tener campo 'data'"
    );
    assert!(
        json.get("total").is_some(),
        "respuesta debe tener campo 'total'"
    );
    assert!(
        json.get("page").is_some(),
        "respuesta debe tener campo 'page'"
    );
    assert!(
        json.get("limit").is_some(),
        "respuesta debe tener campo 'limit'"
    );

    assert!(json["data"].is_array(), "'data' debe ser array");
    assert_eq!(json["page"], 1, "page debe ser 1");
    assert_eq!(json["limit"], 10, "limit debe ser 10");

    let data = json["data"].as_array().unwrap();
    // We inserted 1 movement.
    assert!(!data.is_empty(), "debe haber al menos 1 movimiento");
    let mv = &data[0];
    assert!(mv.get("id").is_some());
    assert!(mv.get("product").is_some());
    assert!(mv.get("delta").is_some());
    assert!(mv.get("reason").is_some());
    assert!(mv.get("created_at").is_some());
}

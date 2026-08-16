use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use pharma_core::config::JwtConfig;
use tower::ServiceExt;

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    }
}

fn state() -> api::AppState {
    api::AppState {
        started_at: chrono::Utc::now(),
        jwt: jwt_cfg(),
        db: None,
        metrics_token: None,
        node_identity: None,
        data_dir: None,
        license: std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
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

#[tokio::test]
async fn me_without_token_returns_401() {
    let app = api::build_router(state());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_with_invalid_token_returns_401() {
    let app = api::build_router(state());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/me")
                .header("authorization", "Bearer not-a-jwt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_with_valid_token_without_db_returns_503() {
    // /me ya nombra el puesto desde DB; sin DB no hay claims útiles → 503.
    let cfg = jwt_cfg();
    let token = auth::issue(&cfg, "user:abc", "tenant:t1", vec!["admin".into()]).unwrap();

    let app = api::build_router(state());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn login_without_db_returns_503() {
    let app = api::build_router(state());
    let body = serde_json::to_vec(&serde_json::json!({
        "tenant": "acme",
        "email": "a@b.cl",
        "password": "x",
    }))
    .unwrap();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn health_live_does_not_require_token() {
    let app = api::build_router(state());
    let res = app
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

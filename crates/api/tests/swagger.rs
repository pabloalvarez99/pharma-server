//! Swagger UI / OpenAPI integration tests.
//!
//! Exercises the config-gated docs mount wired in `api::build_router`:
//! * `/api-docs/openapi.json` serves the `ApiDoc` document when docs are on,
//! * `/swagger-ui` is served (303 → `/swagger-ui/` → 200) when docs are on,
//! * both 404 when docs are disabled via `AppState.docs_enabled = false`.
//!
//! Harness mirrors `tests/auth.rs`: a no-DB `AppState` + `oneshot`.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use pharma_core::config::JwtConfig;
use tower::ServiceExt;

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    }
}

fn state(docs_enabled: bool) -> api::AppState {
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
        docs_enabled,
    }
}

async fn get(app: &axum::Router, uri: &str) -> axum::http::Response<Body> {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn openapi_json_served_and_parses() {
    let app = api::build_router(state(true));
    let resp = get(&app, "/api-docs/openapi.json").await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let doc: serde_json::Value = serde_json::from_slice(&body).expect("openapi.json is valid JSON");

    // OpenAPI documents carry a top-level `openapi` version string (e.g. 3.1.x).
    let version = doc
        .get("openapi")
        .and_then(|v| v.as_str())
        .expect("doc has `openapi` version field");
    assert!(
        version.starts_with("3."),
        "expected OpenAPI 3.x, got {version}"
    );

    // The document is non-trivial: the registered domain schema is present.
    assert!(
        doc.pointer("/components/schemas/StockMovementDto")
            .is_some(),
        "expected registered StockMovementDto schema in components, body: {doc}"
    );
}

#[tokio::test]
async fn swagger_ui_is_served() {
    let app = api::build_router(state(true));

    // `/swagger-ui` (no trailing slash) redirects to `/swagger-ui/`.
    let resp = get(&app, "/swagger-ui").await;
    assert!(
        resp.status().is_redirection(),
        "expected 3xx redirect at /swagger-ui, got {}",
        resp.status()
    );

    // The UI index itself returns 200.
    let resp = get(&app, "/swagger-ui/").await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "expected 200 serving the Swagger UI index"
    );
}

#[tokio::test]
async fn docs_disabled_returns_404() {
    let app = api::build_router(state(false));

    let resp = get(&app, "/api-docs/openapi.json").await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "openapi.json must 404 when docs are disabled"
    );

    let resp = get(&app, "/swagger-ui").await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "swagger-ui must 404 when docs are disabled"
    );
}

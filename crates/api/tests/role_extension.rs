//! Regression: role-gate middleware must NOT 500 with
//! `Missing request extension: Extension<AllowedRoles>`.
//!
//! Root cause was `Stack::new(inner, outer)` argument order swapped in
//! `middleware::role::layer`: the gate (which extracts the extension) ran
//! BEFORE the layer that attached it, so every admin-only mutating endpoint
//! returned 500 instead of 401/403/2xx.
//!
//! These tests pin the contract for every wired-up admin route. unauth (no
//! Bearer) returns 401, never 500. Wrong role (cashier) returns 403, never 500.
//! Allowed role (admin) reaches the handler — not 500 from a missing extension;
//! at worst a 503 when downstream services we did not wire are needed.

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

fn state() -> api::AppState {
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
        stock_webhook: Arc::new(pharma_core::config::StockWebhookConfig::default()),
    }
}

fn token_for(role: &str) -> String {
    auth::issue(&jwt_cfg(), "user:u1", "tenant:t1", vec![role.into()]).unwrap()
}

async fn post(app: axum::Router, path: &str, token: Option<&str>) -> (StatusCode, String) {
    let mut req = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    // Minimum valid NewProduct body — keeps deserialize from short-circuiting
    // before the middleware runs. The 503 we want surfaces from `db_of` in the
    // handler (db: None), proving the middleware passed.
    let body = serde_json::json!({
        "name": "Paracetamol 500mg",
        "price": "1990",
    });
    let res = app
        .oneshot(
            req.body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn post_products_without_token_returns_401_not_500() {
    let app = api::build_router(state());
    let (status, body) = post(app, "/api/v1/products", None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unauth POST /products: expected 401, got {status}; body={body}"
    );
    assert!(
        !body.contains("Missing request extension"),
        "envelope leaked axum internal error: {body}"
    );
}

#[tokio::test]
async fn post_products_with_cashier_role_returns_403_not_500() {
    let token = token_for("cashier");
    let app = api::build_router(state());
    let (status, body) = post(app, "/api/v1/products", Some(&token)).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "cashier POST /products: expected 403, got {status}; body={body}"
    );
    assert!(
        !body.contains("Missing request extension"),
        "envelope leaked axum internal error: {body}"
    );
    // Verify standard error envelope is returned, not a raw axum error string.
    let v: serde_json::Value = serde_json::from_str(&body).expect("envelope is JSON");
    assert_eq!(v["error"]["code"], "FORBIDDEN");
}

#[tokio::test]
async fn post_products_with_admin_role_passes_middleware_not_500() {
    // db: None → handler hits `db_of(...)` and returns 503 SERVICE_UNAVAILABLE.
    // The point of this test: status MUST be 503 (handler reached), never the
    // pre-bugfix 500 "Missing request extension".
    let token = token_for("admin");
    let app = api::build_router(state());
    let (status, body) = post(app, "/api/v1/products", Some(&token)).await;
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "admin POST /products without db: expected 503 (handler ran), got {status}; body={body}"
    );
    assert!(
        !body.contains("Missing request extension"),
        "envelope leaked axum internal error: {body}"
    );
    let v: serde_json::Value = serde_json::from_str(&body).expect("envelope is JSON");
    assert_eq!(v["error"]["code"], "SERVICE_UNAVAILABLE");
}

#[tokio::test]
async fn post_agent_order_accept_with_cashier_returns_403_not_500() {
    // Second admin-gated surface to prove the fix is global to `role::layer`,
    // not just one route.
    let token = token_for("cashier");
    let app = api::build_router(state());
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/agent-orders/agent_order:abc/accept")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8_lossy(&body);
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "cashier POST /agent-orders/*/accept: expected 403, got {status}; body={body}"
    );
    assert!(
        !body.contains("Missing request extension"),
        "envelope leaked axum internal error: {body}"
    );
}

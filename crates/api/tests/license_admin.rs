//! Admin endpoints for license hot-reload + status.
//!
//! - `POST /api/v1/admin/license/reload` re-reads disk, swaps ArcSwap.
//! - `GET  /api/v1/admin/license/status` returns active license summary.
//!
//! Both require admin/owner role.

use std::sync::Arc;

use arc_swap::ArcSwap;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use license::schema::{License, Tier, SCHEMA_VERSION};
use pharma_core::config::JwtConfig;
use tempfile::tempdir;
use tower::ServiceExt;
use uuid::Uuid;

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    }
}

fn pro_license() -> License {
    License {
        schema_version: SCHEMA_VERSION,
        license_id: "lic_test_pro".into(),
        tenant_id: Uuid::nil(),
        tier: Tier::Pro,
        features: vec!["reports.margins_daily".into()],
        bought_addons: Vec::new(),
        seat_count: 5,
        issued_at: Utc::now() - Duration::days(1),
        expires_at: Some(Utc::now() + Duration::days(30)),
        issuer_did: "did:pharma:test".into(),
        key_id: "lk-test".into(),
        signature: String::new(),
        metadata: None,
    }
}

fn state_with(lic: License, license_path: Option<std::path::PathBuf>) -> api::AppState {
    api::AppState {
        started_at: Utc::now(),
        jwt: jwt_cfg(),
        db: None,
        metrics_token: None,
        node_identity: None,
        data_dir: None,
        license: Arc::new(ArcSwap::from_pointee(lic)),
        license_path,
        rate_limit: None,
        docs_enabled: true,
        public_catalog: pharma_core::config::PublicCatalogConfig::default(),
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
    }
}

fn token_for(role: &str) -> String {
    auth::issue(&jwt_cfg(), "user:u1", "tenant:t1", vec![role.into()]).unwrap()
}

async fn call(
    app: axum::Router,
    method: &str,
    path: &str,
    role: &str,
) -> (StatusCode, serde_json::Value) {
    let token = token_for(role);
    let res = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
async fn status_returns_active_summary() {
    let app = api::build_router(state_with(pro_license(), None));
    let (status, json) = call(app, "GET", "/api/v1/admin/license/status", "admin").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["tier"], "pro");
    assert_eq!(json["status"], "active");
    assert_eq!(json["license_id"], "lic_test_pro");
    assert_eq!(json["features"][0], "reports.margins_daily");
    assert_eq!(json["seat_count"], 5);
}

#[tokio::test]
async fn status_forbidden_without_admin_role() {
    let app = api::build_router(state_with(pro_license(), None));
    let (status, _) = call(app, "GET", "/api/v1/admin/license/status", "cashier").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn reload_forbidden_without_admin_role() {
    let app = api::build_router(state_with(pro_license(), None));
    let (status, _) = call(app, "POST", "/api/v1/admin/license/reload", "cashier").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn reload_without_path_returns_503() {
    // `license_path` not configured (test default) â†’ reload cannot find a
    // file to re-read. The endpoint exists in the table; admin is allowed;
    // failure mode is 503, not 500.
    let app = api::build_router(state_with(pro_license(), None));
    let (status, _) = call(app, "POST", "/api/v1/admin/license/reload", "admin").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn reload_with_missing_file_falls_back_to_free() {
    // license_path points at a non-existent file â†’ loader logs + returns
    // free_default (ADR-0005). Endpoint returns 200 with the fallback summary.
    let dir = tempdir().unwrap();
    let path = dir.path().join("license.json");
    let app = api::build_router(state_with(pro_license(), Some(path)));
    let (status, json) = call(app, "POST", "/api/v1/admin/license/reload", "admin").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["tier"], "free");
    assert!(json["features"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "reports.sales_daily"));
}

#[tokio::test]
async fn reload_with_invalid_file_falls_back_to_free() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("license.json");
    std::fs::write(&path, b"{not even json").unwrap();
    let app = api::build_router(state_with(pro_license(), Some(path)));
    let (status, json) = call(app, "POST", "/api/v1/admin/license/reload", "admin").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["tier"], "free");
}

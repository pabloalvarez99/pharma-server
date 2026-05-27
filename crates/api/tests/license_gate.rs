//! Integration tests for the license gate on `reports.margins_daily`
//! (Fase 10d POC). Verifies:
//! * Free tier â†’ 402 FEATURE_REQUIRES_UPGRADE before any DB work.
//! * Pro tier with the feature â†’ gate passes (and we see the downstream
//!   "no DB wired" 503 instead of a 402, proving the gate let us through).

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use license::schema::{License, Tier, SCHEMA_VERSION};
use pharma_core::config::JwtConfig;
use tower::ServiceExt;
use uuid::Uuid;

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    }
}

fn state_with_license(lic: License) -> api::AppState {
    api::AppState {
        started_at: chrono::Utc::now(),
        jwt: jwt_cfg(),
        db: None,
        metrics_token: None,
        node_identity: None,
        data_dir: None,
        license: Arc::new(arc_swap::ArcSwap::from_pointee(lic)),
        license_path: None,
        rate_limit: None,
        docs_enabled: true,
        public_catalog: pharma_core::config::PublicCatalogConfig::default(),
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
    }
}

fn pro_license_with(feature: &str) -> License {
    License {
        schema_version: SCHEMA_VERSION,
        license_id: "lic_test_pro".into(),
        tenant_id: Uuid::nil(),
        tier: Tier::Pro,
        features: vec![feature.to_string()],
        bought_addons: Vec::new(),
        seat_count: 1,
        issued_at: Utc::now() - Duration::days(1),
        expires_at: Some(Utc::now() + Duration::days(30)),
        issuer_did: "did:pharma:test".into(),
        key_id: "lk-test".into(),
        signature: String::new(),
        metadata: None,
    }
}

async fn get_margins(app: axum::Router) -> (StatusCode, serde_json::Value) {
    let token = auth::issue(&jwt_cfg(), "user:u1", "tenant:t1", vec!["admin".into()]).unwrap();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/reports/margins-daily")
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
async fn free_tier_blocks_margins_daily_with_402() {
    let app = api::build_router(state_with_license(License::free_default(Uuid::nil())));
    let (status, json) = get_margins(app).await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED);
    assert_eq!(json["error"]["code"], "FEATURE_REQUIRES_UPGRADE");
    assert_eq!(json["error"]["details"]["feature"], "reports.margins_daily");
    assert_eq!(json["error"]["details"]["tier_required"], "pro");
}

#[tokio::test]
async fn pro_tier_passes_gate_then_hits_db_unavailable() {
    let app = api::build_router(state_with_license(pro_license_with(
        "reports.margins_daily",
    )));
    let (status, _) = get_margins(app).await;
    // Gate let us through; downstream sees no DB â†’ 503. Proves the gate is
    // not the blocker.
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

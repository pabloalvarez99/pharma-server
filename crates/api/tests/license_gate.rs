//! Integration tests for the license gate on `reports.margins_daily`
//! (Fase 10d POC). Verifies:
//! * Free tier reaches the handler (no 402): reports over the tenant own data
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
        stock_webhook: Arc::new(pharma_core::config::StockWebhookConfig::default()),
        provisioning_key: None,
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
async fn free_tier_is_served_margins_daily() {
    let app = api::build_router(state_with_license(License::free_default(Uuid::nil())));
    let (status, json) = get_margins(app).await;
    // Sin DB cableada el handler llega hasta el 503 de mas abajo. Lo que
    // importa es que llegue: un 402 aca significaria que se volvio a cobrar
    // el margen.
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "Free tiene que atravesar hasta el handler: {json}"
    );
}

/// El caso que motivo abrir el gate: una licencia sin **ninguna** feature en
/// la lista igual ve el margen.
///
/// Es la situacion real de un negocio que se dio de alta antes del cambio y
/// tiene en disco un archivo firmado con la lista vieja. Free es el piso del
/// producto; ningun archivo puede bajarlo.
#[tokio::test]
async fn a_license_file_listing_nothing_still_sees_margins() {
    let mut lic = License::free_default(Uuid::nil());
    lic.features.clear();
    let (status, json) = get_margins(api::build_router(state_with_license(lic))).await;
    assert_ne!(
        status,
        StatusCode::PAYMENT_REQUIRED,
        "un archivo viejo no puede cobrar lo que es gratis: {json}"
    );
}

#[tokio::test]
async fn paid_features_are_still_paid() {
    let lic = pro_license_with("integrations.sii_dte_auto");
    assert!(license::gate::entitled(&lic, "integrations.sii_dte_auto"));

    let free = License::free_default(Uuid::nil());
    assert!(
        !license::gate::entitled(&free, "integrations.sii_dte_auto"),
        "abrir los reportes no puede abrir de paso lo que cuesta plata operar"
    );
}

#[tokio::test]
async fn pro_tier_also_reaches_the_handler() {
    let app = api::build_router(state_with_license(pro_license_with(
        "reports.margins_daily",
    )));
    let (status, _) = get_margins(app).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

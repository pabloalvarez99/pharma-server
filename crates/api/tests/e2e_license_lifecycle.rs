//! E2E del ciclo de vida de la license firmada (cierra Fase 10e del BACKLOG).
//!
//! A diferencia de `license_gate.rs` (que arma un `License` en memoria), estos
//! tests ejercen el **camino de producción completo**: una license Ed25519
//! firmada en disco → `api::load_license_from_with_keys` (parse + verify +
//! consulta CRL) → `AppState` → router HTTP → gate de feature. Cubre:
//!
//! * license Pro firmada en disco ⇒ pasa el gate de `reports.margins_daily`.
//! * misma license + cache CRL que la revoca ⇒ **degrada a Free end-to-end**
//!   (ADR-0006 + ADR-0005 §6, nunca kill-switch) ⇒ 402.
//! * license con firma alterada en disco ⇒ fallback a Free ⇒ 402.
//!
//! El minteo usa un keypair efímero (`agent::Identity`) + tabla de claves
//! inyectada, así no hace falta la clave privada real del licenser.

use std::sync::Arc;

use agent::identity::Identity;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use license::schema::{License, Metadata, Tier, SCHEMA_VERSION};
use pharma_core::config::JwtConfig;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

const TEST_KEY_ID: &str = "lk-e2e-test";

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    }
}

/// Pro license con la feature pedida, firmable contra `id`.
fn pro_license(id: &Identity, license_id: &str, feature: &str) -> License {
    License {
        schema_version: SCHEMA_VERSION,
        license_id: license_id.into(),
        tenant_id: Uuid::nil(),
        tier: Tier::Pro,
        features: vec![feature.to_string()],
        bought_addons: Vec::new(),
        seat_count: 1,
        issued_at: Utc::now() - Duration::days(1),
        expires_at: Some(Utc::now() + Duration::days(30)),
        issuer_did: id.did(),
        key_id: TEST_KEY_ID.into(),
        signature: String::new(),
        metadata: Some(Metadata::default()),
    }
}

/// Serializa + firma el canonical-JSON (mismo esquema que el licenser real:
/// firma sobre el documento sin el campo `signature`).
fn mint_signed_bytes(id: &Identity, lic: &License) -> Vec<u8> {
    let mut value = serde_json::to_value(lic).expect("serialize license");
    let unsigned = license::verify::canonical_unsigned_bytes(&value).expect("canonical");
    let sig = id.sign(&unsigned);
    value["signature"] = Value::String(B64.encode(sig.to_bytes()));
    serde_json::to_vec_pretty(&value).expect("encode")
}

/// Tabla de claves del licenser para el verify inyectado.
fn keys_for(id: &Identity) -> Vec<(&'static str, String)> {
    vec![(TEST_KEY_ID, id.did())]
}

fn keys_as_refs<'a>(keys: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    keys.iter().map(|(k, v)| (*k, v.as_str())).collect()
}

fn state_with_license(lic: License) -> api::AppState {
    api::AppState {
        started_at: Utc::now(),
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

async fn get_margins(app: axum::Router) -> (StatusCode, Value) {
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
    let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json)
}

/// Escribe un `crl_state.json` (cache local ya aplicado) que revoca `ids`.
fn write_crl_state(dir: &std::path::Path, version: u64, ids: &[&str]) {
    let revoked: Vec<Value> = ids.iter().map(|i| Value::String((*i).into())).collect();
    let state = serde_json::json!({
        "last_seen_version": version,
        "updated_at": Utc::now(),
        "revoked": revoked,
    });
    let path = license::default_crl_state_path(dir);
    std::fs::write(&path, serde_json::to_vec_pretty(&state).unwrap()).unwrap();
}

#[tokio::test]
async fn signed_pro_license_on_disk_passes_gate() {
    let id = Identity::generate();
    let dir = tempfile::tempdir().unwrap();
    let lic = pro_license(&id, "lic_e2e_pro", "reports.margins_daily");
    let path = license::default_license_path(dir.path());
    std::fs::write(&path, mint_signed_bytes(&id, &lic)).unwrap();

    // Camino de carga de producción (con claves inyectadas): firma válida ⇒ Pro.
    let keys = keys_for(&id);
    let loaded = api::load_license_from_with_keys(&path, &keys_as_refs(&keys));
    assert_eq!(loaded.tier, Tier::Pro);
    assert_eq!(loaded.license_id, "lic_e2e_pro");

    // Router con esa license ⇒ el gate la deja pasar; sin DB cae a 503 (prueba
    // que el gate NO fue el bloqueante, igual que `license_gate.rs`).
    let (status, _) = get_margins(api::build_router(state_with_license(loaded))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn revoked_license_degrades_to_free_end_to_end() {
    let id = Identity::generate();
    let dir = tempfile::tempdir().unwrap();
    let lic = pro_license(&id, "lic_e2e_revoked", "reports.margins_daily");
    let path = license::default_license_path(dir.path());
    std::fs::write(&path, mint_signed_bytes(&id, &lic)).unwrap();

    // CRL local revoca exactamente este license_id.
    write_crl_state(dir.path(), 5, &["lic_e2e_revoked"]);

    // Carga: la firma es válida pero el CRL la revoca ⇒ degrada a Free.
    let keys = keys_for(&id);
    let loaded = api::load_license_from_with_keys(&path, &keys_as_refs(&keys));
    assert_eq!(loaded.tier, Tier::Free, "revocada debe degradar a Free");

    // End-to-end: Free ⇒ el gate de margins-daily responde 402 (no kill-switch:
    // el core sigue vivo; sólo la feature paga queda bloqueada).
    let (status, json) = get_margins(api::build_router(state_with_license(loaded))).await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED);
    assert_eq!(json["error"]["code"], "FEATURE_REQUIRES_UPGRADE");
}

#[tokio::test]
async fn crl_revoking_other_license_does_not_degrade() {
    let id = Identity::generate();
    let dir = tempfile::tempdir().unwrap();
    let lic = pro_license(&id, "lic_e2e_keep", "reports.margins_daily");
    let path = license::default_license_path(dir.path());
    std::fs::write(&path, mint_signed_bytes(&id, &lic)).unwrap();

    // CRL revoca OTRO id ⇒ esta license sigue Pro.
    write_crl_state(dir.path(), 9, &["lic_someone_else"]);

    let keys = keys_for(&id);
    let loaded = api::load_license_from_with_keys(&path, &keys_as_refs(&keys));
    assert_eq!(loaded.tier, Tier::Pro);
}

#[tokio::test]
async fn tampered_license_on_disk_falls_back_to_free() {
    let id = Identity::generate();
    let dir = tempfile::tempdir().unwrap();
    let lic = pro_license(&id, "lic_e2e_tampered", "reports.margins_daily");
    let path = license::default_license_path(dir.path());

    // Firma válida, luego se altera el license_id en disco ⇒ firma no calza.
    let signed = String::from_utf8(mint_signed_bytes(&id, &lic)).unwrap();
    let tampered = signed.replace("lic_e2e_tampered", "lic_e2e_hacked");
    std::fs::write(&path, tampered).unwrap();

    let keys = keys_for(&id);
    let loaded = api::load_license_from_with_keys(&path, &keys_as_refs(&keys));
    assert_eq!(loaded.tier, Tier::Free, "firma alterada ⇒ fallback a Free");

    let (status, _) = get_margins(api::build_router(state_with_license(loaded))).await;
    assert_eq!(status, StatusCode::PAYMENT_REQUIRED);
}

#[tokio::test]
async fn wrong_issuer_did_falls_back_to_free() {
    // Firmada con `id` pero la tabla de claves mapea el mismo key_id a otro DID
    // ⇒ issuer_did no coincide ⇒ fallback a Free (binario con clave incorrecta
    // / license falsificada).
    let id = Identity::generate();
    let dir = tempfile::tempdir().unwrap();
    let lic = pro_license(&id, "lic_e2e_unknown", "reports.margins_daily");
    let path = license::default_license_path(dir.path());
    std::fs::write(&path, mint_signed_bytes(&id, &lic)).unwrap();

    let other = Identity::generate();
    let wrong_keys = keys_for(&other); // mismo key_id, DID distinto
    let loaded = api::load_license_from_with_keys(&path, &keys_as_refs(&wrong_keys));
    assert_eq!(loaded.tier, Tier::Free);
}

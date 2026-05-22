//! Granular role-gate matrix tests (cashier / pharmacist / admin / owner).
//!
//! These probe the `role::layer` gate via the public router. Mutating
//! endpoints intercepted by the gate return 403 before touching the DB,
//! so `db: None` is fine for matrix coverage. The legacy fallback (no
//! `roles` claim → assume admin) is also covered.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use jsonwebtoken::{encode, EncodingKey, Header};
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
        stock_webhook: std::sync::Arc::new(pharma_core::config::StockWebhookConfig::default()),
    }
}

fn token_with_roles(roles: Vec<&str>) -> String {
    let cfg = jwt_cfg();
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "sub": "user:abc",
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

/// Legacy JWT (no `roles` field at all) — backward-compat path.
fn token_legacy_no_roles() -> String {
    let cfg = jwt_cfg();
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "sub": "user:abc",
        "tenant_id": "tenant:1",
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

async fn hit(method: &str, path: &str, token: &str, body: &str) -> StatusCode {
    let app = api::build_router(state());
    let req = Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    app.oneshot(req).await.unwrap().status()
}

// ---------------------------------------------------------------------------
// Cashier: POS yes, prescriptions no, catalog mutate no.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cashier_can_open_cash_session_but_not_create_prescription() {
    let cashier = token_with_roles(vec!["cashier"]);

    // Cashier reaches POST /cash-sessions (gate passes; handler then 503 on
    // missing DB). The important thing: NOT 403.
    let st = hit("POST", "/api/v1/cash-sessions", &cashier, "{}").await;
    assert_ne!(st, StatusCode::FORBIDDEN, "cashier must pass caja gate");

    // Cashier blocked from creating prescriptions (Ley 20.000).
    let st = hit("POST", "/api/v1/prescriptions", &cashier, "{}").await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    // Cashier blocked from mutating catalog.
    let st = hit("POST", "/api/v1/products", &cashier, "{}").await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

// ---------------------------------------------------------------------------
// Pharmacist: prescriptions yes, catalog mutate no, POS yes.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pharmacist_can_prescribe_but_not_mutate_catalog() {
    let pharm = token_with_roles(vec!["pharmacist"]);

    let st = hit("POST", "/api/v1/prescriptions", &pharm, "{}").await;
    assert_ne!(st, StatusCode::FORBIDDEN, "pharmacist must pass rx gate");

    let st = hit("POST", "/api/v1/products", &pharm, "{}").await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    // POS (cashier_plus) — pharmacist included by ladder.
    let st = hit("POST", "/api/v1/pos/sale", &pharm, "{}").await;
    assert_ne!(st, StatusCode::FORBIDDEN);
}

// ---------------------------------------------------------------------------
// Admin: full access.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn admin_passes_all_gates() {
    let admin = token_with_roles(vec!["admin"]);
    for (m, p) in [
        ("POST", "/api/v1/pos/sale"),
        ("POST", "/api/v1/prescriptions"),
        ("POST", "/api/v1/products"),
        ("POST", "/api/v1/cash-sessions"),
        ("POST", "/api/v1/clientes"),
    ] {
        let st = hit(m, p, &admin, "{}").await;
        assert_ne!(st, StatusCode::FORBIDDEN, "admin blocked at {m} {p}");
    }
}

// ---------------------------------------------------------------------------
// Owner: also full access.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn owner_passes_all_gates() {
    let owner = token_with_roles(vec!["owner"]);
    for (m, p) in [
        ("POST", "/api/v1/pos/sale"),
        ("POST", "/api/v1/prescriptions"),
        ("POST", "/api/v1/products"),
        ("PUT", "/api/v1/settings/store_name"),
    ] {
        let st = hit(m, p, &owner, r#"{"value":"x"}"#).await;
        assert_ne!(st, StatusCode::FORBIDDEN, "owner blocked at {m} {p}");
    }
}

// ---------------------------------------------------------------------------
// Legacy JWT (no `roles`) → default ["admin"] via serde fallback.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn legacy_jwt_without_roles_defaults_to_admin() {
    let tok = token_legacy_no_roles();
    let st = hit("POST", "/api/v1/products", &tok, "{}").await;
    assert_ne!(
        st,
        StatusCode::FORBIDDEN,
        "legacy claim must fall back to admin"
    );
}

// ---------------------------------------------------------------------------
// Empty roles vec → no access.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_roles_array_forbidden() {
    let tok = token_with_roles(vec![]);
    let st = hit("POST", "/api/v1/pos/sale", &tok, "{}").await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

// ---------------------------------------------------------------------------
// RoleSet bitflags coverage.
// ---------------------------------------------------------------------------

#[test]
fn roleset_from_claim_parses_known_roles() {
    use api::role::RoleSet;
    let s = RoleSet::from_claim(&["cashier".into(), "pharmacist".into(), "unknown".into()]);
    assert!(s.contains(RoleSet::CASHIER));
    assert!(s.contains(RoleSet::PHARMACIST));
    assert!(!s.contains(RoleSet::ADMIN));
    assert!(!s.contains(RoleSet::OWNER));
}

#[test]
fn roleset_contains_any_matches_intersection() {
    use api::role::RoleSet;
    let s = RoleSet::from_claim(&["pharmacist".into()]);
    assert!(s.contains_any(RoleSet::PHARMACIST | RoleSet::ADMIN));
    assert!(!s.contains_any(RoleSet::CASHIER | RoleSet::OWNER));
}

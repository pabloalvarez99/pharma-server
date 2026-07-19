//! Role gate for the agent's write endpoint `POST /api/v1/assist/act`
//! (ADR-0016, Wave 3). Reads (`/assist/ask`) stay open to counter staff; the
//! write/confirm endpoint is admin/owner only. `db: None` is fine: the gate
//! runs before the handler, so a 403 is decided without a DB.

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

#[tokio::test]
async fn cashier_cannot_act() {
    let cashier = token_with_roles(vec!["cashier"]);
    let st = hit(
        "POST",
        "/api/v1/assist/act",
        &cashier,
        r#"{"confirm_token":"x"}"#,
    )
    .await;
    assert_eq!(
        st,
        StatusCode::FORBIDDEN,
        "cashier must not reach /assist/act"
    );
}

#[tokio::test]
async fn pharmacist_cannot_act() {
    let pharm = token_with_roles(vec!["pharmacist"]);
    let st = hit(
        "POST",
        "/api/v1/assist/act",
        &pharm,
        r#"{"confirm_token":"x"}"#,
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_and_owner_pass_the_gate() {
    for role in ["admin", "owner"] {
        let tok = token_with_roles(vec![role]);
        let st = hit(
            "POST",
            "/api/v1/assist/act",
            &tok,
            r#"{"confirm_token":"x"}"#,
        )
        .await;
        // Past the gate: handler runs (503 on db:None or 400 invalid token),
        // never 403.
        assert_ne!(st, StatusCode::FORBIDDEN, "{role} must pass the act gate");
    }
}

#[tokio::test]
async fn cashier_can_still_ask() {
    let cashier = token_with_roles(vec!["cashier"]);
    let st = hit(
        "POST",
        "/api/v1/assist/ask",
        &cashier,
        r#"{"question":"ventas hoy"}"#,
    )
    .await;
    assert_ne!(st, StatusCode::FORBIDDEN, "reads stay open to cashier");
}

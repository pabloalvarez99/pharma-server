//! Free Web PR2 — storefront API keys (`/api/v1/admin/web/keys` + the
//! `RequireApiKey` bearer extractor).
//!
//! Contract under test:
//! * Plaintext key + HMAC secret appear exactly once (create/rotate); the
//!   listing never carries hash nor secret.
//! * Missing/unknown/inactive bearer → uniform 401 `INVALID_API_KEY`.
//! * Revoke and rotate kill the old plaintext immediately.
//! * `require_scope` → 403 `SCOPE_DENIED`; a key can never be bound to a
//!   foreign tenant (`ensure_key_matches_tenant`).

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Json, Router,
};
use http_body_util::BodyExt;
use pharma_core::config::{DbConfig, JwtConfig};
use surrealdb::sql::Thing;
use tempfile::TempDir;
use tower::ServiceExt;

use api::public_auth::{ensure_key_matches_tenant, require_scope, RequireApiKey, WebApiKeyCtx};

const MIGRATIONS_DIR: &str = "../../migrations";

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    }
}

struct TestDb {
    db: Arc<db::Db>,
    _dir: TempDir,
}

async fn spawn_test_db() -> TestDb {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = DbConfig {
        path: dir.path().to_string_lossy().into_owned(),
        namespace: "pharma_test".into(),
        database: "main".into(),
    };
    let handle = db::connect(&cfg).await.expect("db connect");
    db::run_migrations(&handle, MIGRATIONS_DIR)
        .await
        .expect("run migrations");
    TestDb {
        db: Arc::new(handle),
        _dir: dir,
    }
}

fn state_with_db(db: Arc<db::Db>) -> api::AppState {
    api::AppState {
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
        stock_webhook: Arc::new(pharma_core::config::StockWebhookConfig::default()),
        provisioning_key: None,
    }
}

async fn seed_tenant(db: &db::Db, slug: &str) -> Thing {
    #[derive(serde::Deserialize)]
    struct Row {
        id: Thing,
    }
    let mut t = db
        .query("CREATE tenant SET name = $name, slug = $slug RETURN AFTER")
        .bind(("name", format!("Tenant {slug}")))
        .bind(("slug", slug.to_string()))
        .await
        .expect("create tenant");
    let tenant: Option<Row> = t.take(0).expect("decode tenant");
    tenant.expect("tenant row").id
}

fn admin_jwt(tenant: &Thing) -> String {
    auth::issue(
        &jwt_cfg(),
        "user:u1",
        &tenant.to_string(),
        vec!["admin".into()],
    )
    .expect("issue admin jwt")
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(b) = bearer {
        req = req.header("authorization", format!("Bearer {b}"));
    }
    let req = match body {
        Some(json) => req
            .header("content-type", "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// Tiny probe surface for the extractor: 200 + the bound tenant/scopes when
/// the bearer is a live storefront key, the extractor's error envelope
/// otherwise. This is exactly how PR3's order routes will consume it.
fn probe_app(state: api::AppState) -> Router {
    async fn probe(RequireApiKey(ctx): RequireApiKey) -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "tenant": ctx.tenant.to_string(),
            "key_id": ctx.key_id.to_string(),
            "scopes": ctx.scopes,
            "has_secret": !ctx.hmac_secret.is_empty(),
        }))
    }
    Router::new().route("/probe", get(probe)).with_state(state)
}

/// Recursive walk: fail if any object key contains `needle` (case-insensitive).
fn assert_no_key_contains(v: &serde_json::Value, needle: &str) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map {
                assert!(
                    !k.to_lowercase().contains(needle),
                    "admin listing leaked key '{k}'"
                );
                assert_no_key_contains(val, needle);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                assert_no_key_contains(item, needle);
            }
        }
        _ => {}
    }
}

async fn create_key(
    app: &Router,
    jwt: &str,
    name: &str,
    scopes: Option<Vec<&str>>,
) -> serde_json::Value {
    let mut body = serde_json::json!({ "name": name });
    if let Some(s) = scopes {
        body["scopes"] = serde_json::json!(s);
    }
    let (status, json) = send(app, "POST", "/api/v1/admin/web/keys", Some(jwt), Some(body)).await;
    assert_eq!(status, StatusCode::CREATED, "create key: {json}");
    json
}

// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_key_returns_plaintext_once_and_list_hides_it() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    let jwt = admin_jwt(&tenant);
    let app = api::build_router(state_with_db(t.db.clone()));

    let created = create_key(&app, &jwt, "Storefront principal", None).await;
    let key = created["key"].as_str().expect("plaintext key");
    let secret = created["hmac_secret"].as_str().expect("hmac secret");
    assert!(key.starts_with("rb_live_"), "key format: {key}");
    assert_eq!(key.len(), "rb_live_".len() + 64);
    assert!(secret.starts_with("whsec_"), "secret format: {secret}");
    assert_eq!(created["key_prefix"], key[..12], "prefix = first 12 chars");
    assert_eq!(created["name"], "Storefront principal");
    assert_eq!(
        created["scopes"],
        serde_json::json!(["catalog:read", "orders:write"]),
        "default scopes"
    );

    // Listing shows metadata only — no hash, no secret, no plaintext.
    let (status, list) = send(&app, "GET", "/api/v1/admin/web/keys", Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK);
    let items = list.as_array().expect("array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["key_prefix"], key[..12]);
    assert_no_key_contains(&list, "hash");
    assert_no_key_contains(&list, "secret");
    assert!(
        !list.to_string().contains(key),
        "plaintext must never appear in the listing"
    );

    // Sin JWT ni rol → la superficie admin está cerrada.
    let (status, _) = send(&app, "GET", "/api/v1/admin/web/keys", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn unknown_or_inactive_key_401_invalid_api_key() {
    let t = spawn_test_db().await;
    let _tenant = seed_tenant(&t.db, "demo").await;
    let probe = probe_app(state_with_db(t.db.clone()));

    // No header.
    let (status, json) = send(&probe, "GET", "/probe", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"]["code"], "INVALID_API_KEY");

    // Unknown key — same envelope, no enumeration.
    let (status, json) = send(&probe, "GET", "/probe", Some("rb_live_nope"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"]["code"], "INVALID_API_KEY");
}

#[tokio::test]
async fn valid_key_binds_ctx_and_revoked_key_401() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    let jwt = admin_jwt(&tenant);
    let app = api::build_router(state_with_db(t.db.clone()));
    let probe = probe_app(state_with_db(t.db.clone()));

    let created = create_key(&app, &jwt, "Web", None).await;
    let key = created["key"].as_str().unwrap();

    let (status, json) = send(&probe, "GET", "/probe", Some(key), None).await;
    assert_eq!(status, StatusCode::OK, "live key passes: {json}");
    assert_eq!(json["tenant"], tenant.to_string());
    assert_eq!(json["has_secret"], true);

    // Revoke → same key now uniformly 401.
    let id = created["id"].as_str().unwrap().to_string();
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/v1/admin/web/keys/{id}"),
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, json) = send(&probe, "GET", "/probe", Some(key), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"]["code"], "INVALID_API_KEY");
}

#[tokio::test]
async fn rotate_invalidates_old_key() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    let jwt = admin_jwt(&tenant);
    let app = api::build_router(state_with_db(t.db.clone()));
    let probe = probe_app(state_with_db(t.db.clone()));

    let created = create_key(&app, &jwt, "Web", Some(vec!["catalog:read"])).await;
    let old_key = created["key"].as_str().unwrap().to_string();
    let id = created["id"].as_str().unwrap().to_string();

    let (status, rotated) = send(
        &app,
        "POST",
        &format!("/api/v1/admin/web/keys/{id}/rotate"),
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "rotate: {rotated}");
    let new_key = rotated["key"].as_str().unwrap();
    assert_ne!(new_key, old_key);
    assert_eq!(rotated["name"], "Web", "rotate keeps the name");
    assert_eq!(
        rotated["scopes"],
        serde_json::json!(["catalog:read"]),
        "rotate keeps the scopes"
    );

    // Old plaintext dead, new one live.
    let (status, _) = send(&probe, "GET", "/probe", Some(&old_key), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(&probe, "GET", "/probe", Some(new_key), None).await;
    assert_eq!(status, StatusCode::OK);

    // Rotating the already-dead id again → 404.
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/v1/admin/web/keys/{id}/rotate"),
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn scope_denied_403() {
    let ctx = WebApiKeyCtx {
        tenant: Thing::from(("tenant", "a")),
        key_id: Thing::from(("api_key", "k")),
        scopes: vec!["catalog:read".into()],
        hmac_secret: "whsec_x".into(),
    };
    require_scope(&ctx, "catalog:read").expect("granted scope passes");
    let err = require_scope(&ctx, "orders:write").unwrap_err();
    assert_eq!(err.code, "SCOPE_DENIED");
    assert_eq!(err.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn key_of_tenant_a_cannot_be_bound_to_tenant_b() {
    let t = spawn_test_db().await;
    let tenant_a = seed_tenant(&t.db, "alpha").await;
    let tenant_b = seed_tenant(&t.db, "beta").await;
    let jwt_a = admin_jwt(&tenant_a);
    let app = api::build_router(state_with_db(t.db.clone()));
    let probe = probe_app(state_with_db(t.db.clone()));

    let created = create_key(&app, &jwt_a, "Alpha web", None).await;
    let key = created["key"].as_str().unwrap();

    // The extractor binds the key to ITS tenant (A)…
    let (status, json) = send(&probe, "GET", "/probe", Some(key), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["tenant"], tenant_a.to_string());

    // …and the tenant-binding helper (PR3 order routes) refuses B.
    let ctx = WebApiKeyCtx {
        tenant: tenant_a.clone(),
        key_id: Thing::from(("api_key", "k")),
        scopes: domain::web_keys::default_scopes(),
        hmac_secret: "whsec_x".into(),
    };
    ensure_key_matches_tenant(&ctx, &tenant_a).expect("own tenant passes");
    let err = ensure_key_matches_tenant(&ctx, &tenant_b).unwrap_err();
    assert_eq!(err.code, "SCOPE_DENIED");
    assert_eq!(err.status, StatusCode::FORBIDDEN);

    // Cross-tenant admin surface: B's admin cannot revoke A's key.
    let jwt_b = admin_jwt(&tenant_b);
    let id = created["id"].as_str().unwrap();
    let (status, _) = send(
        &app,
        "DELETE",
        &format!("/api/v1/admin/web/keys/{id}"),
        Some(&jwt_b),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A's key still alive.
    let (status, _) = send(&probe, "GET", "/probe", Some(key), None).await;
    assert_eq!(status, StatusCode::OK);
}

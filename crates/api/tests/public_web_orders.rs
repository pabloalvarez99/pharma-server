//! Free Web PR3 — public pickup orders (`POST /api/v1/public/{slug}/orders/web`)
//! + admin transitions (`POST /api/v1/admin/orders/{id}/transition`).
//!
//! Contract under test:
//! * Happy path: 201, `RET-XXXX` pickup code, server-side total,
//!   `stock_reserved` incremented (stock itself untouched).
//! * `Idempotency-Key` replay → 200 identical body, NO double reservation.
//! * Oversell (`qty > stock - reserved`) → 422 `INSUFFICIENT_STOCK`;
//!   hidden product → 422 `PRODUCT_NOT_AVAILABLE`.
//! * Unpublished tenant → 404 darkness even with a valid key.
//! * HMAC: tampered body → 401 `SIGNATURE_INVALID`; stale ts → 401
//!   `TIMESTAMP_SKEW`.
//! * Missing scope / cross-tenant key → 403 `SCOPE_DENIED`.
//! * Lifecycle: reserved→preparing→ready_for_pickup→completed releases the
//!   reserve AND decrements stock; cancel releases only.
//! * `GET /api/v1/orders?channel=web` filters to web orders.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use hmac::{Hmac, Mac};
use http_body_util::BodyExt;
use pharma_core::config::{DbConfig, JwtConfig};
use sha2::{Digest, Sha256};
use surrealdb::sql::Thing;
use tempfile::TempDir;
use tower::ServiceExt;

type HmacSha256 = Hmac<Sha256>;

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

async fn publish_web(db: &db::Db, tenant: &Thing) {
    domain::sales::service::set_setting(db, tenant, "web.published", "true")
        .await
        .expect("set web.published");
}

/// Direct-DB product seed. Returns the record id (`product:xxx`).
async fn seed_product(
    db: &db::Db,
    tenant: &Thing,
    name: &str,
    slug: &str,
    price_clp: i64,
    stock: i64,
    online_visible: bool,
) -> String {
    #[derive(serde::Deserialize)]
    struct Row {
        id: Thing,
    }
    let mut r = db
        .query(
            "CREATE product SET tenant = $t, name = $name, slug = $slug, \
             price = <decimal> $price, stock = $stock, active = true, \
             online_visible = $vis, prescription_type = 'direct' RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("name", name.to_string()))
        .bind(("slug", slug.to_string()))
        .bind(("price", price_clp))
        .bind(("stock", stock))
        .bind(("vis", online_visible))
        .await
        .expect("create product");
    let row: Option<Row> = r.take(0).expect("decode product");
    row.expect("product row").id.to_string()
}

async fn product_counters(db: &db::Db, product_id: &str) -> (i64, i64) {
    #[derive(serde::Deserialize)]
    struct Row {
        stock: i64,
        #[serde(default)]
        stock_reserved: i64,
    }
    let pid = surrealdb::sql::thing(product_id).expect("product thing");
    let mut r = db
        .query("SELECT stock, stock_reserved FROM $id")
        .bind(("id", pid))
        .await
        .expect("counters query");
    let row: Option<Row> = r.take(0).expect("decode counters");
    let row = row.expect("product exists");
    (row.stock, row.stock_reserved)
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

/// Sign `body` per the PR3 contract:
/// `hex(HMAC_SHA256(secret, "{ts}.POST.{path}.{sha256_hex(body)}"))`.
fn sign(secret: &str, ts: i64, path: &str, body: &str) -> String {
    let body_hash = hex::encode(Sha256::digest(body.as_bytes()));
    let canonical = format!("{ts}.POST.{path}.{body_hash}");
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(canonical.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

struct SignedPost<'a> {
    key: &'a str,
    secret: &'a str,
    idempotency_key: Option<&'a str>,
    /// Override the signed+sent timestamp (stale-ts test).
    ts: Option<i64>,
    /// Body actually SENT (tamper test signs `body` but sends this).
    send_body: Option<String>,
}

async fn post_web_order(
    app: &Router,
    slug: &str,
    body: &serde_json::Value,
    opts: SignedPost<'_>,
) -> (StatusCode, serde_json::Value) {
    let path = format!("/api/v1/public/{slug}/orders/web");
    let body_s = body.to_string();
    let ts = opts.ts.unwrap_or_else(|| chrono::Utc::now().timestamp());
    let sig = sign(opts.secret, ts, &path, &body_s);
    let sent = opts.send_body.unwrap_or(body_s);
    let mut req = Request::builder()
        .method("POST")
        .uri(&path)
        .header("authorization", format!("Bearer {}", opts.key))
        .header("content-type", "application/json")
        .header("x-rb-timestamp", ts.to_string())
        .header("x-rb-signature", sig);
    if let Some(k) = opts.idempotency_key {
        req = req.header("idempotency-key", k);
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::from(sent)).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// Create a storefront key over the admin API; returns `(key, hmac_secret)`.
async fn mint_key(app: &Router, jwt: &str, scopes: Option<Vec<&str>>) -> (String, String) {
    let mut body = serde_json::json!({ "name": "Storefront" });
    if let Some(s) = scopes {
        body["scopes"] = serde_json::json!(s);
    }
    let (status, json) = send(app, "POST", "/api/v1/admin/web/keys", Some(jwt), Some(body)).await;
    assert_eq!(status, StatusCode::CREATED, "mint key: {json}");
    (
        json["key"].as_str().unwrap().to_string(),
        json["hmac_secret"].as_str().unwrap().to_string(),
    )
}

fn order_body(product_id: &str, qty: i64) -> serde_json::Value {
    serde_json::json!({
        "customer": { "name": "Ana Pérez", "phone": "+56987654321" },
        "fulfillment": { "type": "pickup", "notes": "después de 18:00" },
        "items": [ { "product_id": product_id, "qty": qty } ]
    })
}

fn opts<'a>(key: &'a str, secret: &'a str) -> SignedPost<'a> {
    SignedPost {
        key,
        secret,
        idempotency_key: None,
        ts: None,
        send_body: None,
    }
}

// ---------------------------------------------------------------------------

#[tokio::test]
async fn happy_path_reserves_stock_and_mints_pickup_code() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    publish_web(&t.db, &tenant).await;
    let pid = seed_product(&t.db, &tenant, "Paracetamol", "para", 1290, 10, true).await;
    let jwt = admin_jwt(&tenant);
    let app = api::build_router(state_with_db(t.db.clone()));
    let (key, secret) = mint_key(&app, &jwt, None).await;

    let (status, json) =
        post_web_order(&app, "demo", &order_body(&pid, 2), opts(&key, &secret)).await;
    assert_eq!(status, StatusCode::CREATED, "create: {json}");
    assert_eq!(json["status"], "reserved");
    assert_eq!(json["currency"], "CLP");
    assert_eq!(json["total"], "2580", "server-side total 2×1290");
    assert!(json["expires_at"].as_str().is_some());
    let code = json["pickup_code"].as_str().expect("pickup code");
    assert_eq!(code.len(), 8, "RET- + 4 chars: {code}");
    assert!(code.starts_with("RET-"));
    assert!(
        code[4..]
            .bytes()
            .all(|b| b"ABCDEFGHJKMNPQRSTUVWXYZ23456789".contains(&b)),
        "alphabet excludes 0/O/1/I/L: {code}"
    );

    // Reservation held, stock untouched.
    let (stock, reserved) = product_counters(&t.db, &pid).await;
    assert_eq!((stock, reserved), (10, 2));
}

#[tokio::test]
async fn idempotency_replay_returns_cached_body_without_double_reserve() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    publish_web(&t.db, &tenant).await;
    let pid = seed_product(&t.db, &tenant, "Ibuprofeno", "ibu", 990, 5, true).await;
    let jwt = admin_jwt(&tenant);
    let app = api::build_router(state_with_db(t.db.clone()));
    let (key, secret) = mint_key(&app, &jwt, None).await;

    let body = order_body(&pid, 1);
    let mut o = opts(&key, &secret);
    o.idempotency_key = Some("11111111-2222-3333-4444-555555555555");
    let (status, first) = post_web_order(&app, "demo", &body, o).await;
    assert_eq!(status, StatusCode::CREATED, "first: {first}");

    let mut o = opts(&key, &secret);
    o.idempotency_key = Some("11111111-2222-3333-4444-555555555555");
    let (status, replay) = post_web_order(&app, "demo", &body, o).await;
    assert_eq!(status, StatusCode::OK, "replay: {replay}");
    assert_eq!(replay, first, "cached body verbatim");

    let (_, reserved) = product_counters(&t.db, &pid).await;
    assert_eq!(reserved, 1, "no double reservation");
}

#[tokio::test]
async fn oversell_and_hidden_product_422() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    publish_web(&t.db, &tenant).await;
    let pid = seed_product(&t.db, &tenant, "Aspirina", "asp", 500, 3, true).await;
    let hidden = seed_product(&t.db, &tenant, "Oculto", "oculto", 500, 50, false).await;
    let jwt = admin_jwt(&tenant);
    let app = api::build_router(state_with_db(t.db.clone()));
    let (key, secret) = mint_key(&app, &jwt, None).await;

    // Reserve 2 of 3, then ask 2 more: 2 > 3-2 → insufficient.
    let (status, json) =
        post_web_order(&app, "demo", &order_body(&pid, 2), opts(&key, &secret)).await;
    assert_eq!(status, StatusCode::CREATED, "seed reserve: {json}");
    let (status, json) =
        post_web_order(&app, "demo", &order_body(&pid, 2), opts(&key, &secret)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json["error"]["code"], "INSUFFICIENT_STOCK");
    let (_, reserved) = product_counters(&t.db, &pid).await;
    assert_eq!(reserved, 2, "failed order reserves nothing");

    // Hidden (online_visible = false) → 422 PRODUCT_NOT_AVAILABLE.
    let (status, json) =
        post_web_order(&app, "demo", &order_body(&hidden, 1), opts(&key, &secret)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json["error"]["code"], "PRODUCT_NOT_AVAILABLE");
}

#[tokio::test]
async fn unpublished_tenant_404_even_with_valid_key() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "dark").await;
    // NOT published.
    let pid = seed_product(&t.db, &tenant, "P", "p", 100, 5, true).await;
    let jwt = admin_jwt(&tenant);
    let app = api::build_router(state_with_db(t.db.clone()));
    let (key, secret) = mint_key(&app, &jwt, None).await;

    let (status, json) =
        post_web_order(&app, "dark", &order_body(&pid, 1), opts(&key, &secret)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "404 darkness: {json}");
    assert_eq!(json["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn tampered_body_and_stale_timestamp_401() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    publish_web(&t.db, &tenant).await;
    let pid = seed_product(&t.db, &tenant, "P", "p", 100, 5, true).await;
    let jwt = admin_jwt(&tenant);
    let app = api::build_router(state_with_db(t.db.clone()));
    let (key, secret) = mint_key(&app, &jwt, None).await;

    // Signature computed over qty=1, body sent with qty=5.
    let body = order_body(&pid, 1);
    let mut o = opts(&key, &secret);
    o.send_body = Some(order_body(&pid, 5).to_string());
    let (status, json) = post_web_order(&app, "demo", &body, o).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"]["code"], "SIGNATURE_INVALID", "{json}");

    // Correctly signed but 10 minutes old.
    let mut o = opts(&key, &secret);
    o.ts = Some(chrono::Utc::now().timestamp() - 600);
    let (status, json) = post_web_order(&app, "demo", &body, o).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"]["code"], "TIMESTAMP_SKEW", "{json}");

    // Nothing got reserved along the way.
    let (_, reserved) = product_counters(&t.db, &pid).await;
    assert_eq!(reserved, 0);
}

#[tokio::test]
async fn missing_scope_and_cross_tenant_key_403() {
    let t = spawn_test_db().await;
    let tenant_a = seed_tenant(&t.db, "alpha").await;
    let tenant_b = seed_tenant(&t.db, "beta").await;
    publish_web(&t.db, &tenant_a).await;
    publish_web(&t.db, &tenant_b).await;
    let pid = seed_product(&t.db, &tenant_a, "P", "p", 100, 5, true).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    // Key without orders:write.
    let (ro_key, ro_secret) =
        mint_key(&app, &admin_jwt(&tenant_a), Some(vec!["catalog:read"])).await;
    let (status, json) = post_web_order(
        &app,
        "alpha",
        &order_body(&pid, 1),
        opts(&ro_key, &ro_secret),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "SCOPE_DENIED", "{json}");

    // B's key against A's slug.
    let (b_key, b_secret) = mint_key(&app, &admin_jwt(&tenant_b), None).await;
    let (status, json) =
        post_web_order(&app, "alpha", &order_body(&pid, 1), opts(&b_key, &b_secret)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "SCOPE_DENIED", "{json}");
}

#[tokio::test]
async fn transition_flow_completes_and_cancel_releases() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    publish_web(&t.db, &tenant).await;
    let pid = seed_product(&t.db, &tenant, "P", "p", 1000, 10, true).await;
    let jwt = admin_jwt(&tenant);
    let app = api::build_router(state_with_db(t.db.clone()));
    let (key, secret) = mint_key(&app, &jwt, None).await;

    // Order 1: full pickup flow.
    let (status, o1) =
        post_web_order(&app, "demo", &order_body(&pid, 2), opts(&key, &secret)).await;
    assert_eq!(status, StatusCode::CREATED, "{o1}");
    let o1_id = o1["order_id"].as_str().unwrap().to_string();
    let transition_uri = |id: &str| format!("/api/v1/admin/orders/{id}/transition");

    for (to, expect_status) in [
        ("preparing", "preparing"),
        ("ready_for_pickup", "ready_for_pickup"),
        ("completed", "completed"),
    ] {
        let (status, json) = send(
            &app,
            "POST",
            &transition_uri(&o1_id),
            Some(&jwt),
            Some(serde_json::json!({ "to": to })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{to}: {json}");
        assert_eq!(json["status"], expect_status);
    }
    // Completed: reserve released AND stock decremented.
    let (stock, reserved) = product_counters(&t.db, &pid).await;
    assert_eq!((stock, reserved), (8, 0), "completed consumes stock");

    // Invalid transition from terminal state.
    let (status, json) = send(
        &app,
        "POST",
        &transition_uri(&o1_id),
        Some(&jwt),
        Some(serde_json::json!({ "to": "preparing" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");

    // Order 2: cancel releases the reserve without touching stock.
    let (status, o2) =
        post_web_order(&app, "demo", &order_body(&pid, 3), opts(&key, &secret)).await;
    assert_eq!(status, StatusCode::CREATED, "{o2}");
    let o2_id = o2["order_id"].as_str().unwrap().to_string();
    let (_, reserved) = product_counters(&t.db, &pid).await;
    assert_eq!(reserved, 3);
    let (status, json) = send(
        &app,
        "POST",
        &transition_uri(&o2_id),
        Some(&jwt),
        Some(serde_json::json!({ "to": "cancelled" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "cancel: {json}");
    let (stock, reserved) = product_counters(&t.db, &pid).await;
    assert_eq!((stock, reserved), (8, 0), "cancel releases, keeps stock");
}

#[tokio::test]
async fn orders_channel_filter_returns_only_web() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    publish_web(&t.db, &tenant).await;
    let pid = seed_product(&t.db, &tenant, "P", "p", 1000, 10, true).await;
    let jwt = admin_jwt(&tenant);
    let app = api::build_router(state_with_db(t.db.clone()));
    let (key, secret) = mint_key(&app, &jwt, None).await;

    // One web order + one legacy POS order (channel NONE).
    let (status, web) =
        post_web_order(&app, "demo", &order_body(&pid, 1), opts(&key, &secret)).await;
    assert_eq!(status, StatusCode::CREATED, "{web}");
    t.db.query(
        "CREATE order SET tenant = $t, status = 'paid', payment_method = 'pos_cash', \
         subtotal = <decimal> 1000, discount = <decimal> 0, total = <decimal> 1000",
    )
    .bind(("t", tenant.clone()))
    .await
    .expect("seed pos order")
    .check()
    .expect("pos order row");

    let (status, all) = send(&app, "GET", "/api/v1/orders", Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        all.as_array().unwrap().len(),
        2,
        "both orders visible unfiltered"
    );

    let (status, filtered) =
        send(&app, "GET", "/api/v1/orders?channel=web", Some(&jwt), None).await;
    assert_eq!(status, StatusCode::OK);
    let rows = filtered.as_array().unwrap();
    assert_eq!(rows.len(), 1, "only the web order: {filtered}");
    assert_eq!(rows[0]["channel"], "web");
    assert_eq!(rows[0]["id"], web["order_id"]);
}

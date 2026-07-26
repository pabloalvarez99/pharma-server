//! Shared helpers for `e2e_*` scenario tests. Spawns an in-process app with a
//! tempfile SurrealKv DB + migrations + JWT-issued admin token.
//!
//! NOT touched by other agents — all changes confined to test-time code.
//!
//! ## BUG-001 workaround for fixture seeding
//! Every role-gated WRITE route (`POST /products`, `/batches`, `/pos/sale`, …)
//! currently 500s because `crate::role::layer` builds its `Stack` with the
//! `Extension<AllowedRoles>` and the `from_fn` gate in the wrong order (the
//! gate runs before the extension is injected). So the HTTP `create_product`
//! / `create_batch` helpers below CANNOT be used to seed. Tests seed fixtures
//! by calling `domain::*::service` directly (same business logic the handler
//! wraps) and reserve the HTTP path for READ routes + the `#[ignore]`d
//! BUG-001 reproductions. See the bitácora bug list.

#![allow(dead_code)]

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use domain::money::Decimal;
use http_body_util::BodyExt;
use license::schema::{License, Tier, SCHEMA_VERSION};
use pharma_core::config::{DbConfig, JwtConfig};
use surrealdb::sql::Thing;
use tempfile::TempDir;
use tower::ServiceExt;

pub const MIGRATIONS_DIR: &str = "../../migrations";

/// Parse a `tenant:xxx` string id into a `Thing` for domain-service calls.
pub fn tid_thing(tenant_id: &str) -> Thing {
    surrealdb::sql::thing(tenant_id).expect("tenant id")
}

/// Seed a product directly through `domain::catalog::service` (bypasses the
/// BUG-001 HTTP write gate). Returns the `product:xxx` id.
pub async fn seed_product(
    db: &db::Db,
    tenant: &Thing,
    name: &str,
    price: &str,
    cost: &str,
    stock: i64,
    active_ingredient: Option<&str>,
) -> String {
    use domain::catalog::model::NewProduct;
    let input = NewProduct {
        name: name.to_string(),
        slug: None,
        description: None,
        price: price.parse::<Decimal>().expect("price"),
        cost_price: Some(cost.parse::<Decimal>().expect("cost")),
        stock,
        category: None,
        image_url: None,
        external_id: None,
        laboratory: None,
        therapeutic_action: None,
        active_ingredient: active_ingredient.map(str::to_string),
        prescription_type: None,
        presentation: None,
        discount_percent: None,
        attrs: None,
    };
    let dto = domain::catalog::service::create_product(db, tenant, input)
        .await
        .expect("seed product");
    dto.id
}

/// Seed a batch directly through `domain::inventory::service`. `expiry` is an
/// RFC3339 datetime. Returns the `product_batch:xxx` id.
pub async fn seed_batch(
    db: &db::Db,
    tenant: &Thing,
    product_id: &str,
    code: &str,
    expiry_rfc3339: &str,
    stock: i64,
    cost: &str,
) -> String {
    use domain::inventory::model::NewBatch;
    let input = NewBatch {
        product: product_id.to_string(),
        branch: None,
        batch_code: code.to_string(),
        expiry_date: chrono::DateTime::parse_from_rfc3339(expiry_rfc3339)
            .expect("expiry rfc3339")
            .with_timezone(&chrono::Utc),
        stock,
        cost: Some(cost.parse::<Decimal>().expect("cost")),
        notes: None,
    };
    let dto = domain::inventory::service::create_batch(db, tenant, input, None)
        .await
        .expect("seed batch");
    dto.id
}

/// Seed a customer directly. Returns the `customer:xxx` id.
pub async fn seed_customer(db: &db::Db, tenant: &Thing, name: &str) -> String {
    use domain::customers::model::NewCustomer;
    let input = NewCustomer {
        name: name.to_string(),
        rut: None,
        phone: None,
        email: None,
    };
    let dto = domain::customers::service::create_customer(db, tenant, input)
        .await
        .expect("seed customer");
    dto.id
}

/// One line for a seeded POS sale: (product_id, product_name, qty, unit_price).
pub struct SaleLine<'a> {
    pub product: &'a str,
    pub name: &'a str,
    pub qty: i64,
    pub unit_price: &'a str,
}

/// Drive a full POS sale through `domain::sales::service::post_sale` (bypasses
/// the BUG-001 HTTP gate). Returns the `PosSaleResponse`.
#[allow(clippy::too_many_arguments)]
pub async fn seed_sale(
    db: &db::Db,
    tenant: &Thing,
    sold_by: Option<&Thing>,
    payment_method: &str,
    cash: Option<&str>,
    card: Option<&str>,
    customer: Option<&str>,
    lines: &[SaleLine<'_>],
) -> domain::sales::model::PosSaleResponse {
    use domain::sales::model::{PosSaleItem, PosSaleRequest};
    let items = lines
        .iter()
        .map(|l| PosSaleItem {
            product: l.product.to_string(),
            product_name: l.name.to_string(),
            quantity: l.qty,
            unit_price: l.unit_price.parse::<Decimal>().expect("unit_price"),
        })
        .collect();
    let req = PosSaleRequest {
        items,
        payment_method: payment_method.to_string(),
        cash_amount: cash.map(|c| c.parse::<Decimal>().expect("cash")),
        card_amount: card.map(|c| c.parse::<Decimal>().expect("card")),
        discount: None,
        customer: customer.map(str::to_string),
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: Vec::new(),
        branch: None,
    };
    domain::sales::service::post_sale(db, tenant, sold_by, None, None, req)
        .await
        .expect("seed sale")
}

/// Open a cash session directly. Returns the `cash_register_session:xxx` id.
pub async fn seed_cash_session(
    db: &db::Db,
    tenant: &Thing,
    user: &Thing,
    register_name: &str,
    opening: &str,
) -> String {
    use domain::cash_register::model::OpenSessionInput;
    let input = OpenSessionInput {
        register_name: register_name.to_string(),
        register: None,
        branch: None,
        opening_cash: opening.parse::<Decimal>().expect("opening"),
        notes: None,
    };
    let dto = domain::cash_register::service::open_session(db, tenant, user, input)
        .await
        .expect("open session");
    dto.id
}

pub fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 3600,
    }
}

pub struct TestDb {
    pub db: Arc<db::Db>,
    pub _dir: TempDir,
}

pub async fn spawn_db() -> TestDb {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = DbConfig {
        path: dir.path().to_string_lossy().into_owned(),
        namespace: "pharma_test".into(),
        database: "main".into(),
    };
    let handle = db::connect(&cfg).await.expect("db connect");
    db::run_migrations(&handle, MIGRATIONS_DIR)
        .await
        .expect("migrations");
    TestDb {
        db: Arc::new(handle),
        _dir: dir,
    }
}

/// AppState wrapping `db`, with a Free-tier license.
pub fn state_free(db: Arc<db::Db>) -> api::AppState {
    api::AppState {
        started_at: chrono::Utc::now(),
        jwt: jwt_cfg(),
        db: Some(db),
        metrics_token: None,
        node_identity: Some(Arc::new(agent::Identity::generate())),
        data_dir: None,
        license: Arc::new(arc_swap::ArcSwap::from_pointee(License::free_default(
            uuid::Uuid::nil(),
        ))),
        license_path: None,
        rate_limit: None,
        docs_enabled: true,
        public_catalog: pharma_core::config::PublicCatalogConfig::default(),
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
        stock_webhook: Arc::new(api::stock_webhook::StockWebhookConfig::default()),
        provisioning_key: None,
    }
}

/// AppState with a synthetic Pro-tier license granting the given features
/// (signature blank — `entitled` only checks feature membership, not signature,
/// at the gate layer).
pub fn state_pro(db: Arc<db::Db>, features: &[&str]) -> api::AppState {
    let lic = License {
        schema_version: SCHEMA_VERSION,
        license_id: "lic_test_pro".into(),
        tenant_id: uuid::Uuid::nil(),
        tier: Tier::Pro,
        features: features.iter().map(|s| s.to_string()).collect(),
        bought_addons: Vec::new(),
        seat_count: 1,
        issued_at: chrono::Utc::now() - chrono::Duration::days(1),
        expires_at: Some(chrono::Utc::now() + chrono::Duration::days(30)),
        issuer_did: "did:pharma:test".into(),
        key_id: "lk-test".into(),
        signature: String::new(),
        metadata: None,
    };
    api::AppState {
        started_at: chrono::Utc::now(),
        jwt: jwt_cfg(),
        db: Some(db),
        metrics_token: None,
        node_identity: Some(Arc::new(agent::Identity::generate())),
        data_dir: None,
        license: Arc::new(arc_swap::ArcSwap::from_pointee(lic)),
        license_path: None,
        rate_limit: None,
        docs_enabled: true,
        public_catalog: pharma_core::config::PublicCatalogConfig::default(),
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
        stock_webhook: Arc::new(api::stock_webhook::StockWebhookConfig::default()),
        provisioning_key: None,
    }
}

/// Seed `tenant` + `user(admin)` and return (`tenant:xxx`, `user:yyy`) record-ids
/// as strings, ready to mint a JWT.
pub async fn seed_tenant_admin(
    db: &db::Db,
    slug: &str,
    email: &str,
) -> (String, String, Vec<String>) {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let mut t = db
        .query("CREATE tenant SET name = $n, slug = $s RETURN AFTER")
        .bind(("n", format!("T-{slug}")))
        .bind(("s", slug.to_string()))
        .await
        .expect("create tenant");
    let tid = t
        .take::<Option<Row>>(0)
        .expect("decode")
        .expect("tenant")
        .id;
    let hash = auth::password::hash("pw-x").expect("hash");
    let mut u = db
        .query(
            "CREATE user SET tenant=$t, email=$e, password=$p, \
             roles=$r RETURN AFTER",
        )
        .bind(("t", tid.clone()))
        .bind(("e", email.to_string()))
        .bind(("p", hash))
        .bind(("r", vec!["admin".to_string(), "cashier".to_string()]))
        .await
        .expect("create user");
    let uid = u.take::<Option<Row>>(0).expect("decode").expect("user").id;
    (
        tid.to_string(),
        uid.to_string(),
        vec!["admin".into(), "cashier".into()],
    )
}

pub fn token_for(user_id: &str, tenant_id: &str, roles: Vec<String>) -> String {
    auth::issue(&jwt_cfg(), user_id, tenant_id, roles).expect("issue jwt")
}

/// Send a JSON POST/GET/etc request to the router and return (status, body json).
pub async fn req_json(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
    extra_headers: &[(&str, &str)],
) -> (StatusCode, serde_json::Value) {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(tok) = token {
        b = b.header("authorization", format!("Bearer {tok}"));
    }
    b = b.header("content-type", "application/json");
    for (k, v) in extra_headers {
        b = b.header(*k, *v);
    }
    let body = match body {
        Some(v) => Body::from(serde_json::to_vec(&v).unwrap()),
        None => Body::empty(),
    };
    let req = b.body(body).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::String(
            String::from_utf8_lossy(&bytes).into_owned(),
        ))
    };
    (status, json)
}

/// POST /api/v1/products — returns product json (with `id` field like "product:xxx").
pub async fn create_product(
    app: &Router,
    token: &str,
    name: &str,
    price: &str,
    cost: &str,
    stock: i64,
    active_ingredient: Option<&str>,
) -> serde_json::Value {
    let body = serde_json::json!({
        "name": name,
        "price": price,
        "cost_price": cost,
        "stock": stock,
        "active_ingredient": active_ingredient,
    });
    let (st, json) = req_json(
        app,
        "POST",
        "/api/v1/products",
        Some(token),
        Some(body),
        &[],
    )
    .await;
    assert!(st.is_success(), "create_product {name} failed: {st} {json}");
    json
}

/// POST /api/v1/batches — returns batch json.
pub async fn create_batch(
    app: &Router,
    token: &str,
    product_id: &str,
    code: &str,
    expiry_iso: &str,
    stock: i64,
    cost: &str,
) -> serde_json::Value {
    let body = serde_json::json!({
        "product": product_id,
        "batch_code": code,
        "expiry_date": expiry_iso,
        "stock": stock,
        "cost": cost,
    });
    let (st, json) = req_json(app, "POST", "/api/v1/batches", Some(token), Some(body), &[]).await;
    assert!(st.is_success(), "create_batch failed: {st} {json}");
    json
}

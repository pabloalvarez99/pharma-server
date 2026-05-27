//! Customer detail + purchase history + loyalty search endpoints.
//!
//! In-memory-ish harness (SurrealKv tempdir, same as `integration_db.rs`).
//! Seeds a tenant/user, a customer, and a few `order`/`order_item` rows
//! directly (read-only endpoints — the sales write path is out of scope here),
//! then drives the HTTP surface end-to-end.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use pharma_core::config::{DbConfig, JwtConfig};
use surrealdb::sql::Thing;
use tempfile::TempDir;
use tower::ServiceExt;

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

#[derive(serde::Deserialize)]
struct IdRow {
    id: Thing,
}

/// Create a tenant + an `admin` user, returning `(tenant_thing, user_thing)`.
async fn seed_tenant_and_user(db: &db::Db, slug: &str) -> (Thing, Thing) {
    let mut t = db
        .query("CREATE tenant SET name = $name, slug = $slug RETURN AFTER")
        .bind(("name", format!("Tenant {slug}")))
        .bind(("slug", slug.to_string()))
        .await
        .expect("create tenant");
    let tenant: Option<IdRow> = t.take(0).expect("decode tenant");
    let tenant_id = tenant.expect("tenant row").id;

    let hash = auth::password::hash("pw").expect("hash");
    let mut u = db
        .query(
            "CREATE user SET tenant = $tenant, email = $email, \
             password = $password, roles = $roles RETURN AFTER",
        )
        .bind(("tenant", tenant_id.clone()))
        .bind(("email", format!("u@{slug}.cl")))
        .bind(("password", hash))
        .bind(("roles", vec!["cashier".to_string()]))
        .await
        .expect("create user");
    let user: Option<IdRow> = u.take(0).expect("decode user");
    (tenant_id, user.expect("user row").id)
}

/// Insert a customer; returns its record id string (`customer:xxx`).
#[allow(clippy::too_many_arguments)]
async fn seed_customer(
    db: &db::Db,
    tenant: &Thing,
    name: &str,
    rut: Option<&str>,
    phone: Option<&str>,
    points: i64,
) -> String {
    let mut r = db
        .query(
            "CREATE customer SET tenant = $t, name = $name, rut = $rut, \
             phone = $phone, loyalty_points = $pts RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("name", name.to_string()))
        .bind(("rut", rut.map(str::to_string)))
        .bind(("phone", phone.map(str::to_string)))
        .bind(("pts", points))
        .await
        .expect("create customer");
    let row: Option<IdRow> = r.take(0).expect("decode customer");
    row.expect("customer row").id.to_string()
}

/// Insert an `order` (with an explicit `created_at`) + `n_items` `order_item`
/// rows so history item counts are non-trivial. `total` is a decimal literal.
#[allow(clippy::too_many_arguments)] // test helper — explicit args > opaque struct.
async fn seed_order(
    db: &db::Db,
    tenant: &Thing,
    customer: &str,
    total: &str,
    payment_method: &str,
    status: &str,
    created_at: &str,
    n_items: i64,
) -> String {
    let cust = surrealdb::sql::thing(customer).unwrap();
    // `total` arrives as a string and is cast to `decimal` in-query so this
    // test needs no `rust_decimal` dependency.
    let mut r = db
        .query(
            "CREATE order SET tenant = $t, customer = $c, status = $st, \
             payment_method = $pm, subtotal = <decimal>$tot, total = <decimal>$tot, \
             created_at = $ca RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("c", cust.clone()))
        .bind(("st", status.to_string()))
        .bind(("pm", payment_method.to_string()))
        .bind(("tot", total.to_string()))
        .bind((
            "ca",
            surrealdb::sql::Datetime::from(
                created_at.parse::<chrono::DateTime<chrono::Utc>>().unwrap(),
            ),
        ))
        .await
        .expect("create order");
    let row: Option<IdRow> = r.take(0).expect("decode order");
    let oid = row.expect("order row").id;

    for i in 0..n_items {
        db.query(
            "CREATE order_item SET tenant = $t, order = $o, \
             product_name = $pn, quantity = 1, unit_price = 100, subtotal = 100",
        )
        .bind(("t", tenant.clone()))
        .bind(("o", oid.clone()))
        .bind(("pn", format!("Item {i}")))
        .await
        .expect("create order_item");
    }
    oid.to_string()
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
    }
}

/// Cashier-role token for `tenant` (any role can read customers).
fn token_for(jwt: &JwtConfig, tenant: &Thing) -> String {
    auth::issue(
        jwt,
        "user:test",
        &tenant.to_string(),
        vec!["cashier".into()],
    )
    .expect("issue")
}

async fn get(
    app: &axum::Router,
    uri: &str,
    token: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut b = Request::builder().uri(uri);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let res = app
        .clone()
        .oneshot(b.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json = if body.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null)
    };
    (status, json)
}

#[tokio::test]
async fn detail_returns_loyalty_and_totals() {
    let t = spawn_test_db().await;
    let (tenant, _user) = seed_tenant_and_user(&t.db, "acme").await;
    let cid = seed_customer(
        &t.db,
        &tenant,
        "Juan Perez",
        Some("123456789"),
        Some("+56911"),
        42,
    )
    .await;
    // Two realized orders + one refunded (must NOT count) + one pending (must NOT count).
    seed_order(
        &t.db,
        &tenant,
        &cid,
        "1000.50",
        "pos_cash",
        "paid",
        "2026-05-01T10:00:00Z",
        2,
    )
    .await;
    seed_order(
        &t.db,
        &tenant,
        &cid,
        "2500.00",
        "pos_debit",
        "completed",
        "2026-05-02T10:00:00Z",
        1,
    )
    .await;
    seed_order(
        &t.db,
        &tenant,
        &cid,
        "9999.00",
        "pos_cash",
        "refunded",
        "2026-05-03T10:00:00Z",
        1,
    )
    .await;
    seed_order(
        &t.db,
        &tenant,
        &cid,
        "5000.00",
        "pos_cash",
        "pending",
        "2026-05-04T10:00:00Z",
        1,
    )
    .await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let tok = token_for(&jwt_cfg(), &tenant);
    let (status, json) = get(&app, &format!("/api/v1/customers/{cid}"), Some(&tok)).await;

    assert_eq!(status, StatusCode::OK, "body: {json}");
    assert_eq!(json["name"], "Juan Perez");
    assert_eq!(json["rut"], "123456789");
    assert_eq!(json["loyalty_points"], 42);
    assert_eq!(json["visit_count"], 2, "only paid+completed count");
    // Money is a JSON string (rust_decimal). 1000.50 + 2500.00 = 3500.50.
    assert_eq!(json["total_spent"], "3500.50", "body: {json}");
}

#[tokio::test]
async fn history_returns_only_that_customer_sorted_desc_and_respects_limit() {
    let t = spawn_test_db().await;
    let (tenant, _user) = seed_tenant_and_user(&t.db, "acme").await;
    let alice = seed_customer(&t.db, &tenant, "Alice", Some("111"), None, 0).await;
    let bob = seed_customer(&t.db, &tenant, "Bob", Some("222"), None, 0).await;

    seed_order(
        &t.db,
        &tenant,
        &alice,
        "100",
        "pos_cash",
        "paid",
        "2026-05-01T10:00:00Z",
        3,
    )
    .await;
    seed_order(
        &t.db,
        &tenant,
        &alice,
        "200",
        "pos_debit",
        "paid",
        "2026-05-03T10:00:00Z",
        1,
    )
    .await;
    seed_order(
        &t.db,
        &tenant,
        &alice,
        "300",
        "pos_cash",
        "completed",
        "2026-05-02T10:00:00Z",
        2,
    )
    .await;
    // Bob's order must never appear in Alice's history.
    seed_order(
        &t.db,
        &tenant,
        &bob,
        "999",
        "pos_cash",
        "paid",
        "2026-05-09T10:00:00Z",
        1,
    )
    .await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let tok = token_for(&jwt_cfg(), &tenant);

    // Full history, newest first.
    let (status, json) = get(
        &app,
        &format!("/api/v1/customers/{alice}/history"),
        Some(&tok),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {json}");
    let arr = json.as_array().expect("array");
    assert_eq!(arr.len(), 3, "only Alice's realized orders");
    // Sorted desc by created_at: 05-03 (200), 05-02 (300), 05-01 (100).
    assert_eq!(arr[0]["total"], "200");
    assert_eq!(arr[1]["total"], "300");
    assert_eq!(arr[2]["total"], "100");
    // Item counts surfaced.
    assert_eq!(arr[2]["items_count"], 3);
    assert_eq!(arr[0]["payment_method"], "pos_debit");

    // limit respected.
    let (status, json) = get(
        &app,
        &format!("/api/v1/customers/{alice}/history?limit=1"),
        Some(&tok),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["total"], "200", "newest within limit");
}

#[tokio::test]
async fn search_by_partial_name_finds_customer() {
    let t = spawn_test_db().await;
    let (tenant, _user) = seed_tenant_and_user(&t.db, "acme").await;
    seed_customer(
        &t.db,
        &tenant,
        "Maria Gonzalez",
        Some("55667788"),
        Some("+56999"),
        0,
    )
    .await;
    seed_customer(&t.db, &tenant, "Pedro Soto", Some("99887766"), None, 0).await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let tok = token_for(&jwt_cfg(), &tenant);

    let (status, json) = get(&app, "/api/v1/customers/search?q=gonza", Some(&tok)).await;
    assert_eq!(status, StatusCode::OK, "body: {json}");
    let arr = json.as_array().expect("array");
    assert_eq!(arr.len(), 1, "case-insensitive partial name match");
    assert_eq!(arr[0]["name"], "Maria Gonzalez");
}

#[tokio::test]
async fn search_by_rut_finds_customer() {
    let t = spawn_test_db().await;
    let (tenant, _user) = seed_tenant_and_user(&t.db, "acme").await;
    seed_customer(&t.db, &tenant, "Maria Gonzalez", Some("55667788"), None, 0).await;
    seed_customer(&t.db, &tenant, "Pedro Soto", Some("99887766"), None, 0).await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let tok = token_for(&jwt_cfg(), &tenant);

    let (status, json) = get(&app, "/api/v1/customers/search?q=556677", Some(&tok)).await;
    assert_eq!(status, StatusCode::OK, "body: {json}");
    let arr = json.as_array().expect("array");
    assert_eq!(arr.len(), 1, "partial rut match");
    assert_eq!(arr[0]["rut"], "55667788");
}

#[tokio::test]
async fn search_by_phone_finds_customer() {
    let t = spawn_test_db().await;
    let (tenant, _user) = seed_tenant_and_user(&t.db, "acme").await;
    seed_customer(
        &t.db,
        &tenant,
        "Maria Gonzalez",
        None,
        Some("+56987654321"),
        0,
    )
    .await;
    seed_customer(&t.db, &tenant, "Pedro Soto", None, Some("+56911112222"), 0).await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let tok = token_for(&jwt_cfg(), &tenant);

    let (status, json) = get(&app, "/api/v1/customers/search?q=8765", Some(&tok)).await;
    assert_eq!(status, StatusCode::OK, "body: {json}");
    let arr = json.as_array().expect("array");
    assert_eq!(arr.len(), 1, "partial phone match");
    assert_eq!(arr[0]["name"], "Maria Gonzalez");
}

#[tokio::test]
async fn other_tenant_cannot_see_customer() {
    let t = spawn_test_db().await;
    let (tenant_a, _u) = seed_tenant_and_user(&t.db, "acme").await;
    let (tenant_b, _u2) = seed_tenant_and_user(&t.db, "globex").await;
    let cid = seed_customer(
        &t.db,
        &tenant_a,
        "Acme Client",
        Some("123"),
        Some("+56900"),
        7,
    )
    .await;
    seed_order(
        &t.db,
        &tenant_a,
        &cid,
        "500",
        "pos_cash",
        "paid",
        "2026-05-01T10:00:00Z",
        1,
    )
    .await;

    let app = api::build_router(state_with_db(t.db.clone()));
    // Token scoped to tenant B; tries to read tenant A's customer.
    let tok_b = token_for(&jwt_cfg(), &tenant_b);

    let (status, _json) = get(&app, &format!("/api/v1/customers/{cid}"), Some(&tok_b)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "detail leaks across tenants");

    let (status, _json) = get(
        &app,
        &format!("/api/v1/customers/{cid}/history"),
        Some(&tok_b),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "history leaks across tenants"
    );

    // Search in tenant B must not surface tenant A's customer.
    let (status, json) = get(&app, "/api/v1/customers/search?q=Acme", Some(&tok_b)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        json.as_array().unwrap().is_empty(),
        "search leaks across tenants"
    );
}

#[tokio::test]
async fn unauthenticated_is_401() {
    let t = spawn_test_db().await;
    let (tenant, _user) = seed_tenant_and_user(&t.db, "acme").await;
    let cid = seed_customer(&t.db, &tenant, "Nobody", None, None, 0).await;

    let app = api::build_router(state_with_db(t.db.clone()));

    for uri in [
        format!("/api/v1/customers/{cid}"),
        format!("/api/v1/customers/{cid}/history"),
        "/api/v1/customers/search?q=x".to_string(),
    ] {
        let (status, _json) = get(&app, &uri, None).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "uri {uri} must require auth"
        );
    }
}

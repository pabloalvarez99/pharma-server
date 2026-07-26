//! HTTP-level tests for `GET /api/v1/orders/{id}/receipt` (printable boleta).
//!
//! In-memory harness: temp-file SurrealDB + migrations, seed a tenant + user +
//! product directly, log in for a JWT, POST a POS sale, then fetch its receipt.
//! Money is asserted as JSON strings (SurrealDB Decimal serializes as a string).

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use pharma_core::config::{DbConfig, JwtConfig};
use serde_json::Value;
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
        stock_webhook: std::sync::Arc::new(pharma_core::config::StockWebhookConfig::default()),
        provisioning_key: None,
    }
}

/// Create a tenant + admin user. Returns `(tenant_thing_string, _user)`.
async fn seed_tenant_and_user(
    db: &db::Db,
    name: &str,
    slug: &str,
    email: &str,
    password: &str,
) -> String {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let mut t = db
        .query("CREATE tenant SET name = $name, slug = $slug RETURN AFTER")
        .bind(("name", name.to_string()))
        .bind(("slug", slug.to_string()))
        .await
        .expect("create tenant");
    let tenant: Option<Row> = t.take(0).expect("decode tenant");
    let tenant_id = tenant.expect("tenant row").id;

    let hash = auth::password::hash(password).expect("hash");
    db.query(
        "CREATE user SET tenant = $tenant, email = $email, \
         password = $password, roles = $roles RETURN AFTER",
    )
    .bind(("tenant", tenant_id.clone()))
    .bind(("email", email.to_string()))
    .bind(("password", hash))
    .bind(("roles", vec!["admin".to_string()]))
    .await
    .expect("create user");

    tenant_id.to_string()
}

/// Seed an active product for `tenant_id`. Returns its record-id string.
async fn seed_product(
    db: &db::Db,
    tenant_id: &str,
    name: &str,
    slug: &str,
    price: i64,
    stock: i64,
) -> String {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let tenant = surrealdb::sql::thing(tenant_id).unwrap();
    let mut r = db
        .query(
            "CREATE product SET tenant = $t, name = $name, slug = $slug, \
             price = $price, stock = $stock, active = true RETURN AFTER",
        )
        .bind(("t", tenant))
        .bind(("name", name.to_string()))
        .bind(("slug", slug.to_string()))
        .bind(("price", price))
        .bind(("stock", stock))
        .await
        .expect("create product");
    let row: Option<Row> = r.take(0).expect("decode product");
    row.expect("product row").id.to_string()
}

async fn login(app: &axum::Router, tenant_slug: &str, email: &str, password: &str) -> String {
    let body = serde_json::to_vec(&serde_json::json!({
        "tenant": tenant_slug,
        "email": email,
        "password": password,
    }))
    .unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/login")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "login should succeed");
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    json["token"].as_str().expect("token").to_string()
}

/// POST a POS sale, returning the created order id.
async fn post_sale(app: &axum::Router, token: &str, sale: Value) -> String {
    let body = serde_json::to_vec(&sale).unwrap();
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/pos/sale")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED, "sale should be created");
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    json["order"]["id"].as_str().expect("order id").to_string()
}

async fn fetch_receipt(app: &axum::Router, token: &str, order_id: &str) -> (StatusCode, Value) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/orders/{order_id}/receipt"))
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

#[tokio::test]
async fn cash_sale_receipt_includes_change() {
    let t = spawn_test_db().await;
    let tenant_id =
        seed_tenant_and_user(&t.db, "Farmacia Uno", "uno", "a@uno.cl", "pw-123456").await;
    let pid = seed_product(&t.db, &tenant_id, "Paracetamol 500", "para-500", 1500, 50).await;
    let app = api::build_router(state_with_db(t.db.clone()));
    let token = login(&app, "uno", "a@uno.cl", "pw-123456").await;

    // 3 x 1500 = 4500 total; paid with 5000 cash -> change 500.
    let order_id = post_sale(
        &app,
        &token,
        serde_json::json!({
            "items": [{
                "product": pid,
                "product_name": "Paracetamol 500",
                "quantity": 3,
                "unit_price": "1500",
            }],
            "payment_method": "pos_cash",
            "cash_amount": "5000",
        }),
    )
    .await;

    let (status, r) = fetch_receipt(&app, &token, &order_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(r["order_id"], order_id);
    assert_eq!(r["payment_method"], "pos_cash");
    assert_eq!(r["total"], "4500");
    assert_eq!(r["cash_amount"], "5000");
    assert_eq!(r["change"], "500");
    assert_eq!(r["footer_note"], "Gracias por su compra · Tu Farmacia");
    assert_eq!(r["tenant_name"], "Farmacia Uno");
    // cashier = claims.sub (the user record id) threaded through the sale.
    assert!(r["cashier"].as_str().unwrap().starts_with("user:"));
    // folio_or_number falls back to the order record-id key when no SII folio.
    assert!(!r["folio_or_number"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn card_sale_receipt_has_null_change() {
    let t = spawn_test_db().await;
    let tenant_id =
        seed_tenant_and_user(&t.db, "Farmacia Dos", "dos", "a@dos.cl", "pw-123456").await;
    let pid = seed_product(&t.db, &tenant_id, "Ibuprofeno 400", "ibu-400", 2000, 30).await;
    let app = api::build_router(state_with_db(t.db.clone()));
    let token = login(&app, "dos", "a@dos.cl", "pw-123456").await;

    let order_id = post_sale(
        &app,
        &token,
        serde_json::json!({
            "items": [{
                "product": pid,
                "product_name": "Ibuprofeno 400",
                "quantity": 2,
                "unit_price": "2000",
            }],
            "payment_method": "pos_debit",
            "card_amount": "4000",
        }),
    )
    .await;

    let (status, r) = fetch_receipt(&app, &token, &order_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(r["payment_method"], "pos_debit");
    assert_eq!(r["card_amount"], "4000");
    assert!(r["change"].is_null(), "card sale has no change");
}

#[tokio::test]
async fn receipt_line_totals_are_qty_times_unit_price() {
    let t = spawn_test_db().await;
    let tenant_id =
        seed_tenant_and_user(&t.db, "Farmacia Tres", "tres", "a@tres.cl", "pw-123456").await;
    let p1 = seed_product(&t.db, &tenant_id, "Aspirina", "aspirina", 1000, 40).await;
    let p2 = seed_product(&t.db, &tenant_id, "Loratadina", "loratadina", 2500, 40).await;
    let app = api::build_router(state_with_db(t.db.clone()));
    let token = login(&app, "tres", "a@tres.cl", "pw-123456").await;

    let order_id = post_sale(
        &app,
        &token,
        serde_json::json!({
            "items": [
                { "product": p1, "product_name": "Aspirina", "quantity": 2, "unit_price": "1000" },
                { "product": p2, "product_name": "Loratadina", "quantity": 4, "unit_price": "2500" },
            ],
            "payment_method": "pos_cash",
            "cash_amount": "20000",
        }),
    )
    .await;

    let (status, r) = fetch_receipt(&app, &token, &order_id).await;
    assert_eq!(status, StatusCode::OK);
    let items = r["items"].as_array().expect("items array");
    assert_eq!(items.len(), 2);
    for it in items {
        let qty: i64 = it["qty"].as_i64().unwrap();
        // Prices in this test are whole CLP, so integer math is exact.
        let unit: i64 = it["unit_price"].as_str().unwrap().parse().unwrap();
        let line: i64 = it["line_total"].as_str().unwrap().parse().unwrap();
        assert_eq!(line, unit * qty, "line_total = qty * unit_price");
    }
}

#[tokio::test]
async fn receipt_money_matches_order() {
    let t = spawn_test_db().await;
    let tenant_id = seed_tenant_and_user(
        &t.db,
        "Farmacia Cuatro",
        "cuatro",
        "a@cuatro.cl",
        "pw-123456",
    )
    .await;
    let pid = seed_product(&t.db, &tenant_id, "Amoxicilina", "amoxi", 3000, 25).await;
    let app = api::build_router(state_with_db(t.db.clone()));
    let token = login(&app, "cuatro", "a@cuatro.cl", "pw-123456").await;

    // 4 x 3000 = 12000 subtotal; 2000 discount -> total 10000.
    let order_id = post_sale(
        &app,
        &token,
        serde_json::json!({
            "items": [{
                "product": pid,
                "product_name": "Amoxicilina",
                "quantity": 4,
                "unit_price": "3000",
            }],
            "payment_method": "pos_cash",
            "cash_amount": "10000",
            "discount": "2000",
        }),
    )
    .await;

    // Cross-check the receipt against the canonical order detail endpoint.
    let detail_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/orders/{order_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_res.status(), StatusCode::OK);
    let dbody = detail_res.into_body().collect().await.unwrap().to_bytes();
    let detail: Value = serde_json::from_slice(&dbody).unwrap();

    let (status, r) = fetch_receipt(&app, &token, &order_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(r["subtotal"], "12000");
    assert_eq!(r["discount"], "2000");
    assert_eq!(r["total"], "10000");
    assert_eq!(r["subtotal"], detail["order"]["subtotal"]);
    assert_eq!(r["discount"], detail["order"]["discount"]);
    assert_eq!(r["total"], detail["order"]["total"]);
    // Exact change with discount applied: 10000 cash - 10000 total = 0.
    assert_eq!(r["change"], "0");
}

#[tokio::test]
async fn cross_tenant_receipt_returns_404() {
    let t = spawn_test_db().await;
    let tenant_a = seed_tenant_and_user(&t.db, "Farmacia A", "fa-a", "a@a.cl", "pw-123456").await;
    seed_tenant_and_user(&t.db, "Farmacia B", "fa-b", "b@b.cl", "pw-123456").await;
    let pid = seed_product(&t.db, &tenant_a, "Vitamina C", "vit-c", 800, 60).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    // Tenant A makes a sale.
    let token_a = login(&app, "fa-a", "a@a.cl", "pw-123456").await;
    let order_id = post_sale(
        &app,
        &token_a,
        serde_json::json!({
            "items": [{
                "product": pid,
                "product_name": "Vitamina C",
                "quantity": 1,
                "unit_price": "800",
            }],
            "payment_method": "pos_cash",
            "cash_amount": "1000",
        }),
    )
    .await;

    // Tenant B tries to fetch A's receipt -> 404.
    let token_b = login(&app, "fa-b", "b@b.cl", "pw-123456").await;
    let (status, r) = fetch_receipt(&app, &token_b, &order_id).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(r["error"]["code"], "NOT_FOUND");

    // Sanity: A still sees its own receipt.
    let (status_a, _) = fetch_receipt(&app, &token_a, &order_id).await;
    assert_eq!(status_a, StatusCode::OK);
}

#[tokio::test]
async fn unauthenticated_receipt_returns_401() {
    let t = spawn_test_db().await;
    let tenant_id = seed_tenant_and_user(
        &t.db,
        "Farmacia Sin Auth",
        "noauth",
        "a@noauth.cl",
        "pw-123456",
    )
    .await;
    let pid = seed_product(&t.db, &tenant_id, "Suero", "suero", 500, 10).await;
    let app = api::build_router(state_with_db(t.db.clone()));
    let token = login(&app, "noauth", "a@noauth.cl", "pw-123456").await;
    let order_id = post_sale(
        &app,
        &token,
        serde_json::json!({
            "items": [{
                "product": pid,
                "product_name": "Suero",
                "quantity": 1,
                "unit_price": "500",
            }],
            "payment_method": "pos_cash",
            "cash_amount": "500",
        }),
    )
    .await;

    // No Authorization header -> 401.
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/orders/{order_id}/receipt"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "MISSING_TOKEN");
}

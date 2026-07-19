//! Integration: variantes multi-SKU (migración 0034 / Opción A).
//!
//! Padre tienda + 2 tallas con barcodes distintos; venta decrementa solo la
//! variante; tenant isolation; producto plano sin variantes sigue vendiendo.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use pharma_core::config::{DbConfig, JwtConfig};
use tempfile::TempDir;
use tower::ServiceExt;

const MIGRATIONS_DIR: &str = "../../migrations";

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret-variants-sku-test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    }
}

struct Spawned {
    app: axum::Router,
    token: String,
    token_t2: String,
    _tmp: TempDir,
}

async fn spawn() -> Spawned {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = DbConfig {
        path: tmp.path().to_string_lossy().into_owned(),
        namespace: "pharma_test".into(),
        database: "main".into(),
    };
    let handle = db::connect(&cfg).await.expect("db connect");
    db::run_migrations(&handle, MIGRATIONS_DIR)
        .await
        .expect("migrations");

    async fn make_tenant_user(
        db: &db::Db,
        slug: &str,
        email: &str,
    ) -> (surrealdb::sql::Thing, surrealdb::sql::Thing) {
        let mut r = db
            .query("CREATE tenant SET name=$n, slug=$s RETURN id")
            .bind(("n", format!("Tenant {slug}")))
            .bind(("s", slug.to_string()))
            .await
            .expect("tenant");
        let tenant: surrealdb::sql::Thing = r
            .take::<Option<_>>((0, "id"))
            .expect("decode")
            .expect("row");
        let pw = auth::password::hash("s3cret-pw").unwrap();
        let mut r = db
            .query(
                "CREATE user SET tenant=$t, email=$e, password=$p, \
                 roles=['admin','owner'] RETURN id",
            )
            .bind(("t", tenant.clone()))
            .bind(("e", email.to_string()))
            .bind(("p", pw))
            .await
            .expect("user");
        let user: surrealdb::sql::Thing = r
            .take::<Option<_>>((0, "id"))
            .expect("decode")
            .expect("row");
        (tenant, user)
    }

    let (tenant, user) = make_tenant_user(&handle, "t1", "a@t1.l").await;
    let (_t2, user2) = make_tenant_user(&handle, "t2", "a@t2.l").await;

    let jwt = jwt_cfg();
    let token = auth::issue(
        &jwt,
        &user.to_string(),
        &tenant.to_string(),
        vec!["admin".into(), "owner".into()],
    )
    .expect("issue");
    let token_t2 = auth::issue(
        &jwt,
        &user2.to_string(),
        &_t2.to_string(),
        vec!["admin".into(), "owner".into()],
    )
    .expect("issue t2");

    let db_arc = Arc::new(handle);
    let state = api::AppState {
        started_at: chrono::Utc::now(),
        jwt,
        db: Some(db_arc),
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
    };

    Spawned {
        app: api::build_router(state),
        token,
        token_t2,
        _tmp: tmp,
    }
}

async fn json_body(res: axum::response::Response) -> serde_json::Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or_else(|_| serde_json::json!({}))
}

async fn post_json(
    app: &axum::Router,
    token: &str,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    (status, json_body(res).await)
}

async fn get_json(app: &axum::Router, token: &str, uri: &str) -> (StatusCode, serde_json::Value) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    (status, json_body(res).await)
}

#[tokio::test]
async fn variants_create_list_barcode_sale_and_isolation() {
    let s = spawn().await;

    // Parent product
    let (st, parent) = post_json(
        &s.app,
        &s.token,
        "/api/v1/products",
        serde_json::json!({
            "name": "Polera basica",
            "price": "9990",
            "stock": 0
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "create parent: {parent}");
    let parent_id = parent["id"].as_str().expect("id").to_string();

    // Two size variants with distinct barcodes
    let (st, m) = post_json(
        &s.app,
        &s.token,
        &format!("/api/v1/products/{parent_id}/variants"),
        serde_json::json!({
            "stock": 10,
            "barcode": "7804999700013",
            "attrs": { "talla": "M" }
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "create M: {m}");
    let m_id = m["id"].as_str().unwrap().to_string();
    assert_eq!(m["parent_id"], parent_id);
    assert_eq!(m["stock"], 10);

    let (st, l) = post_json(
        &s.app,
        &s.token,
        &format!("/api/v1/products/{parent_id}/variants"),
        serde_json::json!({
            "stock": 5,
            "barcode": "7804999700020",
            "attrs": { "talla": "L" }
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "create L: {l}");
    let l_id = l["id"].as_str().unwrap().to_string();

    // List variants
    let (st, kids) = get_json(
        &s.app,
        &s.token,
        &format!("/api/v1/products/{parent_id}/variants"),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(kids.as_array().unwrap().len(), 2);

    // Barcode resolves to M
    let (st, by_bc) = get_json(
        &s.app,
        &s.token,
        "/api/v1/products/by-barcode/7804999700013",
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(by_bc["id"], m_id);

    // Sale of variant M decrements only M (POS sale responds 201 Created).
    let (st, sale) = post_json(
        &s.app,
        &s.token,
        "/api/v1/pos/sale",
        serde_json::json!({
            "items": [{
                "product": m_id,
                "product_name": "Polera M",
                "quantity": 3,
                "unit_price": "9990"
            }],
            "payment_method": "pos_cash",
            "cash_amount": "30000"
        }),
    )
    .await;
    assert!(
        st == StatusCode::OK || st == StatusCode::CREATED,
        "sale: {st} {sale}"
    );

    let (st, m_after) = get_json(&s.app, &s.token, &format!("/api/v1/products/{m_id}")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(m_after["stock"], 7);

    let (st, l_after) = get_json(&s.app, &s.token, &format!("/api/v1/products/{l_id}")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(l_after["stock"], 5, "L untouched");

    // Tenant isolation: t2 cannot list t1 variants
    let (st, _) = get_json(
        &s.app,
        &s.token_t2,
        &format!("/api/v1/products/{parent_id}/variants"),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    let (st, _) = get_json(
        &s.app,
        &s.token_t2,
        "/api/v1/products/by-barcode/7804999700013",
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // Plain product (no variants) still sells — farmacia/minimarket path
    let (st, plain) = post_json(
        &s.app,
        &s.token,
        "/api/v1/products",
        serde_json::json!({
            "name": "Paracetamol 500",
            "price": "1500",
            "stock": 20
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let plain_id = plain["id"].as_str().unwrap().to_string();
    let (st, sale2) = post_json(
        &s.app,
        &s.token,
        "/api/v1/pos/sale",
        serde_json::json!({
            "items": [{
                "product": plain_id,
                "product_name": "Paracetamol 500",
                "quantity": 2,
                "unit_price": "1500"
            }],
            "payment_method": "pos_cash",
            "cash_amount": "3000"
        }),
    )
    .await;
    assert!(
        st == StatusCode::OK || st == StatusCode::CREATED,
        "plain sale: {st} {sale2}"
    );
    let (st, plain_after) =
        get_json(&s.app, &s.token, &format!("/api/v1/products/{plain_id}")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(plain_after["stock"], 18);
}

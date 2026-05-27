//! Integration tests for `POST /api/v1/products/import` — the catalog bulk
//! import endpoint. Covers happy-path multipart CSV, edge cases (empty CSV,
//! malformed columns) and the upsert-by-external_id semantics that back
//! re-runnable catalog migrations (see `scripts/import_full_catalog.ps1`).
//!
//! Each test spawns a fresh tempdir SurrealKv with the full migrations set,
//! seeds a tenant + admin user, logs in to obtain a Bearer JWT, then drives
//! the endpoint with a hand-rolled multipart body (axum's `Multipart`
//! extractor parses it the same way nginx / browsers do).
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
const BOUNDARY: &str = "----pharma-import-test-boundary";

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret-test-secret-test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    }
}

struct Spawned {
    app: axum::Router,
    token: String,
    _tmp: TempDir,
    db: Arc<db::Db>,
    tenant: surrealdb::sql::Thing,
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

    let mut r = handle
        .query("CREATE tenant SET name='T', slug='t' RETURN id")
        .await
        .expect("create tenant");
    let tenant: surrealdb::sql::Thing = r
        .take::<Option<_>>((0, "id"))
        .expect("decode tenant")
        .expect("tenant row");
    let pw = auth::password::hash("s3cret-pw").unwrap();
    let mut r = handle
        .query(
            "CREATE user SET tenant=$t, email='a@t.l', password=$p, \
             roles=['admin','owner'] RETURN id",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", pw))
        .await
        .expect("create user");
    let user: surrealdb::sql::Thing = r
        .take::<Option<_>>((0, "id"))
        .expect("decode user")
        .expect("user row");

    let jwt = jwt_cfg();
    let token = auth::issue(
        &jwt,
        &user.to_string(),
        &tenant.to_string(),
        vec!["admin".into(), "owner".into()],
    )
    .expect("issue");

    let db_arc = Arc::new(handle);
    let state = api::AppState {
        started_at: chrono::Utc::now(),
        jwt,
        db: Some(db_arc.clone()),
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
        _tmp: tmp,
        db: db_arc,
        tenant,
    }
}

/// Hand-builds a `multipart/form-data` body with a single `file` field
/// containing `csv` bytes. Matches axum's Multipart extractor.
fn multipart_body(csv: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"catalog.csv\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: text/csv\r\n\r\n");
    body.extend_from_slice(csv.as_bytes());
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

async fn post_import(app: &axum::Router, token: &str, csv: &str) -> (StatusCode, serde_json::Value) {
    let body = multipart_body(csv);
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/products/import")
                .header("authorization", format!("Bearer {token}"))
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={BOUNDARY}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            serde_json::json!({ "_raw": String::from_utf8_lossy(&bytes).to_string() })
        })
    };
    (status, json)
}

async fn count_products(db: &db::Db, tenant: &surrealdb::sql::Thing) -> i64 {
    #[derive(serde::Deserialize)]
    struct Row {
        c: i64,
    }
    let mut q = db
        .query("SELECT count() AS c FROM product WHERE tenant = $t GROUP ALL")
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let rows: Vec<Row> = q.take(0).unwrap();
    rows.first().map(|r| r.c).unwrap_or(0)
}

// 5 valid rows → all created.
#[tokio::test]
async fn import_small_valid_csv_creates_all_rows() {
    let s = spawn().await;
    let csv = "name,price,cost_price,stock,external_id,active_ingredient\n\
               Paracetamol 500mg,1990,1200,40,SKU001,paracetamol\n\
               Ibuprofeno 400mg,2490,1500,25,SKU002,ibuprofeno\n\
               Amoxicilina 500mg,4990,2800,15,SKU003,amoxicilina\n\
               Loratadina 10mg,3490,2000,30,SKU004,loratadina\n\
               Omeprazol 20mg,5490,3100,18,SKU005,omeprazol\n";
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 5);
    assert_eq!(json["updated"], 0);
    assert_eq!(json["failed"], 0);
    assert_eq!(count_products(&s.db, &s.tenant).await, 5);
}

// Empty CSV (header only, zero data rows) → 200 with zero counters and no
// errors. This is the response shape callers MUST be able to retry against.
#[tokio::test]
async fn import_empty_csv_returns_zero_counts() {
    let s = spawn().await;
    let csv = "name,price,external_id\n";
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 0);
    assert_eq!(json["updated"], 0);
    assert_eq!(json["failed"], 0);
    assert!(json["errors"].as_array().unwrap().is_empty());
    assert_eq!(count_products(&s.db, &s.tenant).await, 0);
}

// Second batch with same external_id → updates existing row (no duplicate).
// Backs `scripts/import_full_catalog.ps1` re-run idempotency contract.
#[tokio::test]
async fn import_duplicate_external_id_in_second_batch_updates_row() {
    let s = spawn().await;
    let first = "name,price,cost_price,stock,external_id\n\
                 Paracetamol 500mg,1990,1200,40,SKU100\n";
    let (status, j1) = post_import(&s.app, &s.token, first).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(j1["created"], 1);
    assert_eq!(j1["updated"], 0);
    assert_eq!(count_products(&s.db, &s.tenant).await, 1);

    // Same external_id, NEW price + ingredient. Must update, not duplicate.
    let second = "name,price,cost_price,stock,external_id,active_ingredient\n\
                  Paracetamol 500mg Genfar,2490,1400,40,SKU100,paracetamol\n";
    let (status, j2) = post_import(&s.app, &s.token, second).await;
    assert_eq!(status, StatusCode::OK, "body={j2}");
    assert_eq!(j2["created"], 0, "second batch must NOT create");
    assert_eq!(j2["updated"], 1, "second batch must UPDATE the row");
    assert_eq!(j2["failed"], 0);
    assert_eq!(
        count_products(&s.db, &s.tenant).await,
        1,
        "no duplicate row allowed on re-import by external_id"
    );

    // Verify the row actually carries the new price + ingredient.
    #[derive(serde::Deserialize)]
    struct Row {
        name: String,
        price: rust_decimal::Decimal,
        active_ingredient: Option<String>,
    }
    let mut q = s
        .db
        .query("SELECT name, price, active_ingredient FROM product WHERE tenant=$t LIMIT 1")
        .bind(("t", s.tenant.clone()))
        .await
        .unwrap();
    let rows: Vec<Row> = q.take(0).unwrap();
    let row = rows.first().expect("one product");
    assert_eq!(row.name, "Paracetamol 500mg Genfar");
    assert_eq!(row.price, rust_decimal::Decimal::from(2490));
    assert_eq!(row.active_ingredient.as_deref(), Some("paracetamol"));
}

// CSV without required `name` column → invalid request (400 envelope).
#[tokio::test]
async fn import_csv_missing_required_column_rejects() {
    let s = spawn().await;
    let csv = "external_id,price\nSKU999,1000\n";
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body={json}");
    assert_eq!(count_products(&s.db, &s.tenant).await, 0);
}

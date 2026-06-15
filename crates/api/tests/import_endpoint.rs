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

async fn post_import(
    app: &axum::Router,
    token: &str,
    csv: &str,
) -> (StatusCode, serde_json::Value) {
    post_import_uri(app, token, csv, "/api/v1/products/import").await
}

async fn post_import_raw(
    app: &axum::Router,
    token: &str,
    csv: &[u8],
    uri: &str,
) -> (StatusCode, serde_json::Value) {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"catalog.csv\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: text/csv\r\n\r\n");
    body.extend_from_slice(csv);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
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
        serde_json::from_slice(&bytes).unwrap_or_else(
            |_| serde_json::json!({ "_raw": String::from_utf8_lossy(&bytes).to_string() }),
        )
    };
    (status, json)
}

async fn post_import_uri(
    app: &axum::Router,
    token: &str,
    csv: &str,
    uri: &str,
) -> (StatusCode, serde_json::Value) {
    let body = multipart_body(csv);
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
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
        serde_json::from_slice(&bytes).unwrap_or_else(
            |_| serde_json::json!({ "_raw": String::from_utf8_lossy(&bytes).to_string() }),
        )
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

// Resolves a product id from a barcode the same way the agent/POS lookups do.
async fn product_for_barcode(
    db: &db::Db,
    tenant: &surrealdb::sql::Thing,
    barcode: &str,
) -> Option<surrealdb::sql::Thing> {
    let mut q = db
        .query(
            "SELECT VALUE product FROM product_barcode \
             WHERE tenant = $t AND barcode = $b LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("b", barcode.to_string()))
        .await
        .unwrap();
    q.take::<Option<surrealdb::sql::Thing>>(0).unwrap()
}

async fn count_barcodes(db: &db::Db, tenant: &surrealdb::sql::Thing) -> i64 {
    #[derive(serde::Deserialize)]
    struct Row {
        c: i64,
    }
    let mut q = db
        .query("SELECT count() AS c FROM product_barcode WHERE tenant = $t GROUP ALL")
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let rows: Vec<Row> = q.take(0).unwrap();
    rows.first().map(|r| r.c).unwrap_or(0)
}

// CSV with a `barcode` column → mapping written to `product_barcode`, pointing
// at the created product. Re-importing the same CSV must NOT duplicate the
// mapping (unique index `(tenant, barcode)` + UPSERT).
#[tokio::test]
async fn import_writes_barcode_mapping_idempotently() {
    let s = spawn().await;
    let csv = "name,price,stock,external_id,barcode\n\
               Paracetamol 500mg,1990,40,SKU001,7801234567890\n";
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 1);
    assert!(json["errors"].as_array().unwrap().is_empty(), "body={json}");
    assert_eq!(count_barcodes(&s.db, &s.tenant).await, 1);

    let pid = product_for_barcode(&s.db, &s.tenant, "7801234567890")
        .await
        .expect("barcode resolves to a product");
    assert_eq!(pid.tb, "product");

    // Re-import (now an update-by-external_id path) must keep exactly one
    // mapping row, still pointing at the same product.
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["updated"], 1);
    assert_eq!(
        count_barcodes(&s.db, &s.tenant).await,
        1,
        "re-import must not duplicate the barcode mapping"
    );
    assert_eq!(
        product_for_barcode(&s.db, &s.tenant, "7801234567890").await,
        Some(pid)
    );
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
    let mut q =
        s.db.query("SELECT name, price, active_ingredient FROM product WHERE tenant=$t LIMIT 1")
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

// Excel-in-Chile exports: `;` delimiter must be sniffed, not rejected.
#[tokio::test]
async fn import_semicolon_delimited_csv_creates_rows() {
    let s = spawn().await;
    let csv = "name;price;stock;external_id\n\
               Paracetamol 500mg;1990;40;SKU001\n\
               Ibuprofeno 400mg;2490;25;SKU002\n";
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 2, "body={json}");
    assert_eq!(count_products(&s.db, &s.tenant).await, 2);
}

// "CSV UTF-8" from Excel ships a BOM glued to the first header — must still
// resolve the `name` column instead of rejecting the whole file.
#[tokio::test]
async fn import_bom_prefixed_csv_creates_rows() {
    let s = spawn().await;
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(b"name,price,external_id\nParacetamol,1990,SKU001\n");
    let (status, json) = post_import_raw(&s.app, &s.token, &bytes, "/api/v1/products/import").await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 1, "body={json}");
    assert_eq!(count_products(&s.db, &s.tenant).await, 1);
}

// A migrant's previous system emits Spanish headers (`nombre`, `precio`,
// `codigo`, `existencia`). They must map to the canonical columns.
#[tokio::test]
async fn import_spanish_headers_create_rows() {
    let s = spawn().await;
    let csv = "Nombre;Precio;Existencia;Código;Categoría\n\
               Paracetamol 500mg;1990;40;SKU001;Analgésicos\n";
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 1, "body={json}");

    #[derive(serde::Deserialize)]
    struct Row {
        name: String,
        price: rust_decimal::Decimal,
        external_id: Option<String>,
    }
    let mut q =
        s.db.query("SELECT name, price, external_id FROM product WHERE tenant=$t LIMIT 1")
            .bind(("t", s.tenant.clone()))
            .await
            .unwrap();
    let rows: Vec<Row> = q.take(0).unwrap();
    let row = rows.first().expect("one product");
    assert_eq!(row.name, "Paracetamol 500mg");
    assert_eq!(row.price, rust_decimal::Decimal::from(1990));
    assert_eq!(row.external_id.as_deref(), Some("SKU001"));
}

// A migrant's CSV carries category NAMES, not `category:xxx` record ids. The
// importer must find-or-create the category, and a re-import must reuse it
// (no duplicate category rows).
#[tokio::test]
async fn import_category_by_name_is_created_then_reused() {
    let s = spawn().await;
    async fn count_categories(db: &db::Db, tenant: &surrealdb::sql::Thing) -> i64 {
        #[derive(serde::Deserialize)]
        struct Row {
            c: i64,
        }
        let mut q = db
            .query("SELECT count() AS c FROM category WHERE tenant = $t GROUP ALL")
            .bind(("t", tenant.clone()))
            .await
            .unwrap();
        let rows: Vec<Row> = q.take(0).unwrap();
        rows.first().map(|r| r.c).unwrap_or(0)
    }

    let csv = "name,price,category,external_id\n\
               Paracetamol,1990,Analgésicos,SKU001\n\
               Ibuprofeno,2490,Analgésicos,SKU002\n";
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 2, "body={json}");
    assert_eq!(
        count_categories(&s.db, &s.tenant).await,
        1,
        "both rows share one created category"
    );

    // Re-import (update path) must NOT create a second category row.
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["updated"], 2, "body={json}");
    assert_eq!(
        count_categories(&s.db, &s.tenant).await,
        1,
        "re-import reuses the existing category, never duplicates"
    );
}

// CLP thousands separator: `1.990` is 1990 pesos, NOT 1.99. This was silent
// data corruption — the worst kind of import bug.
#[tokio::test]
async fn import_clp_thousands_price_stores_integer_pesos() {
    let s = spawn().await;
    let csv = "name;precio;external_id\nParacetamol;1.990;SKU001\nIbuprofeno;12.500;SKU002\n";
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 2, "body={json}");

    #[derive(serde::Deserialize)]
    struct Row {
        price: rust_decimal::Decimal,
    }
    let mut q =
        s.db.query("SELECT price FROM product WHERE tenant=$t AND external_id='SKU001' LIMIT 1")
            .bind(("t", s.tenant.clone()))
            .await
            .unwrap();
    let rows: Vec<Row> = q.take(0).unwrap();
    assert_eq!(
        rows.first().expect("product").price,
        rust_decimal::Decimal::from(1990),
        "1.990 must parse as 1990 pesos, not 1.99"
    );
}

// A row with a bad price is reported per-line; the good rows still import
// (partial import, not all-or-nothing).
#[tokio::test]
async fn import_partial_bad_price_reports_row_keeps_good() {
    let s = spawn().await;
    let csv = "name,price,external_id\n\
               Bueno,1990,SKU001\n\
               Malo,no-es-precio,SKU002\n\
               Otro Bueno,2490,SKU003\n";
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 2, "body={json}");
    assert_eq!(json["failed"], 1, "body={json}");
    let errors = json["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["line"], 3, "malo está en la línea 3");
    assert!(
        errors[0]["message"]
            .as_str()
            .unwrap()
            .contains("precio inválido"),
        "mensaje accionable: {}",
        errors[0]["message"]
    );
    assert_eq!(count_products(&s.db, &s.tenant).await, 2);
}

// dry_run=true → validate + count WITHOUT writing anything (preview step).
#[tokio::test]
async fn import_dry_run_counts_but_writes_nothing() {
    let s = spawn().await;
    let csv = "name,price,external_id\n\
               Bueno,1990,SKU001\n\
               Malo,xxx,SKU002\n\
               Otro,2490,SKU003\n";
    let (status, json) = post_import_uri(
        &s.app,
        &s.token,
        csv,
        "/api/v1/products/import?dry_run=true",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 2, "body={json}");
    assert_eq!(json["failed"], 1, "body={json}");
    assert_eq!(json["dry_run"], true, "body={json}");
    assert_eq!(
        count_products(&s.db, &s.tenant).await,
        0,
        "dry-run must not persist any product"
    );

    // A real (non-dry) import of the same file then writes the good rows.
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 2);
    assert_eq!(count_products(&s.db, &s.tenant).await, 2);
}

// Trailing empty rows (Excel padding) are skipped silently, not flagged.
#[tokio::test]
async fn import_blank_trailing_rows_are_skipped() {
    let s = spawn().await;
    let csv = "name,price,external_id\nParacetamol,1990,SKU001\n,,\n,,\n";
    let (status, json) = post_import(&s.app, &s.token, csv).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 1, "body={json}");
    assert_eq!(json["failed"], 0, "blank rows must not count as failures");
    assert!(json["errors"].as_array().unwrap().is_empty());
}

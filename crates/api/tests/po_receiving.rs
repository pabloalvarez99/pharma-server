//! Purchase-order receiving (recepción de mercadería) over the HTTP API on
//! an in-memory SurrealDB with the real migrations applied.
//!
//! Endpoint: `POST /api/v1/purchase-orders/{id}/receive` (admin/owner,
//! tenant-scoped). Body: `{ lines: [{ po_line_id, qty_received, lot?,
//! expiry_date? }], notes? }`. Partial receipts allowed.
//!
//! Semantics asserted here:
//! - full receipt → PO `received`, stock bumped, WAC recomputed exactly;
//! - partial receipt → `partially_received`, only received qty added;
//! - lot+expiry → `product_batch` created;
//! - receiving a `draft` PO → 409 CONFLICT;
//! - non-admin → 403;
//! - cross-tenant isolation (PO not visible to another tenant → 404).

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use pharma_core::config::{DbConfig, JwtConfig};
use rust_decimal::Decimal;
use std::str::FromStr;
use surrealdb::sql::Thing;
use tempfile::TempDir;
use tower::ServiceExt;

const MIGRATIONS_DIR: &str = "../../migrations";

/// Test helper — `dec("123.45")` -> `Decimal`. Avoids `Decimal::from_str` clutter at every assert.
fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("valid decimal literal")
}

/// SurrealDB renders Decimal as `"175dec"` when cast `<string>`. Strip the
/// `dec` suffix so `rust_decimal::from_str` accepts it.
fn parse_surreal_decimal(s: &str) -> Decimal {
    let trimmed = s.strip_suffix("dec").unwrap_or(s);
    Decimal::from_str(trimmed).unwrap_or_else(|e| panic!("surreal decimal {s:?}: {e:?}"))
}

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
        started_at: Utc::now(),
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

/// Create a tenant row, return its `Thing`.
async fn create_tenant(db: &db::Db, slug: &str) -> Thing {
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
    let row: Option<Row> = t.take(0).expect("decode tenant");
    row.expect("tenant row").id
}

fn token_for(tenant: &Thing, role: &str) -> String {
    auth::issue(
        &jwt_cfg(),
        "user:u1",
        &tenant.to_string(),
        vec![role.into()],
    )
    .unwrap()
}

/// Seed stock directly (Fase 3 inventory is out of scope for these tests).
async fn set_stock(db: &db::Db, product_id: &str, stock: i64) {
    let pid = surrealdb::sql::thing(product_id).unwrap();
    db.query("UPDATE product SET stock = $s WHERE id = $p")
        .bind(("p", pid))
        .bind(("s", stock))
        .await
        .unwrap();
}

/// Move a PO into a given status directly (no "send" endpoint in scope).
async fn set_po_status(db: &db::Db, po_id: &str, status: &str) {
    let po = surrealdb::sql::thing(po_id).unwrap();
    db.query("UPDATE purchase_order SET status = $st WHERE id = $p")
        .bind(("p", po))
        .bind(("st", status.to_string()))
        .await
        .unwrap();
}

/// Read `(stock, cost_price-as-string)` for a product. `cost_price` is cast to
/// a string in SurrealQL so the test needs no `rust_decimal` dependency
/// (decimals serialize as JSON strings — see project gotchas).
async fn product_stock_cost(db: &db::Db, product_id: &str) -> (i64, Option<String>) {
    #[derive(serde::Deserialize)]
    struct Row {
        stock: i64,
        cost: Option<String>,
    }
    let pid = surrealdb::sql::thing(product_id).unwrap();
    let mut q = db
        .query("SELECT stock, <string> cost_price AS cost FROM product WHERE id = $p LIMIT 1")
        .bind(("p", pid))
        .await
        .unwrap();
    let row: Option<Row> = q.take(0).unwrap();
    let row = row.unwrap();
    (row.stock, row.cost)
}

/// Count `stock_movement` rows for a tenant filtered by reason.
async fn movement_sum(db: &db::Db, tenant: &Thing, reason: &str) -> (i64, i64) {
    #[derive(serde::Deserialize)]
    struct Row {
        delta: i64,
    }
    let mut q = db
        .query("SELECT delta FROM stock_movement WHERE tenant = $t AND reason = $r")
        .bind(("t", tenant.clone()))
        .bind(("r", reason.to_string()))
        .await
        .unwrap();
    let rows: Vec<Row> = q.take(0).unwrap();
    let count = rows.len() as i64;
    let total: i64 = rows.iter().map(|r| r.delta).sum();
    (count, total)
}

async fn http_receive(
    app: axum::Router,
    po_id: &str,
    token: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/purchase-orders/{po_id}/receive"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// Seed a supplier + one catalogued product + a PO with a single line on it,
/// then move the PO to `sent`. Returns (po_id, line_id, product_id). Built
/// with raw SurrealQL so the test crate needs no `rust_decimal`/`domain`
/// model deps. `cost_price` is left unset when `prod_cost` is `None` (so the
/// first-receipt WAC-seed path is exercised).
async fn seed_po_single_line(
    db: &db::Db,
    tenant: &Thing,
    prod_name: &str,
    prod_cost: Option<&str>,
    qty: i64,
    unit_cost: &str,
) -> (String, String, String) {
    #[derive(serde::Deserialize)]
    struct Id {
        id: Thing,
    }

    let slug = prod_name.to_lowercase();
    let cost_clause = match prod_cost {
        Some(c) => format!(", cost_price = <decimal> {c}"),
        None => String::new(),
    };
    let mut pr = db
        .query(format!(
            "CREATE product SET tenant = $t, name = $n, slug = $slug, \
             price = <decimal> 1990, stock = 0{cost_clause} RETURN AFTER"
        ))
        .bind(("t", tenant.clone()))
        .bind(("n", prod_name.to_string()))
        .bind(("slug", slug))
        .await
        .expect("create product");
    let prod: Option<Id> = pr.take(0).expect("decode product");
    let pid = prod.expect("product row").id;

    let mut sr = db
        .query("CREATE supplier SET tenant = $t, name = 'ACME' RETURN AFTER")
        .bind(("t", tenant.clone()))
        .await
        .expect("create supplier");
    let sup: Option<Id> = sr.take(0).expect("decode supplier");
    let sid = sup.expect("supplier row").id;

    let subtotal = format!("(<decimal> {unit_cost}) * {qty}");
    let mut por = db
        .query(format!(
            "CREATE purchase_order SET tenant = $t, supplier = $s, status = 'draft', \
             currency = 'CLP', total = {subtotal} RETURN AFTER"
        ))
        .bind(("t", tenant.clone()))
        .bind(("s", sid))
        .await
        .expect("create po");
    let po: Option<Id> = por.take(0).expect("decode po");
    let po_id = po.expect("po row").id;

    let mut lir = db
        .query(format!(
            "CREATE purchase_order_item SET tenant = $t, purchase_order = $po, \
             product = $p, product_name = $n, quantity = $q, \
             unit_cost = <decimal> {unit_cost}, subtotal = {subtotal} RETURN AFTER"
        ))
        .bind(("t", tenant.clone()))
        .bind(("po", po_id.clone()))
        .bind(("p", pid.clone()))
        .bind(("n", prod_name.to_string()))
        .bind(("q", qty))
        .await
        .expect("create po item");
    let line: Option<Id> = lir.take(0).expect("decode po item");
    let line_id = line.expect("po item row").id;

    set_po_status(db, &po_id.to_string(), "sent").await;
    (po_id.to_string(), line_id.to_string(), pid.to_string())
}

#[tokio::test]
async fn full_receipt_marks_received_bumps_stock_and_recomputes_wac() {
    let t = spawn_test_db().await;
    let tenant = create_tenant(&t.db, "acme").await;
    // Seed: 10 units @ cost 100. Receive 30 @ 200 → WAC = (10*100 + 30*200)/40
    // = 7000/40 = 175. Stock 10 → 40.
    let (po_id, line_id, pid) =
        seed_po_single_line(&t.db, &tenant, "Paracetamol", Some("100"), 30, "200").await;
    set_stock(&t.db, &pid, 10).await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant, "admin");
    let (status, json) = http_receive(
        app,
        &po_id,
        &token,
        serde_json::json!({ "lines": [{ "po_line_id": line_id, "qty_received": 30 }] }),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["status"], "received");
    assert_eq!(json["items"][0]["qty_received"], 30);

    let (stock, cost) = product_stock_cost(&t.db, &pid).await;
    assert_eq!(stock, 40);
    assert_eq!(cost.as_deref().map(parse_surreal_decimal), Some(dec("175")));

    // Exactly one purchase_receive movement, delta +30.
    let (count, total) = movement_sum(&t.db, &tenant, "purchase_receive").await;
    assert_eq!(count, 1);
    assert_eq!(total, 30);
}

#[tokio::test]
async fn partial_receipt_marks_partially_received_and_adds_only_received_qty() {
    let t = spawn_test_db().await;
    let tenant = create_tenant(&t.db, "acme").await;
    // Order 100 @ 50, product starts empty + no prior cost. Receive 40 only.
    // First receipt seeds cost to line average = 50. Stock 0 → 40.
    let (po_id, line_id, pid) =
        seed_po_single_line(&t.db, &tenant, "Ibuprofeno", None, 100, "50").await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant, "admin");
    let (status, json) = http_receive(
        app,
        &po_id,
        &token,
        serde_json::json!({ "lines": [{ "po_line_id": line_id, "qty_received": 40 }] }),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["status"], "partially_received");
    assert_eq!(json["items"][0]["qty_received"], 40);

    let (stock, cost) = product_stock_cost(&t.db, &pid).await;
    assert_eq!(stock, 40);
    assert_eq!(cost.as_deref().map(parse_surreal_decimal), Some(dec("50")));
    let (count, total) = movement_sum(&t.db, &tenant, "purchase_receive").await;
    assert_eq!(count, 1);
    assert_eq!(total, 40);
}

#[tokio::test]
async fn second_partial_receipt_completes_po_to_received() {
    let t = spawn_test_db().await;
    let tenant = create_tenant(&t.db, "acme").await;
    let (po_id, line_id, pid) =
        seed_po_single_line(&t.db, &tenant, "Amoxicilina", None, 100, "50").await;

    let token = token_for(&tenant, "admin");
    // First receive 60 → partially_received.
    let app = api::build_router(state_with_db(t.db.clone()));
    let (s1, j1) = http_receive(
        app,
        &po_id,
        &token,
        serde_json::json!({ "lines": [{ "po_line_id": line_id, "qty_received": 60 }] }),
    )
    .await;
    assert_eq!(s1, StatusCode::OK, "body={j1}");
    assert_eq!(j1["status"], "partially_received");

    // Second receive 40 → completes to received (cumulative 100/100).
    let app2 = api::build_router(state_with_db(t.db.clone()));
    let (s2, j2) = http_receive(
        app2,
        &po_id,
        &token,
        serde_json::json!({ "lines": [{ "po_line_id": line_id, "qty_received": 40 }] }),
    )
    .await;
    assert_eq!(s2, StatusCode::OK, "body={j2}");
    assert_eq!(j2["status"], "received");
    assert_eq!(j2["items"][0]["qty_received"], 100);

    let (stock, _cost) = product_stock_cost(&t.db, &pid).await;
    assert_eq!(stock, 100);
    // Two receipts → two movements summing to 100.
    let (count, total) = movement_sum(&t.db, &tenant, "purchase_receive").await;
    assert_eq!(count, 2);
    assert_eq!(total, 100);
}

#[tokio::test]
async fn receipt_with_lot_and_expiry_creates_product_batch() {
    let t = spawn_test_db().await;
    let tenant = create_tenant(&t.db, "acme").await;
    let (po_id, line_id, pid) =
        seed_po_single_line(&t.db, &tenant, "Loratadina", Some("80"), 25, "120").await;
    let expiry =
        (Utc::now() + Duration::days(365)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant, "admin");
    let (status, json) = http_receive(
        app,
        &po_id,
        &token,
        serde_json::json!({
            "lines": [{
                "po_line_id": line_id,
                "qty_received": 25,
                "lot": "L-2026-A",
                "expiry_date": expiry,
            }]
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["status"], "received");

    #[derive(serde::Deserialize)]
    struct Batch {
        batch_code: String,
        stock: i64,
        product: Thing,
        cost: Option<String>,
    }
    let mut q =
        t.db.query("SELECT batch_code, stock, product, cost FROM product_batch WHERE tenant = $t")
            .bind(("t", tenant.clone()))
            .await
            .unwrap();
    let rows: Vec<Batch> = q.take(0).unwrap();
    assert_eq!(rows.len(), 1, "one product_batch created");
    assert_eq!(rows[0].batch_code, "L-2026-A");
    assert_eq!(rows[0].stock, 25);
    assert_eq!(rows[0].product.to_string(), pid);
    assert_eq!(
        rows[0].cost.as_deref().map(parse_surreal_decimal),
        Some(dec("120"))
    );
}

#[tokio::test]
async fn receiving_a_draft_po_is_conflict() {
    let t = spawn_test_db().await;
    let tenant = create_tenant(&t.db, "acme").await;
    let (po_id, line_id, _pid) =
        seed_po_single_line(&t.db, &tenant, "Omeprazol", Some("90"), 10, "150").await;
    // Force back to draft — receiving must be refused.
    set_po_status(&t.db, &po_id, "draft").await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant, "admin");
    let (status, json) = http_receive(
        app,
        &po_id,
        &token,
        serde_json::json!({ "lines": [{ "po_line_id": line_id, "qty_received": 10 }] }),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT, "body={json}");
    assert_eq!(json["error"]["code"], "CONFLICT");
    // State untouched: no stock movement, PO still draft.
    let (count, _) = movement_sum(&t.db, &tenant, "purchase_receive").await;
    assert_eq!(count, 0);
}

#[tokio::test]
async fn non_admin_role_is_forbidden() {
    let t = spawn_test_db().await;
    let tenant = create_tenant(&t.db, "acme").await;
    let (po_id, line_id, _pid) =
        seed_po_single_line(&t.db, &tenant, "Diclofenaco", Some("70"), 10, "110").await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant, "cashier");
    let (status, _json) = http_receive(
        app,
        &po_id,
        &token,
        serde_json::json!({ "lines": [{ "po_line_id": line_id, "qty_received": 10 }] }),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    // Role gate runs before the handler → nothing moved.
    let (count, _) = movement_sum(&t.db, &tenant, "purchase_receive").await;
    assert_eq!(count, 0);
}

#[tokio::test]
async fn cross_tenant_po_is_not_found() {
    let t = spawn_test_db().await;
    let tenant_a = create_tenant(&t.db, "acme").await;
    let tenant_b = create_tenant(&t.db, "globex").await;
    let (po_id, line_id, _pid) =
        seed_po_single_line(&t.db, &tenant_a, "Cetirizina", Some("60"), 10, "95").await;

    // Tenant B tries to receive tenant A's PO.
    let app = api::build_router(state_with_db(t.db.clone()));
    let token_b = token_for(&tenant_b, "admin");
    let (status, json) = http_receive(
        app,
        &po_id,
        &token_b,
        serde_json::json!({ "lines": [{ "po_line_id": line_id, "qty_received": 10 }] }),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND, "body={json}");
    // Tenant A's PO untouched.
    let (count, _) = movement_sum(&t.db, &tenant_a, "purchase_receive").await;
    assert_eq!(count, 0);
}

#[tokio::test]
async fn over_receipt_beyond_ordered_qty_is_conflict() {
    let t = spawn_test_db().await;
    let tenant = create_tenant(&t.db, "acme").await;
    let (po_id, line_id, _pid) =
        seed_po_single_line(&t.db, &tenant, "Metformina", Some("40"), 10, "55").await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant, "admin");
    let (status, json) = http_receive(
        app,
        &po_id,
        &token,
        serde_json::json!({ "lines": [{ "po_line_id": line_id, "qty_received": 11 }] }),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT, "body={json}");
    let (count, _) = movement_sum(&t.db, &tenant, "purchase_receive").await;
    assert_eq!(count, 0);
}

async fn http_send(app: axum::Router, po_id: &str, token: &str) -> (StatusCode, serde_json::Value) {
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/purchase-orders/{po_id}/send"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// BUG-bob-002 regression: a freshly created PO is `draft`; goods receipt only
/// accepts `sent`/`approved`/`partially_received`. `POST /send` is the missing
/// transition that makes the whole create→issue→receive lifecycle reachable
/// over HTTP. Here: send a draft → `sent`, then receive succeeds.
#[tokio::test]
async fn send_moves_draft_to_sent_then_receivable() {
    let t = spawn_test_db().await;
    let tenant = create_tenant(&t.db, "acme").await;
    let (po_id, line_id, pid) =
        seed_po_single_line(&t.db, &tenant, "Ibuprofeno", Some("80"), 12, "120").await;
    // seed helper leaves the PO `sent`; force back to `draft` to model a fresh
    // create and exercise the real `/send` route.
    set_po_status(&t.db, &po_id, "draft").await;
    set_stock(&t.db, &pid, 0).await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant, "admin");

    let (status, json) = http_send(app, &po_id, &token).await;
    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["status"], "sent");

    // Now receivable.
    let app = api::build_router(state_with_db(t.db.clone()));
    let (rstatus, rjson) = http_receive(
        app,
        &po_id,
        &token,
        serde_json::json!({ "lines": [{ "po_line_id": line_id, "qty_received": 12 }] }),
    )
    .await;
    assert_eq!(rstatus, StatusCode::OK, "body={rjson}");
    assert_eq!(rjson["status"], "received");
    let (stock, _cost) = product_stock_cost(&t.db, &pid).await;
    assert_eq!(stock, 12);
}

#[tokio::test]
async fn send_refuses_non_draft_po() {
    let t = spawn_test_db().await;
    let tenant = create_tenant(&t.db, "acme").await;
    // seed helper leaves the PO already `sent`.
    let (po_id, _line_id, _pid) =
        seed_po_single_line(&t.db, &tenant, "Loratadina", Some("50"), 5, "70").await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant, "admin");
    let (status, json) = http_send(app, &po_id, &token).await;
    assert_eq!(status, StatusCode::CONFLICT, "body={json}");
    assert_eq!(json["error"]["code"], "CONFLICT");
}

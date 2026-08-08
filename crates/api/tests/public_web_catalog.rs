//! Free Web PR1 — public storefront catalog (`/api/v1/public/{slug}/…`).
//!
//! Safety contract under test (ADR-0020):
//! * 404 darkness: `web.published != "true"` → uniform 404 on every route.
//! * Public JSON never carries cost/margin fields nor integer stock.
//! * Prices are decimal strings.
//! * Tenant isolation + per-SKU opt-in (`online_visible`) + OTC-only
//!   (`prescription_type = 'direct'`).

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

/// Direct-DB product seed with the public-web knobs. Price/cost as SurrealQL
/// decimal casts; everything else through binds.
#[allow(clippy::too_many_arguments)]
async fn seed_product(
    db: &db::Db,
    tenant: &Thing,
    name: &str,
    slug: &str,
    stock: i64,
    active: bool,
    online_visible: bool,
    prescription_type: &str,
) {
    db.query(
        "CREATE product SET tenant = $t, name = $name, slug = $slug, \
         price = <decimal> 1990, cost_price = <decimal> 900, stock = $stock, \
         active = $active, online_visible = $vis, prescription_type = $ptype",
    )
    .bind(("t", tenant.clone()))
    .bind(("name", name.to_string()))
    .bind(("slug", slug.to_string()))
    .bind(("stock", stock))
    .bind(("active", active))
    .bind(("vis", online_visible))
    .bind(("ptype", prescription_type.to_string()))
    .await
    .expect("create product")
    .check()
    .expect("product row");
}

async fn publish_web(db: &db::Db, tenant: &Thing, store_name: &str) {
    domain::sales::service::set_setting(db, tenant, "web.published", "true")
        .await
        .expect("set web.published");
    domain::sales::service::set_setting(db, tenant, "web.store_name", store_name)
        .await
        .expect("set web.store_name");
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let res = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// Recursive walk: fail if any object key contains `needle` (case-insensitive).
fn assert_no_key_contains(v: &serde_json::Value, needle: &str) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map {
                assert!(
                    !k.to_lowercase().contains(needle),
                    "public JSON leaked key '{k}'"
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

// ---------------------------------------------------------------------------

#[tokio::test]
async fn unpublished_returns_404_on_all_three_routes() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    seed_product(
        &t.db,
        &tenant,
        "Paracetamol",
        "paracetamol",
        10,
        true,
        true,
        "direct",
    )
    .await;
    // No web.published setting at all → darkness.
    let app = api::build_router(state_with_db(t.db.clone()));

    for uri in [
        "/api/v1/public/demo/store",
        "/api/v1/public/demo/catalog",
        "/api/v1/public/demo/catalog/paracetamol",
    ] {
        let (status, json) = get_json(&app, uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "expected 404 for {uri}");
        assert_eq!(json["error"]["code"], "NOT_FOUND", "envelope for {uri}");
    }

    // Explicit "false" is equally dark.
    domain::sales::service::set_setting(&t.db, &tenant, "web.published", "false")
        .await
        .unwrap();
    let (status, _) = get_json(&app, "/api/v1/public/demo/store").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Unknown tenant slug: same 404, no enumeration.
    let (status, _) = get_json(&app, "/api/v1/public/nope/store").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn published_catalog_lists_only_active_and_visible() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    seed_product(&t.db, &tenant, "Visible", "a", 10, true, true, "direct").await;
    seed_product(&t.db, &tenant, "Not online", "b", 10, true, false, "direct").await;
    seed_product(&t.db, &tenant, "Inactive", "c", 10, false, true, "direct").await;
    publish_web(&t.db, &tenant, "Farmacia Demo").await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (status, json) = get_json(&app, "/api/v1/public/demo/catalog").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["store"]["name"], "Farmacia Demo");
    assert_eq!(json["store"]["slug"], "demo");
    assert_eq!(json["store"]["currency"], "CLP");
    assert_eq!(json["store"]["pickup_enabled"], true);

    let items = json["items"].as_array().expect("items array");
    assert_eq!(items.len(), 1, "only A is active+visible: {items:?}");
    assert_eq!(items[0]["slug"], "a");
    assert_eq!(items[0]["availability"], "in_stock");
    // Page not full → no next_offset.
    assert!(json["next_offset"].is_null());
}

#[tokio::test]
async fn public_json_has_no_cost_fields() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    seed_product(&t.db, &tenant, "Visible", "a", 10, true, true, "direct").await;
    publish_web(&t.db, &tenant, "Farmacia Demo").await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (status, page) = get_json(&app, "/api/v1/public/demo/catalog").await;
    assert_eq!(status, StatusCode::OK);
    assert_no_key_contains(&page, "cost");
    assert_no_key_contains(&page, "margin");
    // Integer stock count must not appear either — only the availability bucket.
    assert_no_key_contains(&page, "stock");

    let (status, detail) = get_json(&app, "/api/v1/public/demo/catalog/a").await;
    assert_eq!(status, StatusCode::OK);
    assert_no_key_contains(&detail, "cost");
    assert_no_key_contains(&detail, "stock");
}

#[tokio::test]
async fn price_clp_is_string() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    seed_product(&t.db, &tenant, "Visible", "a", 10, true, true, "direct").await;
    publish_web(&t.db, &tenant, "Farmacia Demo").await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (status, json) = get_json(&app, "/api/v1/public/demo/catalog").await;
    assert_eq!(status, StatusCode::OK);
    let price = &json["items"][0]["price_clp"];
    assert!(price.is_string(), "price_clp must be a string: {price:?}");
    assert_eq!(price, "1990");
}

#[tokio::test]
async fn tenant_isolation() {
    let t = spawn_test_db().await;
    let ta = seed_tenant(&t.db, "alpha").await;
    let tb = seed_tenant(&t.db, "beta").await;
    seed_product(
        &t.db,
        &ta,
        "Alpha item",
        "alpha-item",
        10,
        true,
        true,
        "direct",
    )
    .await;
    seed_product(
        &t.db,
        &tb,
        "Beta item",
        "beta-item",
        10,
        true,
        true,
        "direct",
    )
    .await;
    publish_web(&t.db, &ta, "Alpha Store").await;
    publish_web(&t.db, &tb, "Beta Store").await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (status, json) = get_json(&app, "/api/v1/public/beta/catalog").await;
    assert_eq!(status, StatusCode::OK);
    let slugs: Vec<&str> = json["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["slug"].as_str().unwrap())
        .collect();
    assert_eq!(slugs, ["beta-item"], "beta must never see alpha products");

    // Alpha's product is 404 through beta's storefront.
    let (status, _) = get_json(&app, "/api/v1/public/beta/catalog/alpha-item").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn product_detail_by_slug_and_404_when_hidden() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    seed_product(&t.db, &tenant, "Visible", "a", 3, true, true, "direct").await;
    seed_product(&t.db, &tenant, "Not online", "b", 10, true, false, "direct").await;
    publish_web(&t.db, &tenant, "Farmacia Demo").await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (status, json) = get_json(&app, "/api/v1/public/demo/catalog/a").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["slug"], "a");
    assert_eq!(json["name"], "Visible");
    // stock 3 <= default low threshold 5 → "low".
    assert_eq!(json["availability"], "low");

    let (status, json) = get_json(&app, "/api/v1/public/demo/catalog/b").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn prescription_products_never_reach_public_catalog() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    seed_product(&t.db, &tenant, "OTC", "otc", 10, true, true, "direct").await;
    // Visible + active but requires prescription → must stay invisible.
    seed_product(
        &t.db,
        &tenant,
        "Controlado",
        "controlado",
        10,
        true,
        true,
        "receta",
    )
    .await;
    publish_web(&t.db, &tenant, "Farmacia Demo").await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (status, json) = get_json(&app, "/api/v1/public/demo/catalog").await;
    assert_eq!(status, StatusCode::OK);
    let slugs: Vec<&str> = json["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["slug"].as_str().unwrap())
        .collect();
    assert_eq!(slugs, ["otc"]);

    let (status, _) = get_json(&app, "/api/v1/public/demo/catalog/controlado").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn online_overrides_and_pagination() {
    let t = spawn_test_db().await;
    let tenant = seed_tenant(&t.db, "demo").await;
    seed_product(&t.db, &tenant, "Interno", "a", 10, true, true, "direct").await;
    // Web-facing overrides: title + price.
    t.db.query(
        "UPDATE product SET online_title = 'Bonito nombre web', \
         online_price = <decimal> 1490 WHERE tenant = $t AND slug = 'a'",
    )
    .bind(("t", tenant.clone()))
    .await
    .unwrap()
    .check()
    .unwrap();
    seed_product(&t.db, &tenant, "Otro", "b", 10, true, true, "direct").await;
    publish_web(&t.db, &tenant, "Farmacia Demo").await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (status, json) = get_json(&app, "/api/v1/public/demo/catalog?limit=1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["items"].as_array().unwrap().len(), 1);
    // Full page → next_offset advances by limit.
    assert_eq!(json["next_offset"], 1);

    let (_, detail) = get_json(&app, "/api/v1/public/demo/catalog/a").await;
    assert_eq!(detail["name"], "Bonito nombre web");
    assert_eq!(detail["price_clp"], "1490");
}

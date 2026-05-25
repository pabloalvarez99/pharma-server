//! Integration tests for the executive dashboard aggregate
//! (`GET /api/v1/reports/dashboard`). Spins up an isolated tempdir SurrealKv,
//! seeds tenants/users/orders/products, and asserts the single-call overview.
//!
//! Decimal money fields are emitted as JSON strings (project convention), so
//! all numeric money assertions compare against string literals.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use license::schema::{License, Tier, SCHEMA_VERSION};
use pharma_core::config::{DbConfig, JwtConfig};
use surrealdb::sql::Thing;
use tempfile::TempDir;
use tower::ServiceExt;
use uuid::Uuid;

const MIGRATIONS_DIR: &str = "../../migrations";

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret-test-secret-test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    }
}

fn pro_license() -> License {
    License {
        schema_version: SCHEMA_VERSION,
        license_id: "lic_test_pro".into(),
        tenant_id: Uuid::nil(),
        tier: Tier::Pro,
        features: vec!["reports.margins_daily".into()],
        bought_addons: Vec::new(),
        seat_count: 1,
        issued_at: Utc::now() - Duration::days(1),
        expires_at: Some(Utc::now() + Duration::days(30)),
        issuer_did: "did:pharma:test".into(),
        key_id: "lk-test".into(),
        signature: String::new(),
        metadata: None,
    }
}

struct Harness {
    app: axum::Router,
    handle: Arc<db::Db>,
    tenant: Thing,
    user: Thing,
    token: String,
    _tmp: TempDir,
}

/// Spawn a router backed by a fresh migrated DB with one tenant + admin user.
/// `license` selects Free vs Pro so the `margen_hoy` gate can be exercised.
async fn spawn(license: License) -> Harness {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("surreal");
    let cfg = DbConfig {
        path: db_path.to_string_lossy().into_owned(),
        namespace: "pharma_test".into(),
        database: "main".into(),
    };
    let handle = db::connect(&cfg).await.expect("connect");
    db::run_migrations(&handle, MIGRATIONS_DIR)
        .await
        .expect("migr");

    let mut r = handle
        .query("CREATE tenant SET name='T', slug='t' RETURN id")
        .await
        .unwrap();
    let tenant: Thing = r.take::<Option<_>>((0, "id")).unwrap().unwrap();
    let mut r = handle
        .query("CREATE user SET tenant=$t, email='a@t.l', password='x', roles=['admin'] RETURN id")
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let user: Thing = r.take::<Option<_>>((0, "id")).unwrap().unwrap();

    let jwt = jwt_cfg();
    let token = auth::issue(
        &jwt,
        &user.to_string(),
        &tenant.to_string(),
        vec!["admin".into()],
    )
    .unwrap();

    let handle = Arc::new(handle);
    let state = api::AppState {
        started_at: Utc::now(),
        jwt,
        db: Some(handle.clone()),
        metrics_token: None,
        node_identity: None,
        data_dir: Some(db_path.clone()),
        license: Arc::new(arc_swap::ArcSwap::from_pointee(license)),
        license_path: None,
    };
    Harness {
        app: api::build_router(state),
        handle,
        tenant,
        user,
        token,
        _tmp: tmp,
    }
}

async fn get_dashboard(app: &axum::Router, token: Option<&str>) -> (StatusCode, serde_json::Value) {
    let mut req = Request::builder().uri("/api/v1/reports/dashboard");
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let res = app
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// Seed a paid order today with two line items (one catalogued w/ cost, one
/// free-text). `total`/`subtotal`/cost are chosen so margin math is exact.
async fn seed_sale_today(h: &Harness) {
    // Catalogued product: price 1000, cost 600, stock 10.
    let mut r = h
        .handle
        .query(
            "CREATE product SET tenant=$t, name='Paracetamol', slug='paracetamol', \
             price=1000dec, cost_price=600dec, stock=10 RETURN id",
        )
        .bind(("t", h.tenant.clone()))
        .await
        .unwrap();
    let product: Thing = r.take::<Option<_>>((0, "id")).unwrap().unwrap();

    // Order: subtotal 2500 (2x1000 + 1x500), cash 1500, card 1000.
    let mut r = h
        .handle
        .query(
            "CREATE order SET tenant=$t, status='paid', payment_method='pos_mixed', \
             subtotal=2500dec, discount=0dec, total=2500dec, cash_amount=1500dec, \
             card_amount=1000dec, sold_by=$u RETURN id",
        )
        .bind(("t", h.tenant.clone()))
        .bind(("u", h.user.clone()))
        .await
        .unwrap();
    let order: Thing = r.take::<Option<_>>((0, "id")).unwrap().unwrap();

    h.handle
        .query(
            "CREATE order_item SET tenant=$t, order=$o, product=$p, \
             product_name='Paracetamol', quantity=2, unit_price=1000dec, subtotal=2000dec",
        )
        .bind(("t", h.tenant.clone()))
        .bind(("o", order.clone()))
        .bind(("p", product))
        .await
        .unwrap();
    h.handle
        .query(
            "CREATE order_item SET tenant=$t, order=$o, \
             product_name='Servicio', quantity=1, unit_price=500dec, subtotal=500dec",
        )
        .bind(("t", h.tenant.clone()))
        .bind(("o", order))
        .await
        .unwrap();
}

#[tokio::test]
async fn empty_tenant_returns_zeros_and_empty_arrays() {
    let h = spawn(License::free_default(Uuid::nil())).await;
    let (status, j) = get_dashboard(&h.app, Some(&h.token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(j["ventas_hoy"]["orders"], 0);
    assert_eq!(j["ventas_hoy"]["revenue"], "0");
    assert_eq!(j["ventas_hoy"]["cash"], "0");
    assert_eq!(j["ventas_hoy"]["card"], "0");
    assert_eq!(j["ventas_mes"]["orders"], 0);
    assert_eq!(j["ventas_mes"]["revenue"], "0");
    assert_eq!(j["inventario"]["total_skus"], 0);
    assert_eq!(j["inventario"]["active_skus"], 0);
    assert_eq!(j["inventario"]["low_stock"], 0);
    assert_eq!(j["inventario"]["out_of_stock"], 0);
    assert_eq!(j["inventario"]["value"], "0");
    assert_eq!(j["top_productos"], serde_json::json!([]));
    assert_eq!(j["por_vencer"], 0);
}

#[tokio::test]
async fn seeded_sale_today_reflected_in_ventas_hoy_and_top() {
    let h = spawn(pro_license()).await;
    seed_sale_today(&h).await;
    let (status, j) = get_dashboard(&h.app, Some(&h.token)).await;
    assert_eq!(status, StatusCode::OK);
    // One order; revenue = order.total = 2500; cash 1500 / card 1000.
    assert_eq!(j["ventas_hoy"]["orders"], 1);
    assert_eq!(j["ventas_hoy"]["revenue"], "2500");
    assert_eq!(j["ventas_hoy"]["cash"], "1500");
    assert_eq!(j["ventas_hoy"]["card"], "1000");
    // Month-to-date includes today's sale.
    assert_eq!(j["ventas_mes"]["orders"], 1);
    assert_eq!(j["ventas_mes"]["revenue"], "2500");
    // Top products: catalogued line (rev 2000) ranks above free-text (500).
    let top = j["top_productos"].as_array().unwrap();
    assert_eq!(top.len(), 2);
    assert_eq!(top[0]["product_name"], "Paracetamol");
    assert_eq!(top[0]["qty_sold"], 2);
    assert_eq!(top[0]["revenue"], "2000");
    // Full ranking fields the client consumes: rank, revenue_pct, abc_class.
    // 2000 of 2500 total = 80% cumulative → bucket A for the top row.
    assert_eq!(top[0]["rank"], 1);
    assert_eq!(top[0]["abc_class"], "A");
    assert!(top[0]["revenue_pct"].is_string());
}

#[tokio::test]
async fn seeded_products_inventory_counts_correct() {
    let h = spawn(License::free_default(Uuid::nil())).await;
    // active, healthy stock, cost 600 → value 6000 (10 * 600).
    h.handle
        .query(
            "CREATE product SET tenant=$t, name='A', slug='a', price=1000dec, \
             cost_price=600dec, stock=10",
        )
        .bind(("t", h.tenant.clone()))
        .await
        .unwrap();
    // active, low stock (<= default threshold) and not out of stock.
    h.handle
        .query("CREATE product SET tenant=$t, name='B', slug='b', price=500dec, stock=3")
        .bind(("t", h.tenant.clone()))
        .await
        .unwrap();
    // active, out of stock (stock 0 counts as low + out).
    h.handle
        .query("CREATE product SET tenant=$t, name='C', slug='c', price=500dec, stock=0")
        .bind(("t", h.tenant.clone()))
        .await
        .unwrap();
    // inactive — counted in total but not active.
    h.handle
        .query(
            "CREATE product SET tenant=$t, name='D', slug='d', price=500dec, \
             stock=50, active=false",
        )
        .bind(("t", h.tenant.clone()))
        .await
        .unwrap();

    let (status, j) = get_dashboard(&h.app, Some(&h.token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(j["inventario"]["total_skus"], 4);
    assert_eq!(j["inventario"]["active_skus"], 3);
    // low_stock counts active products at/below threshold (B + C).
    assert_eq!(j["inventario"]["low_stock"], 2);
    assert_eq!(j["inventario"]["out_of_stock"], 1);
    // value = sum(stock * cost) = 10*600 only (others have no cost_price).
    assert_eq!(j["inventario"]["value"], "6000");
}

#[tokio::test]
async fn free_tier_margen_hoy_is_null() {
    let h = spawn(License::free_default(Uuid::nil())).await;
    seed_sale_today(&h).await;
    let (status, j) = get_dashboard(&h.app, Some(&h.token)).await;
    // Whole endpoint still 200 (degrade the one field, never 402).
    assert_eq!(status, StatusCode::OK);
    assert!(
        j["margen_hoy"].is_null(),
        "expected null, got {}",
        j["margen_hoy"]
    );
    // The rest of the overview is unaffected.
    assert_eq!(j["ventas_hoy"]["orders"], 1);
}

#[tokio::test]
async fn pro_tier_margen_hoy_populated() {
    let h = spawn(pro_license()).await;
    seed_sale_today(&h).await;
    let (status, j) = get_dashboard(&h.app, Some(&h.token)).await;
    assert_eq!(status, StatusCode::OK);
    // revenue = 2500 (2000 + 500); cost = 2 * 600 = 1200 (free-text line has
    // no product cost). margin_pct = (2500-1200)/2500*100 = 52.00.
    assert_eq!(j["margen_hoy"]["revenue"], "2500");
    assert_eq!(j["margen_hoy"]["cost"], "1200");
    assert_eq!(j["margen_hoy"]["margin_pct"], "52.00");
}

#[tokio::test]
async fn cross_tenant_isolation() {
    // Tenant A has a sale + products; tenant B (different JWT) sees nothing.
    let h = spawn(pro_license()).await;
    seed_sale_today(&h).await;

    // Second tenant in the same DB.
    let mut r = h
        .handle
        .query("CREATE tenant SET name='Other', slug='other' RETURN id")
        .await
        .unwrap();
    let tenant_b: Thing = r.take::<Option<_>>((0, "id")).unwrap().unwrap();
    let mut r = h
        .handle
        .query("CREATE user SET tenant=$t, email='b@t.l', password='x', roles=['admin'] RETURN id")
        .bind(("t", tenant_b.clone()))
        .await
        .unwrap();
    let user_b: Thing = r.take::<Option<_>>((0, "id")).unwrap().unwrap();
    let token_b = auth::issue(
        &jwt_cfg(),
        &user_b.to_string(),
        &tenant_b.to_string(),
        vec!["admin".into()],
    )
    .unwrap();

    let (status, j) = get_dashboard(&h.app, Some(&token_b)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(j["ventas_hoy"]["orders"], 0);
    assert_eq!(j["ventas_hoy"]["revenue"], "0");
    assert_eq!(j["inventario"]["total_skus"], 0);
    assert_eq!(j["top_productos"], serde_json::json!([]));
    // Pro license is shared, so the gate still opens — but with no data the
    // margin aggregate is all zeros, never tenant A's figures.
    assert_eq!(j["margen_hoy"]["revenue"], "0");
}

#[tokio::test]
async fn unauthenticated_returns_401() {
    let h = spawn(License::free_default(Uuid::nil())).await;
    let (status, _) = get_dashboard(&h.app, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn cajero_role_is_forbidden() {
    // The dashboard surfaces revenue/margin: cajero is excluded by the role
    // gate even with a valid token for the right tenant.
    let h = spawn(License::free_default(Uuid::nil())).await;
    let cajero = auth::issue(
        &jwt_cfg(),
        &h.user.to_string(),
        &h.tenant.to_string(),
        vec!["cajero".into()],
    )
    .unwrap();
    let (status, j) = get_dashboard(&h.app, Some(&cajero)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(j["error"]["code"], "FORBIDDEN");
}

//! Tests for the nightly near-expiry alert scan.
//!
//! Uses SurrealDB `kv-mem` (schemaless) to avoid SurrealKV file locks — same
//! pattern as `crates/dte/tests/caf_folio_atomic.rs`. Batches are seeded with
//! relative expiry via SurrealQL `time::now() + Nd`, matching the live
//! `product_batch` field shape the scan reads (`tenant, product, batch_code,
//! expiry_date, stock, active`).

use jobs::{run_near_expiry_scan, NearExpiryAlert};
use surrealdb::engine::local::{Db, Mem};
use surrealdb::Surreal;

/// Fresh in-memory db with one tenant `tenant:t1` and a product `product:p1`.
async fn setup() -> Surreal<Db> {
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    db.query("CREATE tenant:t1 SET name = 'farmacia test'; CREATE product:p1 SET tenant = tenant:t1, name = 'P';")
        .await
        .unwrap()
        .check()
        .unwrap();
    db
}

/// Seed one batch. `expiry` is a raw SurrealQL datetime expression, e.g.
/// `"time::now() + 10d"` or `"time::now() - 5d"`.
async fn seed_batch(
    db: &Surreal<Db>,
    tenant: &str,
    product: &str,
    code: &str,
    expiry: &str,
    stock: i64,
    active: bool,
) {
    let q = format!(
        "CREATE product_batch SET tenant = {tenant}, product = {product}, \
         batch_code = $code, expiry_date = {expiry}, stock = $stock, active = $active;"
    );
    db.query(q)
        .bind(("code", code.to_string()))
        .bind(("stock", stock))
        .bind(("active", active))
        .await
        .unwrap()
        .check()
        .unwrap();
}

fn codes(alerts: &[NearExpiryAlert]) -> Vec<&str> {
    alerts.iter().map(|a| a.lot.as_str()).collect()
}

#[tokio::test]
async fn within_30d_filters_and_sorts_by_days_asc() {
    let db = setup().await;
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "SOON",
        "time::now() + 10d",
        5,
        true,
    )
    .await;
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "EDGE",
        "time::now() + 25d",
        3,
        true,
    )
    .await;
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "FAR",
        "time::now() + 200d",
        9,
        true,
    )
    .await;

    let alerts = run_near_expiry_scan(&db, 30).await.unwrap();

    // FAR (200d) excluded; SOON before EDGE (sorted by days_to_expiry asc).
    assert_eq!(codes(&alerts), vec!["SOON", "EDGE"]);
    assert!(alerts[0].days_to_expiry <= alerts[1].days_to_expiry);
    assert_eq!(alerts[0].stock, 5);
    assert_eq!(alerts[0].sku, "product:p1");
    assert_eq!(alerts[0].tenant, "tenant:t1");
    assert!((9..=10).contains(&alerts[0].days_to_expiry));
}

#[tokio::test]
async fn expired_lot_included_with_negative_days() {
    let db = setup().await;
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "EXPIRED",
        "time::now() - 5d",
        2,
        true,
    )
    .await;
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "SOON",
        "time::now() + 3d",
        4,
        true,
    )
    .await;

    let alerts = run_near_expiry_scan(&db, 30).await.unwrap();

    // Already-expired sorts first (most negative days_to_expiry).
    assert_eq!(codes(&alerts), vec!["EXPIRED", "SOON"]);
    assert!(alerts[0].days_to_expiry < 0, "expired => negative days");
    assert_eq!(alerts[0].lot, "EXPIRED");
}

#[tokio::test]
async fn zero_stock_lot_excluded() {
    let db = setup().await;
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "ZERO",
        "time::now() + 1d",
        0,
        true,
    )
    .await;
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "HAS",
        "time::now() + 1d",
        7,
        true,
    )
    .await;

    let alerts = run_near_expiry_scan(&db, 30).await.unwrap();

    assert_eq!(codes(&alerts), vec!["HAS"], "stock=0 batch excluded");
}

#[tokio::test]
async fn inactive_lot_excluded() {
    let db = setup().await;
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "INACTIVE",
        "time::now() + 1d",
        7,
        false,
    )
    .await;
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "ACTIVE",
        "time::now() + 1d",
        7,
        true,
    )
    .await;

    let alerts = run_near_expiry_scan(&db, 30).await.unwrap();

    assert_eq!(
        codes(&alerts),
        vec!["ACTIVE"],
        "active=false batch excluded"
    );
}

#[tokio::test]
async fn multi_tenant_scan_returns_all_tenants_distinctly() {
    let db = setup().await;
    db.query(
        "CREATE tenant:t2 SET name = 'otra'; CREATE product:p2 SET tenant = tenant:t2, name = 'Q';",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "A_SOON",
        "time::now() + 5d",
        5,
        true,
    )
    .await;
    seed_batch(
        &db,
        "tenant:t2",
        "product:p2",
        "B_SOON",
        "time::now() + 6d",
        8,
        true,
    )
    .await;

    let alerts = run_near_expiry_scan(&db, 30).await.unwrap();

    // Each alert is tagged with its own tenant; no bleed between tenants.
    assert_eq!(alerts.len(), 2);
    let a = alerts.iter().find(|x| x.lot == "A_SOON").unwrap();
    let b = alerts.iter().find(|x| x.lot == "B_SOON").unwrap();
    assert_eq!(a.tenant, "tenant:t1");
    assert_eq!(a.sku, "product:p1");
    assert_eq!(b.tenant, "tenant:t2");
    assert_eq!(b.sku, "product:p2");
}

#[tokio::test]
async fn empty_db_returns_empty_vec() {
    let db = setup().await;
    let alerts = run_near_expiry_scan(&db, 30).await.unwrap();
    assert!(alerts.is_empty(), "no batches => empty vec, no error");
}

#[tokio::test]
async fn within_days_window_widens_to_pull_far_lots() {
    let db = setup().await;
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "SOON",
        "time::now() + 10d",
        5,
        true,
    )
    .await;
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "FAR",
        "time::now() + 200d",
        9,
        true,
    )
    .await;

    // 30d window: only SOON.
    let narrow = run_near_expiry_scan(&db, 30).await.unwrap();
    assert_eq!(codes(&narrow), vec!["SOON"]);

    // 365d window: SOON + FAR, still sorted asc.
    let wide = run_near_expiry_scan(&db, 365).await.unwrap();
    assert_eq!(codes(&wide), vec!["SOON", "FAR"]);
}

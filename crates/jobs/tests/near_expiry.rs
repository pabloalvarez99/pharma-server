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

// --- lotes por sucursal (migración 0042) -----------------------------------

/// Seed a batch that lives in a given branch (`None` = casa matriz).
async fn seed_batch_in(
    db: &Surreal<Db>,
    tenant: &str,
    product: &str,
    branch: Option<&str>,
    code: &str,
    expiry: &str,
    stock: i64,
) {
    let br = branch.unwrap_or("NONE");
    let q = format!(
        "CREATE product_batch SET tenant = {tenant}, product = {product}, branch = {br}, \
         batch_code = $code, expiry_date = {expiry}, stock = $stock, active = true;"
    );
    db.query(q)
        .bind(("code", code.to_string()))
        .bind(("stock", stock))
        .await
        .unwrap()
        .check()
        .unwrap();
}

#[tokio::test]
async fn cada_alerta_trae_la_sucursal_del_lote() {
    let db = setup().await;
    db.query("CREATE branch:a SET tenant = tenant:t1, name = 'Local A'; CREATE branch:b SET tenant = tenant:t1, name = 'Local B';")
        .await
        .unwrap()
        .check()
        .unwrap();
    // Mismo producto, misma ventana: lo único que los distingue es el local.
    seed_batch_in(
        &db,
        "tenant:t1",
        "product:p1",
        Some("branch:a"),
        "EN_A",
        "time::now() + 5d",
        3,
    )
    .await;
    seed_batch_in(
        &db,
        "tenant:t1",
        "product:p1",
        Some("branch:b"),
        "EN_B",
        "time::now() + 6d",
        4,
    )
    .await;

    let alerts = run_near_expiry_scan(&db, 30).await.unwrap();

    assert_eq!(alerts.len(), 2);
    let a = alerts.iter().find(|x| x.lot == "EN_A").unwrap();
    let b = alerts.iter().find(|x| x.lot == "EN_B").unwrap();
    assert_eq!(
        a.branch.as_deref(),
        Some("branch:a"),
        "la alerta del lote de A dice A"
    );
    assert_eq!(
        b.branch.as_deref(),
        Some("branch:b"),
        "la alerta del lote de B dice B"
    );
}

#[tokio::test]
async fn lote_sin_sucursal_cae_en_casa_matriz() {
    let db = setup().await;
    // Un negocio de un solo local (o un instalado pre-0042) no estampa `branch`:
    // la alerta tiene que salir igual, con `None` = casa matriz, sin inventar
    // una sucursal.
    seed_batch(
        &db,
        "tenant:t1",
        "product:p1",
        "SIN_LOCAL",
        "time::now() + 3d",
        2,
        true,
    )
    .await;

    let alerts = run_near_expiry_scan(&db, 30).await.unwrap();

    assert_eq!(codes(&alerts), vec!["SIN_LOCAL"]);
    assert_eq!(alerts[0].branch, None, "sin branch = casa matriz");
}

#[tokio::test]
async fn dos_locales_del_mismo_tenant_no_se_mezclan_en_el_conteo() {
    let db = setup().await;
    db.query("CREATE branch:a SET tenant = tenant:t1, name = 'Local A'; CREATE branch:b SET tenant = tenant:t1, name = 'Local B';")
        .await
        .unwrap()
        .check()
        .unwrap();
    seed_batch_in(
        &db,
        "tenant:t1",
        "product:p1",
        Some("branch:a"),
        "A1",
        "time::now() + 2d",
        1,
    )
    .await;
    seed_batch_in(
        &db,
        "tenant:t1",
        "product:p1",
        Some("branch:a"),
        "A2",
        "time::now() + 9d",
        1,
    )
    .await;
    seed_batch_in(
        &db,
        "tenant:t1",
        "product:p1",
        Some("branch:b"),
        "B1",
        "time::now() + 4d",
        1,
    )
    .await;

    let alerts = run_near_expiry_scan(&db, 30).await.unwrap();

    // El digest agrupa por (tenant, local): 2 grupos, 2 lotes en A y 1 en B.
    // Se verifica sobre las alertas (el log no es observable desde el test).
    let en_a = alerts
        .iter()
        .filter(|x| x.branch.as_deref() == Some("branch:a"))
        .count();
    let en_b = alerts
        .iter()
        .filter(|x| x.branch.as_deref() == Some("branch:b"))
        .count();
    assert_eq!((en_a, en_b), (2, 1));
    // Orden global intacto: el más urgente primero, sea de donde sea.
    assert_eq!(codes(&alerts), vec!["A1", "B1", "A2"]);
}

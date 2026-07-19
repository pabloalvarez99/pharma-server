//! Catalog integration tests on an in-memory SurrealDB (`kv-mem`).
//! Each test gets an isolated db + the real migrations applied.

use domain::catalog::{model::*, service};
use rust_decimal::Decimal;
use std::str::FromStr;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

type Db = Surreal<surrealdb::engine::local::Db>;

async fn setup() -> (Db, Thing) {
    let db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations)
        .await
        .expect("migrations apply");
    let mut r = db
        .query("CREATE tenant SET name = 'Farmacia Test', slug = 'test' RETURN id")
        .await
        .unwrap();
    let id: Option<Thing> = r.take((0, "id")).unwrap();
    (db, id.expect("tenant id"))
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn new_product(name: &str, price: &str) -> NewProduct {
    NewProduct {
        name: name.into(),
        slug: None,
        description: None,
        price: dec(price),
        cost_price: None,
        stock: 0,
        category: None,
        image_url: None,
        external_id: None,
        laboratory: None,
        therapeutic_action: None,
        active_ingredient: None,
        prescription_type: None,
        presentation: None,
        discount_percent: None,
        attrs: None,
    }
}

#[tokio::test]
async fn create_autogenerates_unique_slug() {
    let (db, t) = setup().await;
    let a = service::create_product(&db, &t, new_product("Paracetamol 500", "1990"))
        .await
        .unwrap();
    assert_eq!(a.slug, "paracetamol-500");
    assert_eq!(a.price, dec("1990"));
    assert!(a.active);

    // same name -> slug collision -> suffixed
    let b = service::create_product(&db, &t, new_product("Paracetamol 500", "2100"))
        .await
        .unwrap();
    assert_eq!(b.slug, "paracetamol-500-2");
}

#[tokio::test]
async fn decimal_round_trips_through_db() {
    let (db, t) = setup().await;
    let mut p = new_product("Test", "12345.67");
    p.cost_price = Some(dec("9999.01"));
    let created = service::create_product(&db, &t, p).await.unwrap();
    let fetched = service::get_product(&db, &t, &created.id).await.unwrap();
    assert_eq!(fetched.price, dec("12345.67"));
    assert_eq!(fetched.cost_price, Some(dec("9999.01")));
    // JSON serializes money as string
    let v = serde_json::to_value(&fetched).unwrap();
    assert_eq!(v["price"], "12345.67");
    assert_eq!(v["cost_price"], "9999.01");
}

#[tokio::test]
async fn filters_and_soft_delete() {
    let (db, t) = setup().await;
    let mut low = new_product("Lowstock", "100");
    low.stock = 2;
    service::create_product(&db, &t, low).await.unwrap();
    let mut hi = new_product("Highstock", "100");
    hi.stock = 50;
    let hi = service::create_product(&db, &t, hi).await.unwrap();

    let low_only = service::list_products(
        &db,
        &t,
        ProductFilters {
            low_stock: Some(5),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(low_only.len(), 1);
    assert_eq!(low_only[0].name, "Lowstock");

    service::delete_product(&db, &t, &hi.id).await.unwrap();
    let after = service::get_product(&db, &t, &hi.id).await.unwrap();
    assert!(!after.active, "soft delete keeps row, flips active");
}

#[tokio::test]
async fn bulk_price_percent_and_amount() {
    let (db, t) = setup().await;
    service::create_product(&db, &t, new_product("A", "1000"))
        .await
        .unwrap();
    service::create_product(&db, &t, new_product("B", "2000"))
        .await
        .unwrap();

    let n = service::bulk_price(
        &db,
        &t,
        BulkPrice {
            mode: BulkPriceMode::Percent,
            value: dec("10"),
            category: None,
            round: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(n, 2);
    let list = service::list_products(&db, &t, ProductFilters::default())
        .await
        .unwrap();
    let mut prices: Vec<Decimal> = list.iter().map(|p| p.price).collect();
    prices.sort();
    assert_eq!(prices, vec![dec("1100"), dec("2200")]);

    service::bulk_price(
        &db,
        &t,
        BulkPrice {
            mode: BulkPriceMode::Amount,
            value: dec("-50"),
            category: None,
            round: true,
        },
    )
    .await
    .unwrap();
    let list = service::list_products(&db, &t, ProductFilters::default())
        .await
        .unwrap();
    let mut prices: Vec<Decimal> = list.iter().map(|p| p.price).collect();
    prices.sort();
    assert_eq!(prices, vec![dec("1050"), dec("2150")]);
}

#[tokio::test]
async fn stats_aggregates() {
    let (db, t) = setup().await;
    let mut a = new_product("A", "100");
    a.stock = 0;
    a.cost_price = Some(dec("50"));
    service::create_product(&db, &t, a).await.unwrap();
    let mut b = new_product("B", "100");
    b.stock = 3;
    b.cost_price = Some(dec("10"));
    service::create_product(&db, &t, b).await.unwrap();

    let s = service::stats(&db, &t).await.unwrap();
    assert_eq!(s.total, 2);
    assert_eq!(s.active, 2);
    assert_eq!(s.out_of_stock, 1);
    assert_eq!(s.low_stock, 2);
    assert_eq!(s.inventory_value, dec("30")); // 0*50 + 3*10
    assert_eq!(s.expired, 0);
}

#[tokio::test]
async fn category_crud_and_product_link() {
    let (db, t) = setup().await;
    let cat = service::create_category(
        &db,
        &t,
        NewCategory {
            name: "Analgésicos".into(),
            slug: None,
            description: None,
            image_url: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(cat.slug, "analgesicos");

    let mut p = new_product("Ibuprofeno", "1500");
    p.category = Some(cat.id.clone());
    let prod = service::create_product(&db, &t, p).await.unwrap();
    assert_eq!(prod.category.as_deref(), Some(cat.id.as_str()));

    // invalid category id rejected
    let mut bad = new_product("X", "1");
    bad.category = Some("category:doesnotexist".into());
    let err = service::create_product(&db, &t, bad).await.unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");

    let list = service::list_products(
        &db,
        &t,
        ProductFilters {
            category: Some(cat.id.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(list.len(), 1);

    service::delete_category(&db, &t, &cat.id).await.unwrap();
    let after = service::get_category(&db, &t, &cat.id).await.unwrap();
    assert!(!after.active);
}

#[tokio::test]
async fn tenant_isolation() {
    let (db, t1) = setup().await;
    let mut r = db
        .query("CREATE tenant SET name = 'Otra', slug = 'otra' RETURN id")
        .await
        .unwrap();
    let t2: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();

    service::create_product(&db, &t1, new_product("Solo T1", "100"))
        .await
        .unwrap();
    let seen = service::list_products(&db, &t2, ProductFilters::default())
        .await
        .unwrap();
    assert!(seen.is_empty(), "tenant 2 must not see tenant 1 products");
}

// ---------------------------------------------------------------------------
// BUG-perf-001: product_stats pre-computed view (migration 0029) replaces the
// O(n) full-scan aggregate. These lock in that the maintained view returns the
// *exact same* numbers the scan did, across create/update/delete, per tenant,
// at scale, and on the upgrade (backfill) path.
// ---------------------------------------------------------------------------

async fn new_tenant(db: &Db, slug: &str) -> Thing {
    let mut r = db
        .query("CREATE tenant SET name = $n, slug = $s RETURN id")
        .bind(("n", slug.to_string()))
        .bind(("s", slug.to_string()))
        .await
        .unwrap();
    r.take::<Option<Thing>>((0, "id"))
        .unwrap()
        .expect("tenant id")
}

fn prod_with(name: &str, stock: i64, cost: &str) -> NewProduct {
    let mut p = new_product(name, "100");
    p.stock = stock;
    p.cost_price = Some(dec(cost));
    p
}

fn empty_update() -> UpdateProduct {
    UpdateProduct {
        name: None,
        description: None,
        price: None,
        cost_price: None,
        category: None,
        image_url: None,
        active: None,
        external_id: None,
        laboratory: None,
        therapeutic_action: None,
        active_ingredient: None,
        prescription_type: None,
        presentation: None,
        discount_percent: None,
        attrs: None,
    }
}

/// Bulk-insert active products `[start, end)` (stock 10, cost 2) for `tenant`
/// via a single raw INSERT — fast catalog fill that skips the per-row slug probe.
async fn bulk_insert_range(db: &Db, tenant: &Thing, start: usize, end: usize) {
    let tok = tenant.to_string();
    let mut rows = String::from("[");
    for i in start..end {
        if i > start {
            rows.push(',');
        }
        rows.push_str(&format!(
            "{{tenant:{tok},name:'P{i}',slug:'p{i}',price:100dec,\
             cost_price:2dec,stock:10,active:true}}"
        ));
    }
    rows.push(']');
    db.query(format!("INSERT INTO product {rows}"))
        .await
        .unwrap()
        .check()
        .unwrap();
}

/// Bulk-insert `n` active products (stock 10, cost 2) for `tenant`.
async fn bulk_insert(db: &Db, tenant: &Thing, n: usize) {
    bulk_insert_range(db, tenant, 0, n).await;
}

/// The baked low-stock threshold in migration 0029's view MUST match the const
/// the scan fallback uses, or the view and fallback would disagree silently.
#[test]
fn low_stock_threshold_matches_view() {
    assert_eq!(
        LOW_STOCK_DEFAULT, 5,
        "migration 0029 bakes `stock <= 5`; keep it in sync with LOW_STOCK_DEFAULT"
    );
}

/// The maintained view must return byte-for-byte the same aggregate the live
/// scan does over a mixed dataset (active/inactive, in/low/out of stock).
#[tokio::test]
async fn stats_view_matches_scan() {
    let (db, t) = setup().await;
    // in stock, active
    service::create_product(&db, &t, prod_with("In", 50, "10"))
        .await
        .unwrap();
    // low stock (<=5), active
    service::create_product(&db, &t, prod_with("Low", 3, "20"))
        .await
        .unwrap();
    // out of stock, active
    service::create_product(&db, &t, prod_with("Out", 0, "30"))
        .await
        .unwrap();
    // low stock but INACTIVE -> excluded from low/out, still counted in total
    // and inventory_value
    let inact = service::create_product(&db, &t, prod_with("Inact", 2, "40"))
        .await
        .unwrap();
    let mut patch = empty_update();
    patch.active = Some(false);
    service::update_product(&db, &t, &inact.id, patch)
        .await
        .unwrap();

    let view = service::stats(&db, &t).await.unwrap();
    let scan = domain::catalog::repo::stats_scan_for_test(&db, &t, LOW_STOCK_DEFAULT)
        .await
        .unwrap();

    assert_eq!(view.total, scan.total);
    assert_eq!(view.active, scan.active);
    assert_eq!(view.low_stock, scan.low_stock);
    assert_eq!(view.out_of_stock, scan.out_of_stock);
    assert_eq!(view.inventory_value, scan.inventory_value);
    // and the absolute expected numbers
    assert_eq!(view.total, 4);
    assert_eq!(view.active, 3);
    assert_eq!(view.low_stock, 2); // Low + Out
    assert_eq!(view.out_of_stock, 1); // Out
    assert_eq!(view.inventory_value, dec("640")); // 500+60+0+80
}

/// Stock edits must move the view incrementally and correctly across the
/// low/out-of-stock boundaries.
#[tokio::test]
async fn stats_incremental_on_stock_update() {
    let (db, t) = setup().await;
    let p = service::create_product(&db, &t, prod_with("P", 10, "10"))
        .await
        .unwrap();

    let s = service::stats(&db, &t).await.unwrap();
    assert_eq!(
        (s.low_stock, s.out_of_stock, s.inventory_value),
        (0, 0, dec("100"))
    );

    // drop to 3 -> crosses into low stock
    let mut adj = StockAdjust {
        set: Some(3),
        delta: None,
        reason: None,
    };
    service::adjust_stock(&db, &t, &p.id, adj, None)
        .await
        .unwrap();
    let s = service::stats(&db, &t).await.unwrap();
    assert_eq!(
        (s.low_stock, s.out_of_stock, s.inventory_value),
        (1, 0, dec("30"))
    );

    // drop to 0 -> out of stock
    adj = StockAdjust {
        set: Some(0),
        delta: None,
        reason: None,
    };
    service::adjust_stock(&db, &t, &p.id, adj, None)
        .await
        .unwrap();
    let s = service::stats(&db, &t).await.unwrap();
    assert_eq!(
        (s.low_stock, s.out_of_stock, s.inventory_value),
        (1, 1, dec("0"))
    );
}

/// Deactivating and soft-deleting must drop a product from active/low/out
/// counts while it stays in `total` and `inventory_value` (matches scan
/// semantics: those have no active filter).
#[tokio::test]
async fn stats_incremental_on_active_and_delete() {
    let (db, t) = setup().await;
    let a = service::create_product(&db, &t, prod_with("A", 3, "10"))
        .await
        .unwrap();
    let b = service::create_product(&db, &t, prod_with("B", 3, "10"))
        .await
        .unwrap();

    let s = service::stats(&db, &t).await.unwrap();
    assert_eq!((s.total, s.active, s.low_stock), (2, 2, 2));

    // deactivate A
    let mut patch = empty_update();
    patch.active = Some(false);
    service::update_product(&db, &t, &a.id, patch)
        .await
        .unwrap();
    let s = service::stats(&db, &t).await.unwrap();
    assert_eq!((s.total, s.active, s.low_stock), (2, 1, 1));
    assert_eq!(s.inventory_value, dec("60")); // both still counted (3*10*2)

    // soft-delete B (sets active=false)
    service::delete_product(&db, &t, &b.id).await.unwrap();
    let s = service::stats(&db, &t).await.unwrap();
    assert_eq!(
        (s.total, s.active, s.low_stock, s.out_of_stock),
        (2, 0, 0, 0)
    );
    assert_eq!(s.inventory_value, dec("60")); // inventory value ignores active
}

/// The view groups by tenant: each tenant sees only its own aggregate.
#[tokio::test]
async fn stats_multitenant_isolation() {
    let (db, t1) = setup().await;
    let t2 = new_tenant(&db, "otra").await;

    service::create_product(&db, &t1, prod_with("A", 10, "5"))
        .await
        .unwrap();
    service::create_product(&db, &t1, prod_with("B", 0, "5"))
        .await
        .unwrap();
    service::create_product(&db, &t2, prod_with("C", 10, "7"))
        .await
        .unwrap();

    let s1 = service::stats(&db, &t1).await.unwrap();
    let s2 = service::stats(&db, &t2).await.unwrap();
    assert_eq!(
        (s1.total, s1.out_of_stock, s1.inventory_value),
        (2, 1, dec("50"))
    );
    assert_eq!(
        (s2.total, s2.out_of_stock, s2.inventory_value),
        (1, 0, dec("70"))
    );
}

/// A tenant with no products returns zeros (view has no row -> scan fallback).
#[tokio::test]
async fn stats_empty_tenant_zeros() {
    let (db, t) = setup().await;
    let s = service::stats(&db, &t).await.unwrap();
    assert_eq!(
        (
            s.total,
            s.active,
            s.low_stock,
            s.out_of_stock,
            s.inventory_value,
            s.expired
        ),
        (0, 0, 0, 0, dec("0"), 0)
    );
}

/// Upgrade path: an install whose catalog predates this feature must read
/// correct numbers immediately — migration 0029 backfills the stats rows from
/// existing `product` data. Simulated by wiping the maintained rows (as if they
/// never existed) and re-running the migration's backfill statement.
#[tokio::test]
async fn stats_backfill_on_define() {
    let (db, t) = setup().await;
    bulk_insert(&db, &t, 20).await; // 20 active, stock 10, cost 2

    // wipe the maintained rows -> simulate products that predate the stats table
    db.query("DELETE product_stats")
        .await
        .unwrap()
        .check()
        .unwrap();
    // with no stats row the read falls back to the live scan -> still correct
    let pre = service::stats(&db, &t).await.unwrap();
    assert_eq!(
        pre.total, 20,
        "no stats row -> scan fallback sees the live rows"
    );

    // re-run the migration's backfill (FOR over the per-tenant aggregate)
    db.query(
        "FOR $r IN (SELECT tenant, count() AS total, count(active = true) AS active, \
             count(active = true AND stock <= 5) AS low_stock, \
             count(active = true AND stock <= 0) AS out_of_stock, \
             math::sum(stock * (cost_price ?? 0dec)) AS inventory_value \
           FROM product GROUP BY tenant) { \
             UPSERT type::thing('product_stats', meta::id($r.tenant)) SET \
               tenant = $r.tenant, total = $r.total, active = $r.active, \
               low_stock = $r.low_stock, out_of_stock = $r.out_of_stock, \
               inventory_value = ($r.inventory_value ?? 0dec); };",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    // the backfill must have created the maintained row (not just left the
    // scan fallback): assert the row exists directly.
    let mut r = db
        .query("SELECT VALUE total FROM product_stats WHERE tenant = $t")
        .bind(("t", t.clone()))
        .await
        .unwrap();
    let rows: Vec<i64> = r.take(0).unwrap();
    assert_eq!(rows, vec![20], "backfill populated the product_stats row");

    let s = service::stats(&db, &t).await.unwrap();
    assert_eq!(s.total, 20);
    assert_eq!(s.active, 20);
    assert_eq!(s.inventory_value, dec("400")); // 20 * 10 * 2
}

/// Correctness must hold at catalog scale (the O(n) cliff this fixes only shows
/// at size). 3 000 products; the view read stays exact. Absolute <50ms p99 at
/// 50k is proven by bob's pos_hotpath bench.
#[tokio::test]
async fn stats_large_dataset_correct() {
    let (db, t) = setup().await;
    bulk_insert(&db, &t, 3_000).await;

    let view = service::stats(&db, &t).await.unwrap();
    let scan = domain::catalog::repo::stats_scan_for_test(&db, &t, LOW_STOCK_DEFAULT)
        .await
        .unwrap();
    assert_eq!(view.total, 3_000);
    assert_eq!(view.active, 3_000);
    assert_eq!(view.total, scan.total);
    assert_eq!(view.inventory_value, scan.inventory_value);
    assert_eq!(view.inventory_value, dec("60000")); // 3000 * 10 * 2
}

/// Before/after proof for BUG-perf-001 at full catalog scale. Ignored (50k seed
/// + percentile pass is too slow for the default suite); run explicitly:
///   cargo test -p domain --test catalog -- --ignored --nocapture stats_perf
/// Asserts the maintained-view path stays under the <50ms POS budget at 50k,
/// where the old O(n) scan blew it (bob's bench: 2.7s p99). Prints both p99s.
#[tokio::test]
#[ignore]
async fn stats_perf_50k_view_vs_scan() {
    use std::time::Instant;
    let (db, t) = setup().await;
    let n = std::env::var("PHARMA_BENCH_PRODUCTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50_000usize);
    // seed in 1k chunks (bulk_insert builds one statement; keep memory sane)
    let mut done = 0;
    while done < n {
        let end = (done + 1000).min(n);
        bulk_insert_range(&db, &t, done, end).await;
        done = end;
    }

    let samples = std::env::var("PHARMA_BENCH_SAMPLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200usize);
    let p99 = |mut lat: Vec<std::time::Duration>| {
        lat.sort_unstable();
        lat[((samples as f64 * 0.99) as usize).min(samples - 1)]
    };

    let mut scan = Vec::with_capacity(samples);
    for _ in 0..samples {
        let s = Instant::now();
        let _ = domain::catalog::repo::stats_scan_for_test(&db, &t, LOW_STOCK_DEFAULT)
            .await
            .unwrap();
        scan.push(s.elapsed());
    }
    let mut view = Vec::with_capacity(samples);
    for _ in 0..samples {
        let s = Instant::now();
        let _ = service::stats(&db, &t).await.unwrap();
        view.push(s.elapsed());
    }
    let scan99 = p99(scan);
    let view99 = p99(view);
    eprintln!(
        "\n=== stock_stats_agg @ {n} SKUs (kv-mem) ===\n  OLD scan  p99 = {:>9.3}ms\n  NEW view  p99 = {:>9.3}ms\n  budget(<50ms p99) = {}\n",
        scan99.as_secs_f64() * 1e3,
        view99.as_secs_f64() * 1e3,
        if view99 <= std::time::Duration::from_millis(50) { "OK" } else { "*** OVER ***" }
    );
    assert!(
        view99 <= std::time::Duration::from_millis(50),
        "view path p99 {view99:?} over <50ms budget at {n} SKUs"
    );
    // sanity: results are identical at scale (same as stats_large_dataset_correct)
    let v = service::stats(&db, &t).await.unwrap();
    let sc = domain::catalog::repo::stats_scan_for_test(&db, &t, LOW_STOCK_DEFAULT)
        .await
        .unwrap();
    assert_eq!(v.total, sc.total);
    assert_eq!(v.inventory_value, sc.inventory_value);
}

// ---------------------------------------------------------------------------
// product.attrs persist on create/update (wire contract for rubro pack)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_and_update_persist_attrs() {
    let (db, t) = setup().await;
    let mut input = new_product("Polera basica", "9990");
    input.attrs = Some(serde_json::json!({
        "talla": "M",
        "color": "Negro",
        "sku": "POL-M-NEG"
    }));
    let created = service::create_product(&db, &t, input).await.unwrap();
    assert_eq!(
        created.attrs,
        Some(serde_json::json!({
            "talla": "M",
            "color": "Negro",
            "sku": "POL-M-NEG"
        }))
    );
    let fetched = service::get_product(&db, &t, &created.id).await.unwrap();
    assert_eq!(fetched.attrs, created.attrs);

    let mut patch = empty_update();
    patch.attrs = Some(serde_json::json!({"talla": "L", "color": "Azul"}));
    let updated = service::update_product(&db, &t, &created.id, patch)
        .await
        .unwrap();
    assert_eq!(
        updated.attrs,
        Some(serde_json::json!({"talla": "L", "color": "Azul"}))
    );

    // Omitted attrs on patch leaves previous value.
    let mut patch2 = empty_update();
    patch2.name = Some("Polera basica L".into());
    let renamed = service::update_product(&db, &t, &created.id, patch2)
        .await
        .unwrap();
    assert_eq!(renamed.name, "Polera basica L");
    assert_eq!(
        renamed.attrs,
        Some(serde_json::json!({"talla": "L", "color": "Azul"}))
    );
}

// ---------------------------------------------------------------------------
// Variantes multi-SKU (migración 0034 / Opción A)
// ---------------------------------------------------------------------------

fn new_variant(talla: &str, barcode: &str, stock: i64) -> NewVariant {
    NewVariant {
        name: None,
        slug: None,
        price: None,
        cost_price: None,
        stock,
        barcode: Some(barcode.into()),
        attrs: Some(serde_json::json!({"talla": talla})),
        external_id: None,
        image_url: None,
    }
}

#[tokio::test]
async fn create_and_list_variants_with_own_barcode_and_stock() {
    let (db, t) = setup().await;
    let parent = service::create_product(&db, &t, new_product("Polera basica", "9990"))
        .await
        .unwrap();
    assert!(parent.parent_id.is_none());

    let m = service::create_variant(&db, &t, &parent.id, new_variant("M", "7804999100011", 10))
        .await
        .unwrap();
    let l = service::create_variant(&db, &t, &parent.id, new_variant("L", "7804999100028", 5))
        .await
        .unwrap();

    assert_eq!(m.parent_id.as_deref(), Some(parent.id.as_str()));
    assert_eq!(l.parent_id.as_deref(), Some(parent.id.as_str()));
    assert_eq!(m.stock, 10);
    assert_eq!(l.stock, 5);
    assert_eq!(m.price, dec("9990"), "inherits parent price");
    assert!(m.name.contains("M"));

    let kids = service::list_variants(&db, &t, &parent.id).await.unwrap();
    assert_eq!(kids.len(), 2);

    let by_m = service::find_by_barcode(&db, &t, "7804999100011")
        .await
        .unwrap();
    assert_eq!(by_m.id, m.id);

    // Default list hides variants
    let top = service::list_products(&db, &t, ProductFilters::default())
        .await
        .unwrap();
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].id, parent.id);

    let all = service::list_products_with_variants(&db, &t, ProductFilters::default(), true)
        .await
        .unwrap();
    assert_eq!(all.len(), 3);
}

#[tokio::test]
async fn variant_rejects_nested_and_duplicate_barcode() {
    let (db, t) = setup().await;
    let parent = service::create_product(&db, &t, new_product("Jean", "15000"))
        .await
        .unwrap();
    let child = service::create_variant(&db, &t, &parent.id, new_variant("32", "7804999200018", 3))
        .await
        .unwrap();

    let err = service::create_variant(
        &db,
        &t,
        &child.id,
        new_variant("nested", "7804999200094", 1),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");

    let err = service::create_variant(&db, &t, &parent.id, new_variant("34", "7804999200018", 1))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
}

#[tokio::test]
async fn variant_tenant_isolation() {
    let (db, t1) = setup().await;
    let mut r = db
        .query("CREATE tenant SET name = 'Shop2', slug = 'shop2' RETURN id")
        .await
        .unwrap();
    let t2: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();

    let parent = service::create_product(&db, &t1, new_product("Camisa", "12000"))
        .await
        .unwrap();
    service::create_variant(&db, &t1, &parent.id, new_variant("S", "7804999300015", 4))
        .await
        .unwrap();

    let err = service::list_variants(&db, &t2, &parent.id)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "NOT_FOUND");

    let err = service::find_by_barcode(&db, &t2, "7804999300015")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "NOT_FOUND");
}

#[tokio::test]
async fn list_variants_ordered_by_name_with_stock_and_barcode() {
    let (db, t) = setup().await;
    // Parent sin barcode de caja (retail shell).
    let parent = service::create_product(&db, &t, new_product("Polera shell", "8990"))
        .await
        .unwrap();
    assert!(parent.barcode.is_none());

    // Create L before M so ORDER BY name is not insert-order.
    let l = service::create_variant(&db, &t, &parent.id, new_variant("L", "7804999110027", 3))
        .await
        .unwrap();
    let m = service::create_variant(&db, &t, &parent.id, new_variant("M", "7804999110010", 8))
        .await
        .unwrap();
    assert_eq!(m.barcode.as_deref(), Some("7804999110010"));
    assert_eq!(l.barcode.as_deref(), Some("7804999110027"));

    let kids = service::list_variants(&db, &t, &parent.id).await.unwrap();
    assert_eq!(kids.len(), 2);
    assert!(
        kids[0].name <= kids[1].name,
        "list must be ORDER BY name: {:?} then {:?}",
        kids[0].name,
        kids[1].name
    );
    let stocks: std::collections::HashMap<_, _> =
        kids.iter().map(|k| (k.id.as_str(), k.stock)).collect();
    assert_eq!(stocks.get(m.id.as_str()).copied(), Some(8));
    assert_eq!(stocks.get(l.id.as_str()).copied(), Some(3));
    for k in &kids {
        assert!(k.barcode.is_some(), "list enriches barcode for {}", k.id);
    }

    // Parent.stock stays 0; sellable stock is sum of children (read-side).
    let parent2 = service::get_product(&db, &t, &parent.id).await.unwrap();
    assert_eq!(parent2.stock, 0, "padre no materializa stock de hijos");
    assert_eq!(parent2.variants_stock, Some(11));
    let sum = service::variants_stock_sum(&db, &t, &parent.id)
        .await
        .unwrap();
    assert_eq!(sum, 11);
}

#[tokio::test]
async fn parent_without_barcode_and_whitespace_barcode_ok() {
    let (db, t) = setup().await;
    let parent = service::create_product(&db, &t, new_product("Jean shell", "19990"))
        .await
        .unwrap();
    // Empty / whitespace barcode = no mapping (allowed).
    let mut v = new_variant("32", "x", 2);
    v.barcode = Some("   ".into());
    let child = service::create_variant(&db, &t, &parent.id, v)
        .await
        .unwrap();
    assert!(child.barcode.is_none());
    let err = service::find_by_barcode(&db, &t, "   ").await.unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
}

#[tokio::test]
async fn create_variant_missing_parent_not_found() {
    let (db, t) = setup().await;
    let err = service::create_variant(
        &db,
        &t,
        "product:doesnotexist",
        new_variant("M", "7804999120016", 1),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "NOT_FOUND");
}

#[tokio::test]
async fn create_variant_barcode_race_second_conflicts() {
    let (db, t) = setup().await;
    let parent = service::create_product(&db, &t, new_product("Race shell", "5000"))
        .await
        .unwrap();
    let code = "7804999130013";
    let a = service::create_variant(&db, &t, &parent.id, new_variant("A", code, 1))
        .await
        .unwrap();
    assert_eq!(a.barcode.as_deref(), Some(code));

    // Second claim of same EAN → CONFLICT; no barcode steal.
    let err = service::create_variant(&db, &t, &parent.id, new_variant("B", code, 1))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
    let msg = err.to_string();
    assert!(
        msg.contains("código de barras") || msg.contains("ya está"),
        "ES conflict message: {msg}"
    );

    let owner = service::find_by_barcode(&db, &t, code).await.unwrap();
    assert_eq!(owner.id, a.id, "first winner keeps barcode");

    // Orphan from failed barcode hard-deleted — only winner remains.
    let kids = service::list_variants(&db, &t, &parent.id).await.unwrap();
    assert_eq!(
        kids.len(),
        1,
        "orphan from failed barcode should be removed"
    );
    assert_eq!(kids[0].id, a.id);
}

#[tokio::test]
async fn create_variant_concurrent_same_barcode_one_wins() {
    let (db, t) = setup().await;
    let parent = service::create_product(&db, &t, new_product("Concurrent shell", "7000"))
        .await
        .unwrap();
    let code = "7804999140010";
    let pid = parent.id.clone();

    let (r1, r2) = tokio::join!(
        service::create_variant(&db, &t, &pid, new_variant("A", code, 2)),
        service::create_variant(&db, &t, &pid, new_variant("B", code, 2)),
    );

    let ok = r1.is_ok() as u8 + r2.is_ok() as u8;
    let err_n = r1.is_err() as u8 + r2.is_err() as u8;
    // Never two Ok (would mean barcode steal / double-map).
    assert!(ok <= 1, "at most one winner: r1={r1:?} r2={r2:?}");
    if ok == 1 {
        assert_eq!(err_n, 1, "loser must conflict: r1={r1:?} r2={r2:?}");
        let loser = match (&r1, &r2) {
            (Err(e), Ok(_)) | (Ok(_), Err(e)) => e,
            _ => panic!("expected exactly one error: r1={r1:?} r2={r2:?}"),
        };
        assert_eq!(loser.code(), "CONFLICT");
        let o = service::find_by_barcode(&db, &t, code).await.unwrap();
        assert_eq!(o.barcode.as_deref(), Some(code));
    }

    let kids = service::list_variants(&db, &t, &parent.id).await.unwrap();
    assert!(
        kids.len() <= 1,
        "expected ≤1 variant after concurrent claim, got {}",
        kids.len()
    );
}

#[tokio::test]
async fn plain_pharmacy_sku_untouched_by_variants_stock_field() {
    // Farmacia path: no parent_id, no children → no variants_stock field.
    let (db, t) = setup().await;
    let p = service::create_product(&db, &t, {
        let mut n = new_product("Paracetamol 500mg", "1990");
        n.stock = 40;
        n
    })
    .await
    .unwrap();
    let got = service::get_product(&db, &t, &p.id).await.unwrap();
    assert!(got.parent_id.is_none());
    assert!(got.variants_stock.is_none());
    assert_eq!(got.stock, 40);
}

#[tokio::test]
async fn find_by_barcode_rejects_parent_shell_with_variants() {
    // Edge: parent somehow has a product_barcode row (import/UPSERT). Scan must
    // not resolve to the non-sellable shell — same ES contract as POS sale.
    let (db, t) = setup().await;
    let parent = service::create_product(&db, &t, new_product("Shell con EAN", "9990"))
        .await
        .unwrap();
    service::create_variant(&db, &t, &parent.id, new_variant("M", "7804999150017", 4))
        .await
        .unwrap();
    let parent_thing = surrealdb::sql::thing(&parent.id).unwrap();
    domain::catalog::repo::upsert_barcode(&db, &t, &parent_thing, "7804999150093")
        .await
        .unwrap();

    let err = service::find_by_barcode(&db, &t, "7804999150093")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
    let msg = err.to_string().to_lowercase();
    assert!(msg.contains("tiene variantes"), "{msg}");

    // Child barcode still resolves.
    let child = service::find_by_barcode(&db, &t, "7804999150017")
        .await
        .unwrap();
    assert_eq!(child.parent_id.as_deref(), Some(parent.id.as_str()));
}

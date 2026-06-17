//! Inventory integration tests on an in-memory SurrealDB (`kv-mem`).
//! Each test gets an isolated db + the real migrations applied.

use chrono::{Duration, Utc};
use domain::catalog::{model::*, service as catalog};
use domain::inventory::{model::*, service as inv};
use rust_decimal::Decimal;
use std::str::FromStr;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

type Db = Surreal<surrealdb::engine::local::Db>;

async fn setup() -> (Db, Thing, String) {
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
    let tenant: Option<Thing> = r.take((0, "id")).unwrap();
    let tenant = tenant.expect("tenant id");
    // Create an admin user so `stock_movement.admin` (record<user>) binds work.
    let mut r = db
        .query(
            "CREATE user SET tenant = $t, email = 'admin@test.local', \
             password = 'x', roles = ['admin'] RETURN id",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let admin: Option<Thing> = r.take((0, "id")).unwrap();
    (db, tenant, admin.expect("admin id").to_string())
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
        cost_price: Some(dec("100")),
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
    }
}

#[tokio::test]
async fn movement_materializes_product_stock_atomically() {
    let (db, t, admin) = setup().await;
    let p = catalog::create_product(&db, &t, new_product("A", "1000"))
        .await
        .unwrap();
    let (m, prod) = inv::add_movement(&db, &t, &p.id, 50, "manual_adjust", Some(&admin), None)
        .await
        .unwrap();
    assert_eq!(m.delta, 50);
    assert_eq!(m.reason, "manual_adjust");
    assert_eq!(prod.stock, 50);
    // Re-read confirms persistence.
    let again = catalog::get_product(&db, &t, &p.id).await.unwrap();
    assert_eq!(again.stock, 50);

    let (_m2, prod2) = inv::add_movement(&db, &t, &p.id, -20, "manual_adjust", Some(&admin), None)
        .await
        .unwrap();
    assert_eq!(prod2.stock, 30);
}

#[tokio::test]
async fn negative_resulting_stock_blocked() {
    let (db, t, admin) = setup().await;
    let p = catalog::create_product(&db, &t, new_product("A", "100"))
        .await
        .unwrap();
    inv::add_movement(&db, &t, &p.id, 5, "manual_adjust", Some(&admin), None)
        .await
        .unwrap();
    let err = inv::add_movement(&db, &t, &p.id, -10, "sale", Some(&admin), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INSUFFICIENT_STOCK");
    let prod = catalog::get_product(&db, &t, &p.id).await.unwrap();
    assert_eq!(prod.stock, 5, "rejected movement leaves stock untouched");
}

#[tokio::test]
async fn zero_delta_rejected() {
    let (db, t, admin) = setup().await;
    let p = catalog::create_product(&db, &t, new_product("A", "100"))
        .await
        .unwrap();
    let err = inv::add_movement(&db, &t, &p.id, 0, "noop", Some(&admin), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
}

#[tokio::test]
async fn catalog_adjust_stock_emits_movement() {
    let (db, t, admin) = setup().await;
    let p = catalog::create_product(&db, &t, new_product("A", "100"))
        .await
        .unwrap();
    let updated = catalog::adjust_stock(
        &db,
        &t,
        &p.id,
        StockAdjust {
            set: Some(42),
            delta: None,
            reason: Some("manual_count".into()),
        },
        Some(&admin),
    )
    .await
    .unwrap();
    assert_eq!(updated.stock, 42);
    let movements = inv::list_movements(
        &db,
        &t,
        MovementFilters {
            product: Some(p.id.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(movements.len(), 1);
    assert_eq!(movements[0].delta, 42);
    assert_eq!(movements[0].reason, "manual_count");
}

#[tokio::test]
async fn batch_creation_emits_initial_movement() {
    let (db, t, admin) = setup().await;
    let p = catalog::create_product(&db, &t, new_product("A", "100"))
        .await
        .unwrap();
    let exp = Utc::now() + Duration::days(180);
    let batch = inv::create_batch(
        &db,
        &t,
        NewBatch {
            product: p.id.clone(),
            batch_code: "L001".into(),
            expiry_date: exp,
            stock: 30,
            cost: Some(dec("50")),
            notes: None,
        },
        Some(&admin),
    )
    .await
    .unwrap();
    assert_eq!(batch.stock, 30);
    let prod = catalog::get_product(&db, &t, &p.id).await.unwrap();
    assert_eq!(prod.stock, 30, "batch_received raises product stock");
    let mvts = inv::list_movements(&db, &t, MovementFilters::default())
        .await
        .unwrap();
    assert_eq!(mvts.len(), 1);
    assert_eq!(mvts[0].reason, "batch_received");
    assert_eq!(mvts[0].delta, 30);
    assert_eq!(mvts[0].r#ref.as_deref(), Some("L001"));
}

#[tokio::test]
async fn fefo_orders_by_expiry_then_created() {
    let (db, t, admin) = setup().await;
    let p = catalog::create_product(&db, &t, new_product("A", "100"))
        .await
        .unwrap();
    let early = Utc::now() + Duration::days(10);
    let mid = Utc::now() + Duration::days(60);
    let late = Utc::now() + Duration::days(365);

    // Create out of order; FEFO must still pick the earliest expiry first.
    let _b_late = inv::create_batch(
        &db,
        &t,
        NewBatch {
            product: p.id.clone(),
            batch_code: "L-late".into(),
            expiry_date: late,
            stock: 5,
            cost: None,
            notes: None,
        },
        Some(&admin),
    )
    .await
    .unwrap();
    let b_early = inv::create_batch(
        &db,
        &t,
        NewBatch {
            product: p.id.clone(),
            batch_code: "L-early".into(),
            expiry_date: early,
            stock: 3,
            cost: None,
            notes: None,
        },
        Some(&admin),
    )
    .await
    .unwrap();
    let b_mid = inv::create_batch(
        &db,
        &t,
        NewBatch {
            product: p.id.clone(),
            batch_code: "L-mid".into(),
            expiry_date: mid,
            stock: 4,
            cost: None,
            notes: None,
        },
        Some(&admin),
    )
    .await
    .unwrap();

    let plan = inv::plan_fefo(&db, &t, &p.id, 6).await.unwrap();
    // Expect 3 from early then 3 from mid.
    assert_eq!(plan.len(), 2);
    assert_eq!(plan[0].batch, b_early.id);
    assert_eq!(plan[0].qty, 3);
    assert_eq!(plan[1].batch, b_mid.id);
    assert_eq!(plan[1].qty, 3);

    // Insufficient available across all batches → InsufficientStock.
    let err = inv::plan_fefo(&db, &t, &p.id, 100).await.unwrap_err();
    assert_eq!(err.code(), "INSUFFICIENT_STOCK");
    let _ = b_mid; // silence unused warnings
}

#[tokio::test]
async fn batch_soft_delete_keeps_row() {
    let (db, t, admin) = setup().await;
    let p = catalog::create_product(&db, &t, new_product("A", "100"))
        .await
        .unwrap();
    let exp = Utc::now() + Duration::days(30);
    let b = inv::create_batch(
        &db,
        &t,
        NewBatch {
            product: p.id.clone(),
            batch_code: "L1".into(),
            expiry_date: exp,
            stock: 10,
            cost: None,
            notes: None,
        },
        Some(&admin),
    )
    .await
    .unwrap();
    inv::delete_batch(&db, &t, &b.id).await.unwrap();
    let after = inv::get_batch(&db, &t, &b.id).await.unwrap();
    assert!(!after.active, "soft-delete keeps row, flips active");
}

#[tokio::test]
async fn faltas_crud_resolved_filter() {
    let (db, t, _admin) = setup().await;
    let p = catalog::create_product(&db, &t, new_product("A", "100"))
        .await
        .unwrap();
    let f = inv::create_falta(
        &db,
        &t,
        NewFalta {
            product: Some(p.id.clone()),
            name: "Paracetamol Forte".into(),
            qty: 2,
            customer_name: None,
            customer_phone: None,
            notes: None,
        },
    )
    .await
    .unwrap();
    assert!(!f.resolved);
    let pending = inv::list_faltas(
        &db,
        &t,
        FaltaFilters {
            resolved: Some(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(pending.len(), 1);

    inv::update_falta(
        &db,
        &t,
        &f.id,
        UpdateFalta {
            resolved: Some(true),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let still_pending = inv::list_faltas(
        &db,
        &t,
        FaltaFilters {
            resolved: Some(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(still_pending.is_empty());
}

#[tokio::test]
async fn tenant_isolation_for_movements_and_batches() {
    let (db, t1, admin1) = setup().await;
    let mut r = db
        .query("CREATE tenant SET name = 'Otra', slug = 'otra' RETURN id")
        .await
        .unwrap();
    let t2: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();
    let p = catalog::create_product(&db, &t1, new_product("A", "100"))
        .await
        .unwrap();
    inv::add_movement(&db, &t1, &p.id, 7, "manual_adjust", Some(&admin1), None)
        .await
        .unwrap();
    let seen = inv::list_movements(&db, &t2, MovementFilters::default())
        .await
        .unwrap();
    assert!(seen.is_empty(), "tenant 2 must not see tenant 1 movements");
    // tenant 2 also cannot mutate tenant 1's product.
    let err = inv::add_movement(&db, &t2, &p.id, 1, "manual_adjust", Some(&admin1), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "NOT_FOUND");
}

#[tokio::test]
async fn summary_aggregates_products_batches_faltas() {
    let (db, t, admin) = setup().await;
    let p = catalog::create_product(&db, &t, new_product("A", "100"))
        .await
        .unwrap();
    inv::add_movement(&db, &t, &p.id, 3, "manual_adjust", Some(&admin), None)
        .await
        .unwrap();
    let exp = Utc::now() + Duration::days(7);
    inv::create_batch(
        &db,
        &t,
        NewBatch {
            product: p.id.clone(),
            batch_code: "L1".into(),
            expiry_date: exp,
            stock: 1,
            cost: Some(dec("80")),
            notes: None,
        },
        Some(&admin),
    )
    .await
    .unwrap();
    inv::create_falta(
        &db,
        &t,
        NewFalta {
            product: None,
            name: "Algo".into(),
            qty: 1,
            customer_name: None,
            customer_phone: None,
            notes: None,
        },
    )
    .await
    .unwrap();
    let s = inv::summary(&db, &t).await.unwrap();
    assert_eq!(s.total_skus, 1);
    assert_eq!(s.active_skus, 1);
    assert_eq!(s.expiring_soon, 1);
    assert_eq!(s.open_faltas, 1);
}

/// The inventory-module-landing summary serves its product-side aggregate from
/// the maintained `product_stats` view (migration 0029) instead of a fresh
/// `GROUP ALL` full scan over `product` — the BUG-perf-001 fix, previously only
/// wired into `catalog::service::stats` (the dashboard). This pins that the
/// numbers stay correct across the writes the view maintains incrementally,
/// including the `active` flip that a `DEFINE TABLE … AS SELECT` view would have
/// mis-handled (the documented SurrealDB view-UPDATE gotcha).
#[tokio::test]
async fn summary_product_side_tracks_maintained_view() {
    let (db, t, admin) = setup().await;
    // A: out of stock (0); B: low (3 <= 5); C: healthy (100). cost = 100 each.
    // A stays at stock 0 (out_of_stock); its id is not needed past creation.
    catalog::create_product(&db, &t, new_product("A", "100"))
        .await
        .unwrap();
    let b = catalog::create_product(&db, &t, new_product("B", "100"))
        .await
        .unwrap();
    let c = catalog::create_product(&db, &t, new_product("C", "100"))
        .await
        .unwrap();
    inv::add_movement(&db, &t, &b.id, 3, "manual_adjust", Some(&admin), None)
        .await
        .unwrap();
    inv::add_movement(&db, &t, &c.id, 100, "manual_adjust", Some(&admin), None)
        .await
        .unwrap();

    let s = inv::summary(&db, &t).await.unwrap();
    assert_eq!(s.total_skus, 3);
    assert_eq!(s.active_skus, 3);
    assert_eq!(s.low_stock, 2, "A(0) + B(3) are <= LOW_STOCK_DEFAULT");
    assert_eq!(s.out_of_stock, 1, "only A is at 0");
    // value = (0 + 3 + 100) * 100 = 10300 (counts active + inactive).
    assert_eq!(s.inventory_value, dec("10300"));

    // Deactivating C must drop it from the active count via the EVENT delta —
    // the exact UPDATE a pre-computed view mis-maintains. Value keeps active +
    // inactive, so it is unchanged.
    catalog::update_product(
        &db,
        &t,
        &c.id.to_string(),
        UpdateProduct {
            active: Some(false),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let s = inv::summary(&db, &t).await.unwrap();
    assert_eq!(s.total_skus, 3);
    assert_eq!(s.active_skus, 2, "C dropped from active");
    assert_eq!(s.low_stock, 2, "A + B still active and low");
    assert_eq!(s.out_of_stock, 1);
    assert_eq!(s.inventory_value, dec("10300"), "value counts inactive too");
}

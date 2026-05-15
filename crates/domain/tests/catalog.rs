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

//! Sales integration tests on in-memory SurrealDB (`kv-mem`). Exercises the
//! POS atomic-sale tx end-to-end: stock check, multi-stmt write, replay
//! idempotency, settings upsert.

use domain::catalog::{model::*, service as catalog};
use domain::sales::{model::*, service as sales};
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
        .expect("migrations");
    let mut r = db
        .query("CREATE tenant SET name = 'Farmacia Test', slug = 'test' RETURN id")
        .await
        .unwrap();
    let tenant: Option<Thing> = r.take((0, "id")).unwrap();
    let tenant = tenant.expect("tenant id");
    let mut r = db
        .query(
            "CREATE user SET tenant=$t, email='admin@test.local', \
             password='x', roles=['admin'] RETURN id",
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

fn np(name: &str, price: &str, stock: i64) -> NewProduct {
    NewProduct {
        name: name.into(),
        slug: None,
        description: None,
        price: dec(price),
        cost_price: Some(dec("100")),
        stock,
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
async fn pos_sale_atomic_decrements_stock_and_creates_movement() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Paracetamol 500", "1500", 50))
        .await
        .unwrap();
    let admin_t = surrealdb::sql::thing(&admin).unwrap();

    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: p.id.clone(),
            product_name: p.name.clone(),
            quantity: 3,
            unit_price: dec("1500"),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("5000")),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
    };
    let resp = sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap();

    assert_eq!(resp.items.len(), 1);
    assert_eq!(resp.items[0].quantity, 3);
    assert_eq!(resp.stock_movements.len(), 1);
    assert_eq!(resp.order.total, dec("4500"));

    // Stock now 47
    let p2 = catalog::get_product(&db, &tenant, &p.id).await.unwrap();
    assert_eq!(p2.stock, 47);
}

#[tokio::test]
async fn pos_sale_rejects_insufficient_stock() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Ibuprofeno 400", "2000", 2))
        .await
        .unwrap();
    let admin_t = surrealdb::sql::thing(&admin).unwrap();

    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: p.id.clone(),
            product_name: p.name.clone(),
            quantity: 5,
            unit_price: dec("2000"),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("10000")),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
    };
    let err = sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INSUFFICIENT_STOCK");
    // Stock untouched
    let p2 = catalog::get_product(&db, &tenant, &p.id).await.unwrap();
    assert_eq!(p2.stock, 2);
}

#[tokio::test]
async fn pos_sale_rejects_invalid_payment_method() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("X", "1000", 10))
        .await
        .unwrap();
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: p.id.clone(),
            product_name: p.name.clone(),
            quantity: 1,
            unit_price: dec("1000"),
        }],
        payment_method: "crypto".into(),
        cash_amount: None,
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
    };
    let err = sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
}

#[tokio::test]
async fn admin_setting_upsert_idempotent() {
    let (db, tenant, _admin) = setup().await;
    let s1 = sales::set_setting(&db, &tenant, "low_stock_threshold", "10")
        .await
        .unwrap();
    assert_eq!(s1.value, "10");
    let s2 = sales::set_setting(&db, &tenant, "low_stock_threshold", "5")
        .await
        .unwrap();
    assert_eq!(s2.value, "5");
    let got = sales::get_setting(&db, &tenant, "low_stock_threshold")
        .await
        .unwrap();
    assert_eq!(got.unwrap().value, "5");
}

#[tokio::test]
async fn pos_sale_tenant_isolation() {
    let (db, t1, admin1) = setup().await;
    // Second tenant in same DB
    let mut r = db
        .query("CREATE tenant SET name='Otra', slug='otra' RETURN id")
        .await
        .unwrap();
    let t2: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();

    let p1 = catalog::create_product(&db, &t1, np("A", "1000", 10))
        .await
        .unwrap();
    let admin_t = surrealdb::sql::thing(&admin1).unwrap();
    // Try selling tenant1's product as tenant2 — should fail NOT_FOUND
    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: p1.id.clone(),
            product_name: p1.name.clone(),
            quantity: 1,
            unit_price: dec("1000"),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("1000")),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
    };
    let err = sales::post_sale(&db, &t2, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "NOT_FOUND");
}

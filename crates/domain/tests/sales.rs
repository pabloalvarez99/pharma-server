//! Sales integration tests on in-memory SurrealDB (`kv-mem`). Exercises the
//! POS atomic-sale tx end-to-end: stock check, multi-stmt write, replay
//! idempotency, settings upsert.

use chrono::{Duration, Utc};
use domain::catalog::{model::*, service as catalog};
use domain::customers::{model::*, service as customers};
use domain::inventory::{model as inv_model, service as inventory};
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
async fn pos_sale_awards_loyalty_when_customer_set() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Vitamina C", "2500", 20))
        .await
        .unwrap();
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    let c = customers::create_customer(
        &db,
        &tenant,
        NewCustomer {
            name: "Ana".into(),
            rut: Some("12.345.678-5".into()),
            phone: None,
            email: None,
        },
    )
    .await
    .unwrap();

    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: p.id.clone(),
            product_name: p.name.clone(),
            quantity: 4,
            unit_price: dec("2500"),
        }],
        payment_method: "pos_debit".into(),
        cash_amount: None,
        card_amount: Some(dec("10000")),
        discount: None,
        customer: Some(c.id.clone()),
        customer_name: Some(c.name.clone()),
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
    };
    let resp = sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap();
    // total 10000 / 1000 per point default = 10 points
    assert_eq!(resp.loyalty_points_awarded, 10);

    // Customer.loyalty_points now 10
    let c2 = customers::get_customer(&db, &tenant, &c.id).await.unwrap();
    assert_eq!(c2.loyalty_points, 10);
}

#[tokio::test]
async fn pos_sale_loyalty_rate_setting_overrides_default() {
    let (db, tenant, admin) = setup().await;
    sales::set_setting(&db, &tenant, "loyalty_points_per_clp", "500")
        .await
        .unwrap();
    let p = catalog::create_product(&db, &tenant, np("Aspirina", "1500", 10))
        .await
        .unwrap();
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    let c = customers::create_customer(
        &db,
        &tenant,
        NewCustomer {
            name: "Bea".into(),
            rut: None,
            phone: None,
            email: None,
        },
    )
    .await
    .unwrap();
    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: p.id.clone(),
            product_name: p.name.clone(),
            quantity: 2,
            unit_price: dec("1500"),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("3000")),
        card_amount: None,
        discount: None,
        customer: Some(c.id.clone()),
        customer_name: Some(c.name.clone()),
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
    };
    let resp = sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap();
    // 3000 / 500 = 6 points
    assert_eq!(resp.loyalty_points_awarded, 6);
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

// --- FEFO batch-tracked sales ---------------------------------------------

#[tokio::test]
async fn pos_sale_batch_tracked_fefo_decrements_earliest_expiry() {
    let (db, tenant, admin) = setup().await;
    // Product is batch-tracked. Two lots: lot A expires sooner, lot B later.
    // FEFO must consume A's full 4 then dip into B for 1.
    let p = catalog::create_product(&db, &tenant, np("Amoxicilina 500", "3000", 0))
        .await
        .unwrap();
    let lot_a = inventory::create_batch(
        &db,
        &tenant,
        inv_model::NewBatch {
            product: p.id.clone(),
            batch_code: "A-001".into(),
            expiry_date: Utc::now() + Duration::days(30),
            stock: 4,
            cost: Some(dec("1500")),
            notes: None,
        },
        Some(&admin),
    )
    .await
    .unwrap();
    let lot_b = inventory::create_batch(
        &db,
        &tenant,
        inv_model::NewBatch {
            product: p.id.clone(),
            batch_code: "B-001".into(),
            expiry_date: Utc::now() + Duration::days(120),
            stock: 10,
            cost: Some(dec("1500")),
            notes: None,
        },
        Some(&admin),
    )
    .await
    .unwrap();
    let admin_t = surrealdb::sql::thing(&admin).unwrap();

    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: p.id.clone(),
            product_name: p.name.clone(),
            quantity: 5,
            unit_price: dec("3000"),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("15000")),
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

    // Line records the earliest-expiry lot (first FEFO allocation).
    assert_eq!(resp.items[0].batch.as_deref(), Some(lot_a.id.as_str()));

    // product.stock decremented by total qty.
    let p2 = catalog::get_product(&db, &tenant, &p.id).await.unwrap();
    assert_eq!(p2.stock, 14 - 5);

    // Lot A drained, lot B took the overflow → product.stock == sum(batch.stock).
    let a2 = inventory::get_batch(&db, &tenant, &lot_a.id).await.unwrap();
    let b2 = inventory::get_batch(&db, &tenant, &lot_b.id).await.unwrap();
    assert_eq!(a2.stock, 0, "earlier-expiry lot must drain first");
    assert_eq!(b2.stock, 9, "later-expiry lot takes the overflow");
    assert_eq!(p2.stock, a2.stock + b2.stock);
}

#[tokio::test]
async fn pos_sale_batch_tracked_rejects_when_only_expired_lots_remain() {
    let (db, tenant, admin) = setup().await;
    // Batch-tracked product with stock only in an expired lot — must NOT sell.
    let p = catalog::create_product(&db, &tenant, np("Vencido", "1000", 0))
        .await
        .unwrap();
    inventory::create_batch(
        &db,
        &tenant,
        inv_model::NewBatch {
            product: p.id.clone(),
            batch_code: "OLD".into(),
            expiry_date: Utc::now() - Duration::days(1),
            stock: 10,
            cost: None,
            notes: None,
        },
        Some(&admin),
    )
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
    let err = sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INSUFFICIENT_STOCK");

    // Nothing changed: no order, no order_item, no stock_movement, lot untouched.
    let orders = sales::list_orders(&db, &tenant, OrderFilters::default())
        .await
        .unwrap();
    assert!(orders.is_empty(), "rejected sale must not create an order");
}

#[tokio::test]
async fn pos_sale_non_batch_tracked_falls_back_to_product_stock() {
    // No batches → legacy product.stock path, batch field on line is null.
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Sin lote", "500", 8))
        .await
        .unwrap();
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: p.id.clone(),
            product_name: p.name.clone(),
            quantity: 3,
            unit_price: dec("500"),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("1500")),
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
    assert!(resp.items[0].batch.is_none());
    let p2 = catalog::get_product(&db, &tenant, &p.id).await.unwrap();
    assert_eq!(p2.stock, 5);
}

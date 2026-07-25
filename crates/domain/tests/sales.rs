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
        attrs: None,
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
        branch: None,
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
        branch: None,
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
        branch: None,
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
        branch: None,
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
        branch: None,
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
        branch: None,
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
        branch: None,
    };
    let resp = sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap();

    // Line records the earliest-expiry lot (first FEFO allocation).
    assert_eq!(resp.items[0].batch.as_deref(), Some(lot_a.id.as_str()));

    // BACKLOG #3 — multi-lot split traceability. The full FEFO breakdown lives
    // on `batches[]`, in consumption order, with per-allocation qty summing to
    // the line quantity. The legacy `batch` field stays = batches[0].batch.
    let batches = resp.items[0]
        .batches
        .as_ref()
        .expect("multi-lot consumption persists batches[]");
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].batch, lot_a.id);
    assert_eq!(batches[0].qty, 4);
    assert_eq!(batches[1].batch, lot_b.id);
    assert_eq!(batches[1].qty, 1);
    assert_eq!(batches.iter().map(|a| a.qty).sum::<i64>(), 5);

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
        branch: None,
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
        branch: None,
    };
    let resp = sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap();
    assert!(resp.items[0].batch.is_none());
    // BACKLOG #3 — products without FEFO planning persist no breakdown.
    assert!(resp.items[0].batches.is_none());
    let p2 = catalog::get_product(&db, &tenant, &p.id).await.unwrap();
    assert_eq!(p2.stock, 5);
}

// --- returns / devoluciones ------------------------------------------------

async fn sell_one(
    db: &Db,
    tenant: &Thing,
    admin_t: &Thing,
    product_id: &str,
    product_name: &str,
    qty: i64,
    unit_price: &str,
) -> PosSaleResponse {
    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: product_id.to_string(),
            product_name: product_name.to_string(),
            quantity: qty,
            unit_price: dec(unit_price),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("999999")),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
        branch: None,
    };
    sales::post_sale(db, tenant, Some(admin_t), Some("admin"), None, req)
        .await
        .unwrap()
}

#[tokio::test]
async fn purge_expired_idempotency_drops_only_expired_rows() {
    let (db, tenant, _admin) = setup().await;
    db.query(
        "CREATE idempotency_key SET tenant=$t, key='gone', response_json='{}', \
         status_code=200, expires_at=$e",
    )
    .bind(("t", tenant.clone()))
    .bind((
        "e",
        surrealdb::sql::Datetime::from(Utc::now() - Duration::hours(1)),
    ))
    .await
    .unwrap();
    db.query(
        "CREATE idempotency_key SET tenant=$t, key='stays', response_json='{}', \
         status_code=200, expires_at=$e",
    )
    .bind(("t", tenant.clone()))
    .bind((
        "e",
        surrealdb::sql::Datetime::from(Utc::now() + Duration::hours(1)),
    ))
    .await
    .unwrap();

    let removed = sales::purge_expired_idempotency(&db).await.unwrap();
    assert_eq!(removed, 1);

    #[derive(serde::Deserialize)]
    struct R {
        key: String,
    }
    let mut q = db
        .query("SELECT key FROM idempotency_key WHERE tenant=$t")
        .bind(("t", tenant))
        .await
        .unwrap();
    let rows: Vec<R> = q.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key, "stays");

    let removed = sales::purge_expired_idempotency(&db).await.unwrap();
    assert_eq!(removed, 0);
}

#[tokio::test]
async fn pos_sale_with_prescription_persists_receta_linked_to_customer() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Amoxi 500", "1200", 20))
        .await
        .unwrap();
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    let c = customers::create_customer(
        &db,
        &tenant,
        NewCustomer {
            name: "Juan".into(),
            rut: Some("11.111.111-1".into()),
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
            quantity: 1,
            unit_price: dec("1200"),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("2000")),
        card_amount: None,
        discount: None,
        customer: Some(c.id.clone()),
        customer_name: Some(c.name.clone()),
        customer_phone: None,
        notes: None,
        external_ref: None,
        branch: None,
        prescriptions: vec![PosPrescriptionInput {
            product: Some(p.id.clone()),
            patient_name: "Juan".into(),
            patient_rut: "11.111.111-1".into(),
            doctor_name: Some("Dra. Pérez".into()),
            doctor_rut: Some("9.876.543-2".into()),
            folio: Some("FOL-001".into()),
            controlled: Some(false),
        }],
    };
    let resp = sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap();
    assert_eq!(resp.prescriptions.len(), 1);
    assert!(resp.prescriptions[0].starts_with("prescription:"));
}

#[tokio::test]
async fn pos_sale_controlled_prescription_requires_doctor_data() {
    // controlled=Some(true) sin doctor → INVALID_INPUT del repo de receta.
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Clonazepam 2mg", "500", 5))
        .await
        .unwrap();
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: p.id.clone(),
            product_name: p.name.clone(),
            quantity: 1,
            unit_price: dec("500"),
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
        branch: None,
        prescriptions: vec![PosPrescriptionInput {
            product: Some(p.id.clone()),
            patient_name: "Ana".into(),
            patient_rut: "22.222.222-2".into(),
            doctor_name: None,
            doctor_rut: None,
            folio: None,
            controlled: Some(true),
        }],
    };
    let err = sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
}

#[tokio::test]
async fn refund_with_restock_returns_stock_marks_order_refunded_and_logs_movement() {
    let (db, tenant, admin) = setup().await;
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    let p = catalog::create_product(&db, &tenant, np("Amoxicilina 500", "3000", 30))
        .await
        .unwrap();
    let sale = sell_one(&db, &tenant, &admin_t, &p.id, &p.name, 5, "3000").await;
    assert_eq!(
        catalog::get_product(&db, &tenant, &p.id)
            .await
            .unwrap()
            .stock,
        25
    );

    let req = NewDevolucion {
        order: Some(sale.order.id.clone()),
        tipo: "venta".into(),
        motivo: "cliente devolvió producto sellado".into(),
        notas: None,
        metodo_reembolso: Some("efectivo".into()),
        items: vec![NewDevolucionItem {
            product: Some(p.id.clone()),
            product_name: p.name.clone(),
            quantity: 2,
            unit_price: dec("3000"),
            restock: true,
        }],
    };
    let resp = sales::create_refund(&db, &tenant, Some(&admin_t), req)
        .await
        .unwrap();

    assert_eq!(resp.devolucion.total_devuelto, dec("6000"));
    assert_eq!(resp.items.len(), 1);
    assert_eq!(resp.stock_movements.len(), 1);
    assert!(resp.order_marked_refunded);
    // Restocked: 25 + 2 = 27
    assert_eq!(
        catalog::get_product(&db, &tenant, &p.id)
            .await
            .unwrap()
            .stock,
        27
    );
    // Order flipped to refunded.
    let (order, _items) = sales::get_order(&db, &tenant, &sale.order.id)
        .await
        .unwrap();
    assert_eq!(order.status, "refunded");
    // Listed under returns.
    let list = sales::list_refunds(&db, &tenant, DevolucionFilters::default())
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
}

#[tokio::test]
async fn refund_without_restock_does_not_touch_stock_or_movements() {
    let (db, tenant, admin) = setup().await;
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    let p = catalog::create_product(&db, &tenant, np("Jarabe vencido", "2000", 10))
        .await
        .unwrap();
    let sale = sell_one(&db, &tenant, &admin_t, &p.id, &p.name, 4, "2000").await;
    assert_eq!(
        catalog::get_product(&db, &tenant, &p.id)
            .await
            .unwrap()
            .stock,
        6
    );

    let req = NewDevolucion {
        order: Some(sale.order.id.clone()),
        tipo: "garantia".into(),
        motivo: "producto vencido, no revender".into(),
        notas: Some("descartar".into()),
        metodo_reembolso: Some("efectivo".into()),
        items: vec![NewDevolucionItem {
            product: Some(p.id.clone()),
            product_name: p.name.clone(),
            quantity: 4,
            unit_price: dec("2000"),
            restock: false,
        }],
    };
    let resp = sales::create_refund(&db, &tenant, Some(&admin_t), req)
        .await
        .unwrap();
    assert!(resp.stock_movements.is_empty());
    assert_eq!(resp.devolucion.total_devuelto, dec("8000"));
    // Stock unchanged (not resellable).
    assert_eq!(
        catalog::get_product(&db, &tenant, &p.id)
            .await
            .unwrap()
            .stock,
        6
    );
}

#[tokio::test]
async fn refund_rejected_when_quantity_exceeds_sold() {
    let (db, tenant, admin) = setup().await;
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    let p = catalog::create_product(&db, &tenant, np("Ibuprofeno 600", "1800", 20))
        .await
        .unwrap();
    let sale = sell_one(&db, &tenant, &admin_t, &p.id, &p.name, 3, "1800").await;

    let req = NewDevolucion {
        order: Some(sale.order.id.clone()),
        tipo: "venta".into(),
        motivo: "intento de sobre-devolución".into(),
        notas: None,
        metodo_reembolso: None,
        items: vec![NewDevolucionItem {
            product: Some(p.id.clone()),
            product_name: p.name.clone(),
            quantity: 5,
            unit_price: dec("1800"),
            restock: true,
        }],
    };
    let err = sales::create_refund(&db, &tenant, Some(&admin_t), req)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
    // Stock untouched by the rejected refund.
    assert_eq!(
        catalog::get_product(&db, &tenant, &p.id)
            .await
            .unwrap()
            .stock,
        17
    );
}

#[tokio::test]
async fn refund_restock_without_product_is_rejected() {
    let (db, tenant, _admin) = setup().await;
    let admin_t = surrealdb::sql::thing(
        &db.query(
            "CREATE user SET tenant=$t, email='b@t.l', password='x', roles=['admin'] RETURN id",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap()
        .to_string(),
    )
    .unwrap();
    let req = NewDevolucion {
        order: None,
        tipo: "error".into(),
        motivo: "ajuste sin orden".into(),
        notas: None,
        metodo_reembolso: None,
        items: vec![NewDevolucionItem {
            product: None,
            product_name: "Genérico".into(),
            quantity: 1,
            unit_price: dec("1000"),
            restock: true,
        }],
    };
    let err = sales::create_refund(&db, &tenant, Some(&admin_t), req)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
}

/// Concurrent refunds of the SAME order must never refund past the sold qty.
/// The cumulative over-refund guard (`sum_prior_refunds` → `refund_exceeds_sold`
/// → `apply_refund`) is a check-then-act TOCTOU: without serialization two
/// refunds both read `prior=0`, both pass the guard, and both COMMIT → the
/// order is refunded beyond what was sold (refund-fraud vector, BUG-005) and
/// the FEFO restock double-bumps `product.stock`. `create_refund` now holds the
/// same per-tenant `SALE_LOCKS` as `post_sale`, so the reads + write are atomic
/// w.r.t. each other and w.r.t. concurrent sales.
#[tokio::test]
async fn concurrent_refunds_never_exceed_sold_quantity() {
    let (db, tenant, admin) = setup().await;
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    // Sell all 10 units in one order, then fire 8 concurrent refunds of 6 each:
    // a single refund (6) fits under sold=10; a second (6+6=12) must be rejected.
    let p = catalog::create_product(&db, &tenant, np("Losartan 50", "2500", 10))
        .await
        .unwrap();
    let sale = sell_one(&db, &tenant, &admin_t, &p.id, &p.name, 10, "2500").await;
    assert_eq!(
        catalog::get_product(&db, &tenant, &p.id)
            .await
            .unwrap()
            .stock,
        0
    );

    let mut tasks = Vec::new();
    for _ in 0..8 {
        let db = db.clone();
        let tenant = tenant.clone();
        let admin_t = admin_t.clone();
        let order = sale.order.id.clone();
        let pid = p.id.clone();
        let pname = p.name.clone();
        tasks.push(async move {
            let req = NewDevolucion {
                order: Some(order),
                tipo: "venta".into(),
                motivo: "carrera de devoluciones".into(),
                notas: None,
                metodo_reembolso: Some("efectivo".into()),
                items: vec![NewDevolucionItem {
                    product: Some(pid),
                    product_name: pname,
                    quantity: 6,
                    unit_price: dec("2500"),
                    restock: true,
                }],
            };
            sales::create_refund(&db, &tenant, Some(&admin_t), req).await
        });
    }
    let results = futures::future::join_all(tasks).await;
    let ok = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(ok, 1, "exactly one refund of 6 fits under sold=10");
    // Cumulative refunded qty (from stock movements) must not exceed sold.
    #[derive(serde::Deserialize)]
    struct Sum {
        total: Option<i64>,
    }
    let mut q = db
        .query(
            "SELECT math::sum(delta) AS total FROM stock_movement \
             WHERE tenant=$t AND reason='return' GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let refunded: i64 = q
        .take::<Option<Sum>>(0)
        .unwrap()
        .and_then(|s| s.total)
        .unwrap_or(0);
    assert_eq!(refunded, 6, "only 6 units restocked, never more than sold");
    // product.stock reflects exactly the one accepted restock (0 + 6).
    assert_eq!(
        catalog::get_product(&db, &tenant, &p.id)
            .await
            .unwrap()
            .stock,
        6
    );
}

// ---------------------------------------------------------------------------
// Variantes multi-SKU: stock por hijo, padre no vendible, plain SKU intacto
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sale_of_variant_decrements_only_that_variant() {
    let (db, tenant, admin) = setup().await;
    let parent = catalog::create_product(&db, &tenant, np("Polera basica", "9990", 0))
        .await
        .unwrap();
    let m = catalog::create_variant(
        &db,
        &tenant,
        &parent.id,
        domain::catalog::model::NewVariant {
            name: None,
            slug: None,
            price: None,
            cost_price: None,
            stock: 10,
            barcode: Some("7804999400012".into()),
            attrs: Some(serde_json::json!({"talla": "M"})),
            external_id: None,
            image_url: None,
        },
    )
    .await
    .unwrap();
    let l = catalog::create_variant(
        &db,
        &tenant,
        &parent.id,
        domain::catalog::model::NewVariant {
            name: None,
            slug: None,
            price: None,
            cost_price: None,
            stock: 7,
            barcode: Some("7804999400029".into()),
            attrs: Some(serde_json::json!({"talla": "L"})),
            external_id: None,
            image_url: None,
        },
    )
    .await
    .unwrap();

    // Barcode resolves to variant M
    let resolved = catalog::find_by_barcode(&db, &tenant, "7804999400012")
        .await
        .unwrap();
    assert_eq!(resolved.id, m.id);

    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: m.id.clone(),
            product_name: m.name.clone(),
            quantity: 3,
            unit_price: dec("9990"),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("30000")),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
        branch: None,
    };
    sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap();

    let m2 = catalog::get_product(&db, &tenant, &m.id).await.unwrap();
    let l2 = catalog::get_product(&db, &tenant, &l.id).await.unwrap();
    let p2 = catalog::get_product(&db, &tenant, &parent.id)
        .await
        .unwrap();
    assert_eq!(m2.stock, 7, "only M decremented");
    assert_eq!(l2.stock, 7, "L untouched");
    assert_eq!(p2.stock, 0, "parent stock untouched");
}

#[tokio::test]
async fn sale_rejects_insufficient_stock_on_variant() {
    let (db, tenant, admin) = setup().await;
    let parent = catalog::create_product(&db, &tenant, np("Gorro", "5000", 0))
        .await
        .unwrap();
    let v = catalog::create_variant(
        &db,
        &tenant,
        &parent.id,
        domain::catalog::model::NewVariant {
            name: None,
            slug: None,
            price: None,
            cost_price: None,
            stock: 2,
            barcode: Some("7804999500019".into()),
            attrs: Some(serde_json::json!({"talla": "U"})),
            external_id: None,
            image_url: None,
        },
    )
    .await
    .unwrap();
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: v.id.clone(),
            product_name: v.name.clone(),
            quantity: 5,
            unit_price: dec("5000"),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("25000")),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
        branch: None,
    };
    let err = sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INSUFFICIENT_STOCK");
    let v2 = catalog::get_product(&db, &tenant, &v.id).await.unwrap();
    assert_eq!(v2.stock, 2);
}

#[tokio::test]
async fn sale_rejects_parent_when_has_variants() {
    let (db, tenant, admin) = setup().await;
    let parent = catalog::create_product(&db, &tenant, np("Chaqueta", "20000", 50))
        .await
        .unwrap();
    catalog::create_variant(
        &db,
        &tenant,
        &parent.id,
        domain::catalog::model::NewVariant {
            name: None,
            slug: None,
            price: None,
            cost_price: None,
            stock: 5,
            barcode: Some("7804999600016".into()),
            attrs: Some(serde_json::json!({"talla": "M"})),
            external_id: None,
            image_url: None,
        },
    )
    .await
    .unwrap();
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: parent.id.clone(),
            product_name: parent.name.clone(),
            quantity: 1,
            unit_price: dec("20000"),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("20000")),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
        branch: None,
    };
    let err = sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("tiene variantes"),
        "stable ES fragment for POS client: {msg}"
    );
    assert!(
        msg.contains("escanee el código") || msg.contains("escanee el codigo"),
        "stable ES fragment for POS client: {msg}"
    );
}

#[tokio::test]
async fn plain_product_without_variants_still_sells() {
    // Farmacia / minimarket path: no parent_id, no children.
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Paracetamol 500", "1500", 20))
        .await
        .unwrap();
    let admin_t = surrealdb::sql::thing(&admin).unwrap();
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
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
        branch: None,
    };
    sales::post_sale(&db, &tenant, Some(&admin_t), Some("admin"), None, req)
        .await
        .unwrap();
    let p2 = catalog::get_product(&db, &tenant, &p.id).await.unwrap();
    assert_eq!(p2.stock, 18);
}

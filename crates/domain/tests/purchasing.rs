//! Purchasing integration tests on an in-memory SurrealDB (`kv-mem`).
//! Each test gets an isolated db + the real migrations applied.

use domain::catalog::{model::NewProduct, service as catalog};
use domain::purchasing::{model::*, service};
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

fn new_supplier(name: &str) -> NewSupplier {
    NewSupplier {
        name: name.into(),
        rut: None,
        contact_name: None,
        contact_email: None,
        contact_phone: None,
        default_invoice_format: None,
    }
}

fn new_product(name: &str, price: &str, cost: Option<&str>) -> NewProduct {
    NewProduct {
        name: name.into(),
        slug: None,
        description: None,
        price: dec(price),
        cost_price: cost.map(dec),
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
async fn supplier_crud_and_soft_delete() {
    let (db, t) = setup().await;
    let s = service::create_supplier(&db, &t, new_supplier("ACME Pharma"))
        .await
        .unwrap();
    assert_eq!(s.name, "ACME Pharma");
    assert!(s.active);

    let updated = service::update_supplier(
        &db,
        &t,
        &s.id,
        UpdateSupplier {
            contact_email: Some("ventas@acme.cl".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.contact_email.as_deref(), Some("ventas@acme.cl"));

    service::delete_supplier(&db, &t, &s.id).await.unwrap();
    let after = service::get_supplier(&db, &t, &s.id).await.unwrap();
    assert!(!after.active, "soft delete keeps row, flips active");
}

#[tokio::test]
async fn price_list_decimal_round_trips() {
    let (db, t) = setup().await;
    let s = service::create_supplier(&db, &t, new_supplier("S1"))
        .await
        .unwrap();
    let created = service::create_price(
        &db,
        &t,
        NewSupplierPrice {
            supplier: s.id.clone(),
            product: None,
            supplier_code: Some("SKU-001".into()),
            description: Some("Caja 100u".into()),
            unit_cost: dec("12345.67"),
            currency: None,
            valid_from: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(created.unit_cost, dec("12345.67"));
    assert_eq!(created.currency, "CLP");

    let v = serde_json::to_value(&created).unwrap();
    assert_eq!(v["unit_cost"], "12345.67");

    let listed = service::list_prices(
        &db,
        &t,
        SupplierPriceFilters {
            supplier: Some(s.id),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].unit_cost, dec("12345.67"));
}

#[tokio::test]
async fn compare_picks_lowest_unit_cost_and_computes_savings() {
    let (db, t) = setup().await;
    let prod = catalog::create_product(&db, &t, new_product("Paracetamol", "1990", Some("900")))
        .await
        .unwrap();
    let s1 = service::create_supplier(&db, &t, new_supplier("Cheap"))
        .await
        .unwrap();
    let s2 = service::create_supplier(&db, &t, new_supplier("Expensive"))
        .await
        .unwrap();
    for (sid, cost) in [(&s1.id, "700"), (&s2.id, "850")] {
        service::create_price(
            &db,
            &t,
            NewSupplierPrice {
                supplier: sid.clone(),
                product: Some(prod.id.clone()),
                supplier_code: None,
                description: None,
                unit_cost: dec(cost),
                currency: None,
                valid_from: None,
            },
        )
        .await
        .unwrap();
    }

    let res = service::compare(
        &db,
        &t,
        CompareRequest {
            items: vec![CompareItem {
                product: Some(prod.id.clone()),
                supplier_code: None,
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(res.items.len(), 1);
    let item = &res.items[0];
    let best = item.best.as_ref().expect("best supplier present");
    assert_eq!(best.supplier, s1.id);
    assert_eq!(best.supplier_name, "Cheap");
    assert_eq!(best.unit_cost, dec("700"));
    assert_eq!(item.current_cost, Some(dec("900")));
    assert_eq!(item.savings, Some(dec("200")));
}

#[tokio::test]
async fn compare_by_supplier_code_when_product_absent() {
    let (db, t) = setup().await;
    let s = service::create_supplier(&db, &t, new_supplier("S"))
        .await
        .unwrap();
    service::create_price(
        &db,
        &t,
        NewSupplierPrice {
            supplier: s.id.clone(),
            product: None,
            supplier_code: Some("CODE-X".into()),
            description: None,
            unit_cost: dec("500"),
            currency: None,
            valid_from: None,
        },
    )
    .await
    .unwrap();

    let res = service::compare(
        &db,
        &t,
        CompareRequest {
            items: vec![CompareItem {
                product: None,
                supplier_code: Some("CODE-X".into()),
            }],
        },
    )
    .await
    .unwrap();
    let item = &res.items[0];
    assert_eq!(item.best.as_ref().unwrap().unit_cost, dec("500"));
    assert!(item.current_cost.is_none());
    assert!(item.savings.is_none());
}

#[tokio::test]
async fn mapping_unique_per_supplier_code() {
    let (db, t) = setup().await;
    let s = service::create_supplier(&db, &t, new_supplier("S"))
        .await
        .unwrap();
    let p = catalog::create_product(&db, &t, new_product("X", "100", None))
        .await
        .unwrap();
    service::map_product(
        &db,
        &t,
        &s.id,
        MapSupplierProduct {
            product: p.id.clone(),
            supplier_code: "ABC".into(),
        },
    )
    .await
    .unwrap();
    // Same (supplier, supplier_code) duplicates → CONFLICT.
    let err = service::map_product(
        &db,
        &t,
        &s.id,
        MapSupplierProduct {
            product: p.id.clone(),
            supplier_code: "ABC".into(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
}

#[tokio::test]
async fn tenant_isolation() {
    let (db, t1) = setup().await;
    let mut r = db
        .query("CREATE tenant SET name = 'Otra', slug = 'otra' RETURN id")
        .await
        .unwrap();
    let t2: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();

    service::create_supplier(&db, &t1, new_supplier("Only T1"))
        .await
        .unwrap();
    let seen = service::list_suppliers(&db, &t2, SupplierFilters::default())
        .await
        .unwrap();
    assert!(seen.is_empty(), "tenant 2 must not see tenant 1 suppliers");
}

// --- purchase orders (Fase 5-full, BACKLOG #8 slice 1) ---------------------

fn po_item(product: Option<&str>, name: &str, qty: i64, cost: &str) -> NewPurchaseOrderItem {
    NewPurchaseOrderItem {
        product: product.map(str::to_string),
        product_name: name.into(),
        quantity: qty,
        unit_cost: dec(cost),
    }
}

#[tokio::test]
async fn po_create_persists_header_lines_and_total_then_get_roundtrips() {
    let (db, t) = setup().await;
    let s = service::create_supplier(&db, &t, new_supplier("ACME"))
        .await
        .unwrap();
    let prod = catalog::create_product(&db, &t, new_product("Paracetamol", "1990", Some("900")))
        .await
        .unwrap();

    let input = NewPurchaseOrder {
        supplier: s.id.clone(),
        currency: None,
        notes: Some("reposición mensual".into()),
        external_ref: Some("OC-001".into()),
        items: vec![
            po_item(Some(&prod.id), "Paracetamol", 10, "900"),
            po_item(None, "Bolsas despacho", 200, "5"),
        ],
    };
    let po = service::create_purchase_order(&db, &t, input)
        .await
        .unwrap();

    assert_eq!(po.status, "draft");
    assert_eq!(po.currency, "CLP");
    assert_eq!(po.supplier, s.id);
    assert_eq!(po.items.len(), 2);
    // total = 10*900 + 200*5 = 9000 + 1000 = 10000.
    assert_eq!(po.total, dec("10000"));
    let line0 = po.items.iter().find(|i| i.product.is_some()).unwrap();
    assert_eq!(line0.product.as_deref(), Some(prod.id.as_str()));
    assert_eq!(line0.subtotal, dec("9000"));
    let line1 = po.items.iter().find(|i| i.product.is_none()).unwrap();
    assert_eq!(line1.product_name, "Bolsas despacho");
    assert_eq!(line1.subtotal, dec("1000"));

    // get returns the same document with its lines.
    let got = service::get_purchase_order(&db, &t, &po.id).await.unwrap();
    assert_eq!(got.id, po.id);
    assert_eq!(got.total, dec("10000"));
    assert_eq!(got.items.len(), 2);
    assert_eq!(got.notes.as_deref(), Some("reposición mensual"));
}

#[tokio::test]
async fn po_create_rejects_empty_items_bad_qty_and_negative_cost() {
    let (db, t) = setup().await;
    let s = service::create_supplier(&db, &t, new_supplier("S"))
        .await
        .unwrap();

    let empty = NewPurchaseOrder {
        supplier: s.id.clone(),
        currency: None,
        notes: None,
        external_ref: None,
        items: vec![],
    };
    assert_eq!(
        service::create_purchase_order(&db, &t, empty)
            .await
            .unwrap_err()
            .code(),
        "INVALID_INPUT"
    );

    let bad_qty = NewPurchaseOrder {
        supplier: s.id.clone(),
        currency: None,
        notes: None,
        external_ref: None,
        items: vec![po_item(None, "X", 0, "100")],
    };
    assert_eq!(
        service::create_purchase_order(&db, &t, bad_qty)
            .await
            .unwrap_err()
            .code(),
        "INVALID_INPUT"
    );

    let neg_cost = NewPurchaseOrder {
        supplier: s.id,
        currency: None,
        notes: None,
        external_ref: None,
        items: vec![po_item(None, "X", 1, "-5")],
    };
    assert_eq!(
        service::create_purchase_order(&db, &t, neg_cost)
            .await
            .unwrap_err()
            .code(),
        "INVALID_INPUT"
    );
}

#[tokio::test]
async fn po_create_rejects_supplier_from_other_tenant() {
    let (db, t1) = setup().await;
    let mut r = db
        .query("CREATE tenant SET name = 'Otra', slug = 'otra' RETURN id")
        .await
        .unwrap();
    let t2: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();
    let foreign = service::create_supplier(&db, &t2, new_supplier("Foreign"))
        .await
        .unwrap();

    let input = NewPurchaseOrder {
        supplier: foreign.id,
        currency: None,
        notes: None,
        external_ref: None,
        items: vec![po_item(None, "X", 1, "100")],
    };
    // Cross-tenant supplier must not be resolvable from t1.
    assert_eq!(
        service::create_purchase_order(&db, &t1, input)
            .await
            .unwrap_err()
            .code(),
        "INVALID_INPUT"
    );
}

#[tokio::test]
async fn po_list_filters_by_status_and_is_tenant_scoped() {
    let (db, t1) = setup().await;
    let mut r = db
        .query("CREATE tenant SET name = 'Otra', slug = 'otra' RETURN id")
        .await
        .unwrap();
    let t2: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();

    let s = service::create_supplier(&db, &t1, new_supplier("S"))
        .await
        .unwrap();
    service::create_purchase_order(
        &db,
        &t1,
        NewPurchaseOrder {
            supplier: s.id.clone(),
            currency: None,
            notes: None,
            external_ref: None,
            items: vec![po_item(None, "X", 1, "100")],
        },
    )
    .await
    .unwrap();

    let mut f = PurchaseOrderFilters::default();
    let all = service::list_purchase_orders(&db, &t1, f.clone())
        .await
        .unwrap();
    assert_eq!(all.len(), 1);
    assert!(all[0].items.is_empty(), "list is header-only");

    f.status = Some("received".into());
    let none = service::list_purchase_orders(&db, &t1, f).await.unwrap();
    assert!(none.is_empty(), "no draft matches status=received");

    let other = service::list_purchase_orders(&db, &t2, PurchaseOrderFilters::default())
        .await
        .unwrap();
    assert!(other.is_empty(), "tenant 2 must not see tenant 1 POs");
}

// --- purchase order receipt (Fase 5-full, BACKLOG #8 slice 2) --------------

async fn product_stock_cost(db: &Db, pid: &str) -> (i64, Option<Decimal>) {
    #[derive(serde::Deserialize)]
    struct R {
        stock: i64,
        cost_price: Option<Decimal>,
    }
    let t = surrealdb::sql::thing(pid).unwrap();
    let mut r = db
        .query("SELECT stock, cost_price FROM product WHERE id = $id LIMIT 1")
        .bind(("id", t))
        .await
        .unwrap();
    let row: Option<R> = r.take(0).unwrap();
    let row = row.unwrap();
    (row.stock, row.cost_price)
}

#[tokio::test]
async fn po_receive_bumps_stock_recomputes_wac_logs_movement_and_marks_received() {
    let (db, t) = setup().await;
    let s = service::create_supplier(&db, &t, new_supplier("ACME"))
        .await
        .unwrap();
    // Seed: 10 units at cost 100 (old WAC). Receipt: 30 units at cost 200.
    // New WAC = (10*100 + 30*200) / 40 = 7000 / 40 = 175.
    let prod = catalog::create_product(&db, &t, new_product("Paracetamol", "1990", Some("100")))
        .await
        .unwrap();
    let pid_thing = surrealdb::sql::thing(&prod.id).unwrap();
    // Seed stock=10 directly (Fase 3 inventory not in scope here).
    db.query("UPDATE product SET stock = 10 WHERE id = $p")
        .bind(("p", pid_thing.clone()))
        .await
        .unwrap();

    let po = service::create_purchase_order(
        &db,
        &t,
        NewPurchaseOrder {
            supplier: s.id.clone(),
            currency: None,
            notes: None,
            external_ref: None,
            items: vec![po_item(Some(&prod.id), "Paracetamol", 30, "200")],
        },
    )
    .await
    .unwrap();
    assert_eq!(po.status, "draft");

    let recv = service::receive_purchase_order(&db, &t, &po.id, None)
        .await
        .unwrap();
    assert_eq!(recv.status, "received");
    let (stock, cost) = product_stock_cost(&db, &prod.id).await;
    assert_eq!(stock, 40);
    assert_eq!(cost, Some(dec("175")));

    // Audit: one movement with reason=purchase_receipt, delta=+30, ref=po_id.
    #[derive(serde::Deserialize)]
    struct Mov {
        delta: i64,
        reason: String,
        #[serde(rename = "ref")]
        reff: Option<String>,
    }
    let mut q = db
        .query("SELECT delta, reason, ref FROM stock_movement WHERE tenant = $t")
        .bind(("t", t.clone()))
        .await
        .unwrap();
    let rows: Vec<Mov> = q.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].delta, 30);
    assert_eq!(rows[0].reason, "purchase_receipt");
    assert_eq!(rows[0].reff.as_deref(), Some(po.id.as_str()));
}

#[tokio::test]
async fn po_receive_first_receipt_seeds_cost_price_without_diluting_to_zero() {
    let (db, t) = setup().await;
    let s = service::create_supplier(&db, &t, new_supplier("S"))
        .await
        .unwrap();
    // Product with NO cost_price. First receipt must seed cost to line avg
    // (not (0*0 + 10*500)/10 which is 500 by accident — but break if we
    // multiplied by old_cost=None=0 and then divided by total). Here:
    // 5 units at 300 + 5 units at 700 = Σcost 5000, qty 10, avg 500.
    let prod = catalog::create_product(&db, &t, new_product("X", "1000", None))
        .await
        .unwrap();
    let po = service::create_purchase_order(
        &db,
        &t,
        NewPurchaseOrder {
            supplier: s.id,
            currency: None,
            notes: None,
            external_ref: None,
            items: vec![
                po_item(Some(&prod.id), "X", 5, "300"),
                po_item(Some(&prod.id), "X", 5, "700"),
            ],
        },
    )
    .await
    .unwrap();
    service::receive_purchase_order(&db, &t, &po.id, None)
        .await
        .unwrap();

    // Aggregated into ONE movement per product, not two.
    #[derive(serde::Deserialize)]
    struct Mov {
        delta: i64,
    }
    let mut q = db
        .query("SELECT delta FROM stock_movement WHERE tenant = $t")
        .bind(("t", t.clone()))
        .await
        .unwrap();
    let rows: Vec<Mov> = q.take(0).unwrap();
    assert_eq!(rows.len(), 1, "two lines on same product → one movement");
    assert_eq!(rows[0].delta, 10);

    let (stock, cost) = product_stock_cost(&db, &prod.id).await;
    assert_eq!(stock, 10);
    assert_eq!(cost, Some(dec("500")));
}

#[tokio::test]
async fn po_receive_skips_free_text_lines() {
    let (db, t) = setup().await;
    let s = service::create_supplier(&db, &t, new_supplier("S"))
        .await
        .unwrap();
    let po = service::create_purchase_order(
        &db,
        &t,
        NewPurchaseOrder {
            supplier: s.id,
            currency: None,
            notes: None,
            external_ref: None,
            items: vec![po_item(None, "Bolsas despacho", 100, "5")],
        },
    )
    .await
    .unwrap();
    let recv = service::receive_purchase_order(&db, &t, &po.id, None)
        .await
        .unwrap();
    assert_eq!(recv.status, "received");
    // No catalogued line → no stock_movement.
    let mut q = db
        .query("SELECT count() AS n FROM stock_movement WHERE tenant = $t GROUP ALL")
        .bind(("t", t.clone()))
        .await
        .unwrap();
    let n: Option<i64> = q.take((0, "n")).unwrap();
    assert_eq!(n.unwrap_or(0), 0);
}

#[tokio::test]
async fn po_receive_refuses_when_not_draft() {
    let (db, t) = setup().await;
    let s = service::create_supplier(&db, &t, new_supplier("S"))
        .await
        .unwrap();
    let prod = catalog::create_product(&db, &t, new_product("X", "100", Some("50")))
        .await
        .unwrap();
    let po = service::create_purchase_order(
        &db,
        &t,
        NewPurchaseOrder {
            supplier: s.id,
            currency: None,
            notes: None,
            external_ref: None,
            items: vec![po_item(Some(&prod.id), "X", 1, "50")],
        },
    )
    .await
    .unwrap();
    service::receive_purchase_order(&db, &t, &po.id, None)
        .await
        .unwrap();
    let err = service::receive_purchase_order(&db, &t, &po.id, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
}

// --- accounts payable (Fase 5-full, BACKLOG #8 slice 3) --------------------

async fn seed_po_with_total(db: &Db, t: &Thing, total: &str) -> String {
    let s = service::create_supplier(db, t, new_supplier("S"))
        .await
        .unwrap();
    service::create_purchase_order(
        db,
        t,
        NewPurchaseOrder {
            supplier: s.id,
            currency: None,
            notes: None,
            external_ref: None,
            items: vec![NewPurchaseOrderItem {
                product: None,
                product_name: "X".into(),
                quantity: 1,
                unit_cost: dec(total),
            }],
        },
    )
    .await
    .unwrap()
    .id
}

#[tokio::test]
async fn po_payment_records_and_summary_tracks_balance_until_fully_paid() {
    let (db, t) = setup().await;
    let po_id = seed_po_with_total(&db, &t, "10000").await;

    // Initial summary: nothing paid, full balance.
    let s0 = service::get_purchase_payment_summary(&db, &t, &po_id)
        .await
        .unwrap();
    assert_eq!(s0.total, dec("10000"));
    assert_eq!(s0.paid, dec("0"));
    assert_eq!(s0.balance, dec("10000"));
    assert!(!s0.fully_paid);
    assert!(s0.payments.is_empty());

    let p1 = service::create_purchase_payment(
        &db,
        &t,
        &po_id,
        NewPurchasePayment {
            amount: dec("4000"),
            currency: None,
            payment_method: Some("transfer".into()),
            cash_session: None,
            reference: Some("TR-1".into()),
            note: None,
            paid_at: None,
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(p1.amount, dec("4000"));
    assert_eq!(p1.payment_method, "transfer");
    // PO currency defaults to CLP (slice 1) → payment inherits unless overridden.
    assert_eq!(p1.currency, "CLP");

    let s1 = service::get_purchase_payment_summary(&db, &t, &po_id)
        .await
        .unwrap();
    assert_eq!(s1.paid, dec("4000"));
    assert_eq!(s1.balance, dec("6000"));
    assert!(!s1.fully_paid);

    // Second payment closes the balance exactly.
    service::create_purchase_payment(
        &db,
        &t,
        &po_id,
        NewPurchasePayment {
            amount: dec("6000"),
            currency: None,
            payment_method: Some("bank".into()),
            cash_session: None,
            reference: None,
            note: None,
            paid_at: None,
        },
        None,
    )
    .await
    .unwrap();
    let s2 = service::get_purchase_payment_summary(&db, &t, &po_id)
        .await
        .unwrap();
    assert_eq!(s2.paid, dec("10000"));
    assert_eq!(s2.balance, dec("0"));
    assert!(s2.fully_paid);
    assert_eq!(s2.payments.len(), 2);
    // List is chronological by paid_at ASC.
    assert_eq!(s2.payments[0].amount, dec("4000"));
    assert_eq!(s2.payments[1].amount, dec("6000"));
}

#[tokio::test]
async fn po_payment_refuses_amount_exceeding_balance() {
    let (db, t) = setup().await;
    let po_id = seed_po_with_total(&db, &t, "1000").await;

    service::create_purchase_payment(
        &db,
        &t,
        &po_id,
        NewPurchasePayment {
            amount: dec("700"),
            currency: None,
            payment_method: Some("cash".into()),
            cash_session: None,
            reference: None,
            note: None,
            paid_at: None,
        },
        None,
    )
    .await
    .unwrap();

    // Remaining 300, intentar 500 → CONFLICT y nada se persiste.
    let err = service::create_purchase_payment(
        &db,
        &t,
        &po_id,
        NewPurchasePayment {
            amount: dec("500"),
            currency: None,
            payment_method: Some("cash".into()),
            cash_session: None,
            reference: None,
            note: None,
            paid_at: None,
        },
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");

    let s = service::get_purchase_payment_summary(&db, &t, &po_id)
        .await
        .unwrap();
    assert_eq!(s.paid, dec("700"));
    assert_eq!(s.payments.len(), 1);
}

#[tokio::test]
async fn po_payment_rejects_invalid_inputs_and_cross_tenant_po() {
    let (db, t1) = setup().await;
    let mut r = db
        .query("CREATE tenant SET name = 'Otra', slug = 'otra' RETURN id")
        .await
        .unwrap();
    let t2: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();
    let po_id = seed_po_with_total(&db, &t1, "1000").await;

    // amount <= 0.
    let err = service::create_purchase_payment(
        &db,
        &t1,
        &po_id,
        NewPurchasePayment {
            amount: dec("0"),
            currency: None,
            payment_method: None,
            cash_session: None,
            reference: None,
            note: None,
            paid_at: None,
        },
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");

    // payment_method desconocido.
    let err = service::create_purchase_payment(
        &db,
        &t1,
        &po_id,
        NewPurchasePayment {
            amount: dec("10"),
            currency: None,
            payment_method: Some("bitcoin".into()),
            cash_session: None,
            reference: None,
            note: None,
            paid_at: None,
        },
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");

    // PO de t1, intentado pagar desde t2 → NotFound (no leak cross-tenant).
    let err = service::create_purchase_payment(
        &db,
        &t2,
        &po_id,
        NewPurchasePayment {
            amount: dec("10"),
            currency: None,
            payment_method: Some("cash".into()),
            cash_session: None,
            reference: None,
            note: None,
            paid_at: None,
        },
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "NOT_FOUND");
}

#[tokio::test]
async fn po_payment_refuses_payment_to_cancelled_order() {
    let (db, t) = setup().await;
    let po_id = seed_po_with_total(&db, &t, "1000").await;
    let po_thing = surrealdb::sql::thing(&po_id).unwrap();
    db.query("UPDATE purchase_order SET status = 'cancelled' WHERE id = $id")
        .bind(("id", po_thing))
        .await
        .unwrap();

    let err = service::create_purchase_payment(
        &db,
        &t,
        &po_id,
        NewPurchasePayment {
            amount: dec("10"),
            currency: None,
            payment_method: Some("cash".into()),
            cash_session: None,
            reference: None,
            note: None,
            paid_at: None,
        },
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
}

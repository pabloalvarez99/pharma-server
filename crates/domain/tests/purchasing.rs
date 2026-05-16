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

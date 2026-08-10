//! Regression tests for the API-perf sweep (BUG-perf-002 class): the report
//! aggregates in `expenses::service` resolve product rows directly by record-id
//! (`FROM $ids`) instead of full-scanning the product table
//! (`FROM product WHERE id IN $ids`). At 50k SKUs the old shape scanned the
//! whole `product` table on every report call (`near_expiry`, `margins_daily`,
//! `stock_rotation`) and the executive dashboard that fans out to them.
//!
//! These lock CORRECTNESS — the record-id fetch must return the exact same rows
//! as the old `IN $ids` query, stay tenant-scoped, and tolerate unknown ids.
//! End-to-end report semantics are covered by `expenses.rs`; this file pins the
//! one query shape that changed.

use std::str::FromStr;

use chrono::{Duration, Utc};
use domain::catalog::{model::NewProduct, service as catalog};
use domain::expenses::{model::*, service};
use domain::sales::{model as smodel, service as sales};
use rust_decimal::Decimal;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

type Db = Surreal<surrealdb::engine::local::Db>;

async fn new_tenant(db: &Db, slug: &str) -> Thing {
    let mut r = db
        .query("CREATE tenant SET name = $n, slug = $s RETURN id")
        .bind(("n", format!("Farmacia {slug}")))
        .bind(("s", slug.to_string()))
        .await
        .unwrap();
    let t: Option<Thing> = r.take((0, "id")).unwrap();
    t.expect("tenant id")
}

async fn setup() -> Db {
    let db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations)
        .await
        .expect("migrations");
    db
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn th(id: &str) -> Thing {
    surrealdb::sql::thing(id).unwrap()
}

fn np(name: &str, stock: i64) -> NewProduct {
    NewProduct {
        name: name.into(),
        slug: None,
        description: None,
        price: dec("1500"),
        cost_price: Some(dec("400")),
        stock,
        category: None,
        image_url: None,
        external_id: None,
        laboratory: None,
        therapeutic_action: None,
        active_ingredient: None,
        prescription_type: None,
        presentation: None,
        physical_stock: None,
        discount_percent: None,
        attrs: None,
    }
}

/// Sell `qty` of `product` so an `order` + `order_item` exist for the reports.
async fn sell(db: &Db, tenant: &Thing, product_id: &str, name: &str, qty: i64) {
    let req = smodel::PosSaleRequest {
        items: vec![smodel::PosSaleItem {
            product: product_id.to_string(),
            product_name: name.to_string(),
            quantity: qty,
            unit_price: dec("1500"),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("100000")),
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
    sales::post_sale(db, tenant, None, None, None, req)
        .await
        .unwrap();
}

async fn seed_batch(db: &Db, tenant: &Thing, product: &Thing, code: &str, days: i64, stock: i64) {
    let expiry = Utc::now() + Duration::days(days);
    db.query(
        "CREATE product_batch SET tenant=$t, product=$p, batch_code=$c, \
         expiry_date=$e, stock=$s, active=true",
    )
    .bind(("t", tenant.clone()))
    .bind(("p", product.clone()))
    .bind(("c", code.to_string()))
    .bind(("e", surrealdb::sql::Datetime::from(expiry)))
    .bind(("s", stock))
    .await
    .unwrap()
    .check()
    .unwrap();
}

/// OLD slow shape — kept only to assert the new `FROM $ids` fetch is identical.
async fn old_product_names(db: &Db, tenant: &Thing, ids: &[Thing]) -> Vec<(String, String)> {
    #[derive(serde::Deserialize)]
    struct P {
        id: Thing,
        name: String,
    }
    let rows: Vec<P> = db
        .query("SELECT id, name FROM product WHERE tenant = $t AND id IN $ids")
        .bind(("t", tenant.clone()))
        .bind(("ids", ids.to_vec()))
        .await
        .unwrap()
        .check()
        .unwrap()
        .take(0)
        .unwrap();
    let mut out: Vec<_> = rows
        .into_iter()
        .map(|p| (p.id.to_string(), p.name))
        .collect();
    out.sort();
    out
}

async fn new_product_names(db: &Db, tenant: &Thing, ids: &[Thing]) -> Vec<(String, String)> {
    #[derive(serde::Deserialize)]
    struct P {
        id: Thing,
        name: String,
    }
    let rows: Vec<P> = db
        .query("SELECT id, name FROM $ids WHERE tenant = $t")
        .bind(("t", tenant.clone()))
        .bind(("ids", ids.to_vec()))
        .await
        .unwrap()
        .check()
        .unwrap()
        .take(0)
        .unwrap();
    let mut out: Vec<_> = rows
        .into_iter()
        .map(|p| (p.id.to_string(), p.name))
        .collect();
    out.sort();
    out
}

/// The query shape that changed in all three report fns is identical and
/// tenant-isolated to the old `IN $ids` shape.
#[tokio::test]
async fn record_id_fetch_matches_old_in_query_and_is_tenant_scoped() {
    let db = setup().await;
    let ta = new_tenant(&db, "a").await;
    let tb = new_tenant(&db, "b").await;
    let a = catalog::create_product(&db, &ta, np("Paracetamol", 50))
        .await
        .unwrap();
    let b = catalog::create_product(&db, &ta, np("Ibuprofeno", 30))
        .await
        .unwrap();
    let foreign = catalog::create_product(&db, &tb, np("Ajeno", 10))
        .await
        .unwrap();
    let ghost = Thing::from(("product", "does_not_exist"));

    // Mix in a foreign-tenant id and a ghost id: both must be dropped.
    let ids = vec![th(&a.id), th(&b.id), th(&foreign.id), ghost];
    let new = new_product_names(&db, &ta, &ids).await;
    let old = old_product_names(&db, &ta, &ids).await;
    assert_eq!(new, old, "record-id fetch must match the old IN $ids query");
    assert_eq!(new.len(), 2, "foreign-tenant + ghost ids excluded");
}

/// `near_expiry` resolves product names via the changed query; result unchanged.
#[tokio::test]
async fn near_expiry_resolves_names_after_record_fetch() {
    let db = setup().await;
    let t = new_tenant(&db, "ne").await;
    let p = catalog::create_product(&db, &t, np("Amoxicilina", 20))
        .await
        .unwrap();
    seed_batch(&db, &t, &th(&p.id), "L1", 10, 5).await;

    let rows = service::near_expiry(&db, &t, NearExpiryFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].product_id, p.id);
    assert_eq!(rows[0].product_name, "Amoxicilina");
}

/// `margins_daily` resolves product costs via the changed query; margin holds.
#[tokio::test]
async fn margins_daily_resolves_costs_after_record_fetch() {
    let db = setup().await;
    let t = new_tenant(&db, "md").await;
    let p = catalog::create_product(&db, &t, np("Vitamina C", 100))
        .await
        .unwrap();
    sell(&db, &t, &p.id, "Vitamina C", 2).await; // revenue 2*1500=3000, cost 2*400=800

    let rows = service::margins_daily(&db, &t, SalesReportFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].revenue, dec("3000"));
    assert_eq!(rows[0].cost, dec("800"));
    assert_eq!(rows[0].margin, dec("2200"));
    assert_eq!(rows[0].items_without_cost, 0);
}

/// `stock_rotation` resolves product name+stock via the changed query.
#[tokio::test]
async fn stock_rotation_resolves_stock_after_record_fetch() {
    let db = setup().await;
    let t = new_tenant(&db, "sr").await;
    let p = catalog::create_product(&db, &t, np("Suero", 10))
        .await
        .unwrap();
    sell(&db, &t, &p.id, "Suero", 4).await; // stock 10 -> 6, qty_sold 4

    let rows = service::stock_rotation(&db, &t, SalesReportFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].product_id, p.id);
    assert_eq!(rows[0].product_name, "Suero");
    assert_eq!(rows[0].qty_sold, 4);
    assert_eq!(rows[0].current_stock, 6);
}

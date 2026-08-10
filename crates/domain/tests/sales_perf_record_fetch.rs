//! Regression tests for BUG-perf-002: the POS write hot path
//! (`load_products_for_sale` + `load_active_ingredients`) must fetch products
//! directly by record-id (`FROM $ids`) instead of full-scanning the product
//! table (`FROM product WHERE id IN $ids`). These tests lock CORRECTNESS — the
//! record-id fetch must return the exact same rows as the old `IN $ids` query,
//! stay tenant-scoped, exclude inactive products, and tolerate unknown ids.
//! (Performance itself is measured by the Criterion bench `pos_hotpath`.)

use domain::catalog::{model::NewProduct, service as catalog};
use domain::sales::{repo, service};
use rust_decimal::Decimal;
use std::str::FromStr;
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

/// catalog Dtos expose `id` as a `String`; the repo/service take `Thing`.
fn th(id: &str) -> Thing {
    surrealdb::sql::thing(id).unwrap()
}

fn np(name: &str, stock: i64, ingredient: Option<&str>) -> NewProduct {
    NewProduct {
        name: name.into(),
        slug: None,
        description: None,
        price: dec("1500"),
        cost_price: Some(dec("100")),
        stock,
        category: None,
        image_url: None,
        external_id: None,
        laboratory: None,
        therapeutic_action: None,
        active_ingredient: ingredient.map(Into::into),
        prescription_type: None,
        presentation: None,
        physical_stock: None,
        discount_percent: None,
        attrs: None,
    }
}

/// The OLD, slow implementation — kept here only to assert the new record-id
/// fetch returns an IDENTICAL result set. (id, stock, name) sorted by id.
async fn old_load_products(db: &Db, tenant: &Thing, ids: &[Thing]) -> Vec<(String, i64, String)> {
    #[derive(serde::Deserialize)]
    struct Row {
        id: Thing,
        stock: i64,
        name: String,
    }
    let rows: Vec<Row> = db
        .query("SELECT id, stock, name FROM product WHERE tenant = $t AND active = true AND id IN $ids")
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
        .map(|r| (r.id.to_string(), r.stock, r.name))
        .collect();
    out.sort();
    out
}

fn norm(rows: Vec<repo::StockCheckOut>) -> Vec<(String, i64, String)> {
    let mut out: Vec<_> = rows
        .into_iter()
        .map(|r| (r.id.to_string(), r.stock, r.name))
        .collect();
    out.sort();
    out
}

#[tokio::test]
async fn load_products_for_sale_matches_old_in_query() {
    let db = setup().await;
    let t = new_tenant(&db, "test").await;
    let a = catalog::create_product(&db, &t, np("Paracetamol", 50, None))
        .await
        .unwrap();
    let b = catalog::create_product(&db, &t, np("Ibuprofeno", 30, None))
        .await
        .unwrap();
    let c = catalog::create_product(&db, &t, np("Aspirina", 12, None))
        .await
        .unwrap();
    let ids = vec![th(&a.id), th(&b.id), th(&c.id)];

    let new = norm(repo::load_products_for_sale(&db, &t, &ids).await.unwrap());
    let old = old_load_products(&db, &t, &ids).await;
    assert_eq!(new, old, "record-id fetch must match the old IN $ids query");
    assert_eq!(new.len(), 3);
}

#[tokio::test]
async fn load_products_for_sale_is_tenant_isolated() {
    let db = setup().await;
    let ta = new_tenant(&db, "a").await;
    let tb = new_tenant(&db, "b").await;
    let pa = catalog::create_product(&db, &ta, np("Solo A", 5, None))
        .await
        .unwrap();
    let pb = catalog::create_product(&db, &tb, np("Solo B", 7, None))
        .await
        .unwrap();

    // Ask tenant A for BOTH ids — must only get A's product back.
    let rows = repo::load_products_for_sale(&db, &ta, &[th(&pa.id), th(&pb.id)])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id.to_string(), pa.id);
    assert_eq!(rows[0].name, "Solo A");
}

#[tokio::test]
async fn load_products_for_sale_excludes_inactive() {
    let db = setup().await;
    let t = new_tenant(&db, "test").await;
    let active = catalog::create_product(&db, &t, np("Activo", 10, None))
        .await
        .unwrap();
    let dead = catalog::create_product(&db, &t, np("Inactivo", 10, None))
        .await
        .unwrap();
    db.query("UPDATE $id SET active = false")
        .bind(("id", th(&dead.id)))
        .await
        .unwrap()
        .check()
        .unwrap();

    let rows = repo::load_products_for_sale(&db, &t, &[th(&active.id), th(&dead.id)])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id.to_string(), active.id);
}

#[tokio::test]
async fn load_products_for_sale_tolerates_missing_ids() {
    let db = setup().await;
    let t = new_tenant(&db, "test").await;
    let real = catalog::create_product(&db, &t, np("Real", 9, None))
        .await
        .unwrap();
    let ghost = Thing::from(("product", "does_not_exist"));

    let rows = repo::load_products_for_sale(&db, &t, &[th(&real.id), ghost])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id.to_string(), real.id);

    // empty input short-circuits to empty, no query.
    let empty = repo::load_products_for_sale(&db, &t, &[]).await.unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn load_active_ingredients_returns_ingredients() {
    let db = setup().await;
    let t = new_tenant(&db, "test").await;
    let a = catalog::create_product(&db, &t, np("Con AI", 5, Some("warfarina")))
        .await
        .unwrap();
    let b = catalog::create_product(&db, &t, np("Sin AI", 5, None))
        .await
        .unwrap();

    let mut ai = service::load_active_ingredients(&db, &t, &[th(&a.id), th(&b.id)])
        .await
        .unwrap();
    ai.sort();
    // null active_ingredient is dropped silently; only "warfarina" remains.
    assert_eq!(ai, vec!["warfarina".to_string()]);
}

#[tokio::test]
async fn load_active_ingredients_is_tenant_isolated_and_missing_safe() {
    let db = setup().await;
    let ta = new_tenant(&db, "a").await;
    let tb = new_tenant(&db, "b").await;
    let pa = catalog::create_product(&db, &ta, np("A", 5, Some("aspirina")))
        .await
        .unwrap();
    let pb = catalog::create_product(&db, &tb, np("B", 5, Some("ibuprofeno")))
        .await
        .unwrap();
    let ghost = Thing::from(("product", "nope"));

    // Tenant A asking for A+B+ghost ids gets only A's ingredient.
    let ai = service::load_active_ingredients(&db, &ta, &[th(&pa.id), th(&pb.id), ghost])
        .await
        .unwrap();
    assert_eq!(ai, vec!["aspirina".to_string()]);

    let empty = service::load_active_ingredients(&db, &ta, &[])
        .await
        .unwrap();
    assert!(empty.is_empty());
}

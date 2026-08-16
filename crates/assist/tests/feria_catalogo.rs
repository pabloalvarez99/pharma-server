//! Integration tests for feria simple-product ensure (agent first sale without SKU).

use std::str::FromStr;

use assist::feria_catalogo::{asegurar_cosa_feria, precio_dicho};
use domain::catalog::model::ProductFilters;
use domain::catalog::service as catalog;
use rust_decimal::Decimal;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

type Db = Surreal<surrealdb::engine::local::Db>;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

async fn setup() -> (Db, Thing) {
    let db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations).await.expect("migr");
    let tenant: Thing = db
        .query("CREATE tenant SET name='Feria A', slug='feria-a' RETURN id")
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    (db, tenant)
}

async fn setup_tenant(db: &Db, name: &str, slug: &str) -> Thing {
    db.query("CREATE tenant SET name=$n, slug=$s RETURN id")
        .bind(("n", name.to_string()))
        .bind(("s", slug.to_string()))
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn crear_tomates_physical_stock_false_stock_cero() {
    let (db, tenant) = setup().await;
    let p = asegurar_cosa_feria(&db, &tenant, "Tomates", dec("2000"))
        .await
        .expect("create");
    assert_eq!(p.name, "Tomates");
    assert_eq!(p.price, dec("2000"));
    assert_eq!(p.stock, 0);
    assert!(!p.physical_stock, "feria simple must not track physical stock");
    let attrs = p.attrs.expect("attrs");
    assert_eq!(attrs["rb_simple"], true);
}

#[tokio::test]
async fn segunda_llamada_mismo_id_sin_cambiar_precio() {
    let (db, tenant) = setup().await;
    let first = asegurar_cosa_feria(&db, &tenant, "Tomates", dec("2000"))
        .await
        .expect("first");
    let second = asegurar_cosa_feria(&db, &tenant, "tomates", dec("9999"))
        .await
        .expect("second");
    assert_eq!(first.id, second.id, "idempotent by name ignore-case");
    assert_eq!(second.price, dec("2000"), "must not patch price on hit");
    assert_eq!(second.name, "Tomates", "keeps original casing");
}

#[test]
fn precio_dicho_parsea_cola_y_none() {
    assert_eq!(precio_dicho("tomates a $2.000"), Some(dec("2000")));
    assert_eq!(precio_dicho("tomates"), None);
}

#[tokio::test]
async fn tenant_b_no_ve_producto_de_a() {
    let db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations).await.expect("migr");

    let tenant_a = setup_tenant(&db, "Feria A", "feria-a").await;
    let tenant_b = setup_tenant(&db, "Feria B", "feria-b").await;

    let a = asegurar_cosa_feria(&db, &tenant_a, "Tomates", dec("2000"))
        .await
        .expect("A");
    assert_eq!(a.name, "Tomates");

    let en_b = catalog::list_products(
        &db,
        &tenant_b,
        ProductFilters {
            search: Some("Tomates".into()),
            active: Some(true),
            limit: Some(10),
            ..ProductFilters::default()
        },
    )
    .await
    .expect("list B");
    assert!(
        en_b.iter().all(|p| p.id != a.id),
        "tenant B must not see A's product"
    );

    let b = asegurar_cosa_feria(&db, &tenant_b, "Tomates", dec("2000"))
        .await
        .expect("B create");
    assert_ne!(a.id, b.id, "each tenant gets its own product row");
}

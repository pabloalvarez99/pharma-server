//! Ensure de producto simple de feria: nombre + precio, sin inventario.
//!
//! «Tomates $2000» no es un SKU con barcode ni stock físico. El ensure crea (o
//! reusa) un producto vendible con `physical_stock = false` y stock 0, para que
//! la venta no muera con «Te quedaste sin stock». Idempotente por nombre.

use domain::catalog::feria::ensure_simple_product;
use domain::{DomainError, DomainResult};
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
        .query("CREATE tenant SET name = 'Puesto Test', slug = 'puesto-a' RETURN id")
        .await
        .unwrap();
    let id: Option<Thing> = r.take((0, "id")).unwrap();
    (db, id.expect("tenant id"))
}

async fn tenant_b(db: &Db) -> Thing {
    let mut r = db
        .query("CREATE tenant SET name = 'Otro Puesto', slug = 'puesto-b' RETURN id")
        .await
        .unwrap();
    let id: Option<Thing> = r.take((0, "id")).unwrap();
    id.expect("tenant b")
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn is_invalid(r: DomainResult<impl std::fmt::Debug>) -> bool {
    matches!(r, Err(DomainError::Invalid(_)))
}

#[tokio::test]
async fn crea_tomates_sin_inventario() {
    let (db, t) = setup().await;
    let p = ensure_simple_product(&db, &t, "Tomates", dec("2000"))
        .await
        .unwrap();
    assert_eq!(p.name, "Tomates");
    assert_eq!(p.price, dec("2000"));
    assert!(!p.physical_stock, "feria informal = servicio, sin stock");
    assert_eq!(p.stock, 0);
    let attrs = p.attrs.expect("attrs");
    assert_eq!(attrs["rb_simple"], true);
    assert!(attrs.get("rb_venta_suelta").is_none());
}

#[tokio::test]
async fn segundo_ensure_mismo_nombre_reusa_id_y_precio() {
    let (db, t) = setup().await;
    let a = ensure_simple_product(&db, &t, "Tomates", dec("2000"))
        .await
        .unwrap();
    let b = ensure_simple_product(&db, &t, "tomates", dec("9999"))
        .await
        .unwrap();
    assert_eq!(a.id, b.id, "idempotente por nombre (case-insensitive)");
    assert_eq!(b.price, dec("2000"), "no patch de precio en ensure");
    assert_eq!(b.name, "Tomates", "conserva el nombre original");
}

#[tokio::test]
async fn tenant_b_no_ve_producto_de_a() {
    let (db, ta) = setup().await;
    let tb = tenant_b(&db).await;
    let a = ensure_simple_product(&db, &ta, "Cilantro", dec("500"))
        .await
        .unwrap();
    let b = ensure_simple_product(&db, &tb, "Cilantro", dec("500"))
        .await
        .unwrap();
    assert_ne!(a.id, b.id, "aislamiento multi-tenant");
}

#[tokio::test]
async fn name_vacio_y_precio_negativo_son_invalid() {
    let (db, t) = setup().await;
    assert!(is_invalid(
        ensure_simple_product(&db, &t, "  ", dec("100")).await
    ));
    assert!(is_invalid(
        ensure_simple_product(&db, &t, "", dec("100")).await
    ));
    assert!(is_invalid(
        ensure_simple_product(&db, &t, "Tomates", dec("-1")).await
    ));
}

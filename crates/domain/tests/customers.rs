//! Customers integration tests on in-memory SurrealDB (`kv-mem`).

use domain::customers::{model::*, service};
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

fn nc(name: &str, rut: Option<&str>) -> NewCustomer {
    NewCustomer {
        name: name.into(),
        rut: rut.map(str::to_string),
        phone: None,
        email: None,
    }
}

#[tokio::test]
async fn create_then_get() {
    let (db, t) = setup().await;
    let c = service::create_customer(&db, &t, nc("Juan Pérez", Some("12.345.678-9")))
        .await
        .unwrap();
    assert_eq!(c.name, "Juan Pérez");
    assert_eq!(c.rut.as_deref(), Some("123456789"));
    assert_eq!(c.loyalty_points, 0);
    assert!(c.active);
    let fetched = service::get_customer(&db, &t, &c.id).await.unwrap();
    assert_eq!(fetched.id, c.id);
}

#[tokio::test]
async fn rut_unique_per_tenant() {
    let (db, t) = setup().await;
    service::create_customer(&db, &t, nc("A", Some("11111111-1")))
        .await
        .unwrap();
    let err = service::create_customer(&db, &t, nc("B", Some("11.111.111-1")))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
}

#[tokio::test]
async fn soft_delete_keeps_row() {
    let (db, t) = setup().await;
    let c = service::create_customer(&db, &t, nc("X", None))
        .await
        .unwrap();
    service::delete_customer(&db, &t, &c.id).await.unwrap();
    let after = service::get_customer(&db, &t, &c.id).await.unwrap();
    assert!(!after.active, "soft delete flips active");
}

#[tokio::test]
async fn tenant_isolation() {
    let (db, t1) = setup().await;
    let mut r = db
        .query("CREATE tenant SET name = 'Otra', slug = 'otra' RETURN id")
        .await
        .unwrap();
    let t2: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();
    // Same RUT is OK across tenants.
    service::create_customer(&db, &t1, nc("T1", Some("9-9")))
        .await
        .unwrap();
    service::create_customer(&db, &t2, nc("T2", Some("9-9")))
        .await
        .unwrap();
    let seen = service::list_customers(&db, &t2, CustomerFilters::default())
        .await
        .unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].name, "T2");
}

#[tokio::test]
async fn loyalty_stats_empty_until_sales() {
    let (db, t) = setup().await;
    service::create_customer(&db, &t, nc("A", None))
        .await
        .unwrap();
    let s = service::loyalty_stats(&db, &t).await.unwrap();
    assert_eq!(s.members, 1);
    assert_eq!(s.points_outstanding, 0);
    assert_eq!(s.points_earned_30d, 0);
    assert!(s.top_customers.is_empty());
    assert!(s.pending_sales_integration);
}

#[tokio::test]
async fn update_rejects_rut_collision() {
    let (db, t) = setup().await;
    service::create_customer(&db, &t, nc("A", Some("1-1")))
        .await
        .unwrap();
    let b = service::create_customer(&db, &t, nc("B", Some("2-2")))
        .await
        .unwrap();
    let err = service::update_customer(
        &db,
        &t,
        &b.id,
        UpdateCustomer {
            rut: Some("1-1".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
}

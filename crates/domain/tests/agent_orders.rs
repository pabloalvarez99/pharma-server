//! Inbound agent_order operator-side tests (kv-mem). Covers tenant-scoped
//! listing, the received→accepted/rejected transition guard, and isolation.

use domain::agent_orders::{model::*, service};
use domain::DomainError;
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
        .query("CREATE tenant SET name = 'Sup', slug = 'sup' RETURN id")
        .await
        .unwrap();
    let id: Option<Thing> = r.take((0, "id")).unwrap();
    (db, id.expect("tenant id"))
}

async fn seed_order(db: &Db, tenant: &Thing, peer: &str, total: f64) -> String {
    #[derive(serde::Deserialize)]
    struct Row {
        id: Thing,
    }
    let mut r = db
        .query(
            "CREATE agent_order SET tenant=$t, peer_did=$p, \
             lines_json='[{\"barcode\":\"X\",\"qty\":2}]', total=$tot, \
             status='received', price_adjusted=true RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", peer.to_string()))
        .bind(("tot", total))
        .await
        .unwrap();
    r.take::<Option<Row>>(0).unwrap().unwrap().id.to_string()
}

#[tokio::test]
async fn list_is_tenant_scoped_and_status_filterable() {
    let (db, tenant) = setup().await;
    let other = {
        let mut r = db
            .query("CREATE tenant SET name='Other', slug='other' RETURN id")
            .await
            .unwrap();
        r.take::<Option<Thing>>((0, "id")).unwrap().unwrap()
    };
    seed_order(&db, &tenant, "did:pharma:buyerA", 1000.0).await;
    seed_order(&db, &tenant, "did:pharma:buyerB", 2000.0).await;
    seed_order(&db, &other, "did:pharma:buyerC", 9999.0).await;

    let all = service::list(&db, &tenant, AgentOrderFilters::default())
        .await
        .unwrap();
    assert_eq!(all.len(), 2, "only this tenant's orders");
    assert!(all.iter().all(|o| o.status == "received"));
    assert!(all.iter().any(|o| (o.total - 2000.0).abs() < f64::EPSILON));
    // lines_json decoded into a JSON array.
    assert!(all[0].lines.is_array());

    let none_accepted = service::list(
        &db,
        &tenant,
        AgentOrderFilters {
            status: Some("accepted".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(none_accepted.is_empty());
}

#[tokio::test]
async fn accept_then_redeciding_is_conflict() {
    let (db, tenant) = setup().await;
    let id = seed_order(&db, &tenant, "did:pharma:b", 500.0).await;

    let accepted = service::decide(&db, &tenant, &id, "accepted")
        .await
        .unwrap();
    assert_eq!(accepted.status, "accepted");
    assert!(accepted.price_adjusted);

    let got = service::get(&db, &tenant, &id).await.unwrap();
    assert_eq!(got.status, "accepted");

    let err = service::decide(&db, &tenant, &id, "rejected")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
}

#[tokio::test]
async fn reject_from_received_works() {
    let (db, tenant) = setup().await;
    let id = seed_order(&db, &tenant, "did:pharma:b", 700.0).await;
    let r = service::decide(&db, &tenant, &id, "rejected")
        .await
        .unwrap();
    assert_eq!(r.status, "rejected");
}

#[tokio::test]
async fn invalid_target_status_rejected() {
    let (db, tenant) = setup().await;
    let id = seed_order(&db, &tenant, "did:pharma:b", 100.0).await;
    let err = service::decide(&db, &tenant, &id, "fulfilled")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
    // unchanged
    assert_eq!(
        service::get(&db, &tenant, &id).await.unwrap().status,
        "received"
    );
}

#[tokio::test]
async fn cross_tenant_get_is_not_found() {
    let (db, tenant) = setup().await;
    let other = {
        let mut r = db
            .query("CREATE tenant SET name='O2', slug='o2' RETURN id")
            .await
            .unwrap();
        r.take::<Option<Thing>>((0, "id")).unwrap().unwrap()
    };
    let id = seed_order(&db, &tenant, "did:pharma:b", 300.0).await;
    let err = service::get(&db, &other, &id).await.unwrap_err();
    assert!(matches!(err, DomainError::NotFound));
}

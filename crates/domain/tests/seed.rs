//! Demo seeding service tests (kv-mem). Locks the stock-ledger invariant the
//! seed must honor and the force/idempotency semantics.

use domain::seed::{seed_demo, SeedSummary};
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

type Db = Surreal<surrealdb::engine::local::Db>;

async fn setup() -> (Db, Thing) {
    let db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations).await.expect("migr");
    let tenant: Thing = db
        .query("CREATE tenant SET name='T', slug='t' RETURN id")
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    (db, tenant)
}

/// product.stock == Σ product_batch.stock == Σ stock_movement.delta, per tenant.
async fn assert_ledger_consistent(db: &Db, tenant: &Thing) {
    let mut r = db
        .query("SELECT id, stock FROM product WHERE tenant = $t")
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    #[derive(serde::Deserialize)]
    struct P {
        id: Thing,
        stock: i64,
    }
    let products: Vec<P> = r.take(0).unwrap();
    assert!(!products.is_empty(), "esperaba productos sembrados");
    for p in products {
        let mut br = db
            .query("SELECT VALUE stock FROM product_batch WHERE tenant = $t AND product = $p")
            .bind(("t", tenant.clone()))
            .bind(("p", p.id.clone()))
            .await
            .unwrap();
        let batch_stocks: Vec<i64> = br.take(0).unwrap();
        let batch_sum: i64 = batch_stocks.iter().sum();

        let mut mr = db
            .query("SELECT VALUE delta FROM stock_movement WHERE tenant = $t AND product = $p")
            .bind(("t", tenant.clone()))
            .bind(("p", p.id.clone()))
            .await
            .unwrap();
        let deltas: Vec<i64> = mr.take(0).unwrap();
        let delta_sum: i64 = deltas.iter().sum();

        assert_eq!(
            p.stock, batch_sum,
            "product.stock == Σ batch.stock ({})",
            p.id
        );
        assert_eq!(
            p.stock, delta_sum,
            "product.stock == Σ movement.delta ({})",
            p.id
        );
    }
}

#[tokio::test]
async fn seeds_pharmacy_and_honors_stock_ledger_invariant() {
    let (db, tenant) = setup().await;
    let s = seed_demo(&db, &tenant, "pharmacy", false).await.unwrap();
    assert_eq!(s.vertical, "pharmacy");
    assert!(s.products_created >= 5);
    assert_eq!(s.products_created, s.batches_created);
    assert_eq!(s.movements_emitted, s.batches_created); // every demo batch has stock>0
    assert_eq!(s.wiped, 0);
    assert_ledger_consistent(&db, &tenant).await;
}

#[tokio::test]
async fn seeds_minimarket_pack() {
    let (db, tenant) = setup().await;
    let s = seed_demo(&db, &tenant, "minimarket", false).await.unwrap();
    assert_eq!(s.vertical, "minimarket");
    assert!(s.products_created >= 5);
    assert_ledger_consistent(&db, &tenant).await;
}

#[tokio::test]
async fn second_seed_without_force_is_rejected() {
    let (db, tenant) = setup().await;
    seed_demo(&db, &tenant, "pharmacy", false).await.unwrap();
    let err = seed_demo(&db, &tenant, "pharmacy", false)
        .await
        .expect_err("debe rechazar re-seed sin force");
    assert!(format!("{err}").to_lowercase().contains("demo"));
}

#[tokio::test]
async fn force_wipes_prior_demo_then_reseeds_no_duplicates() {
    let (db, tenant) = setup().await;
    let first = seed_demo(&db, &tenant, "pharmacy", false).await.unwrap();
    let again: SeedSummary = seed_demo(&db, &tenant, "pharmacy", true).await.unwrap();
    assert_eq!(again.wiped, first.products_created);

    // After a force re-seed the catalog has exactly one demo pack (no dupes) and
    // the ledger is still consistent.
    let mut r = db
        .query(
            "SELECT VALUE id FROM product \
             WHERE tenant = $t AND string::starts_with(external_id, 'DEMO-')",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let ids: Vec<Thing> = r.take(0).unwrap();
    assert_eq!(ids.len(), again.products_created);
    assert_ledger_consistent(&db, &tenant).await;
}

#[tokio::test]
async fn unknown_vertical_is_rejected() {
    let (db, tenant) = setup().await;
    assert!(seed_demo(&db, &tenant, "casino", false).await.is_err());
}

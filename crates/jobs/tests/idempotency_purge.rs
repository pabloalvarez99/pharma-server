//! Integration test for the daily idempotency-purge cron body
//! ([`jobs::run_idempotency_purge`]) against an in-memory SurrealDB.
//!
//! Seeds 5 `idempotency_key` rows (3 already expired, 2 still live) and asserts
//! exactly the 2 live rows survive. `run_idempotency_purge` delegates to
//! `domain::sales::service::purge_expired_idempotency`, which keys off
//! `expires_at` (= insert time + 24h), so the fixtures set `expires_at`
//! directly to control past/future.

use chrono::{Duration, Utc};
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
        .expect("migrations");
    let mut r = db
        .query("CREATE tenant SET name = 'Farmacia Test', slug = 'test' RETURN id")
        .await
        .unwrap();
    let tenant: Option<Thing> = r.take((0, "id")).unwrap();
    (db, tenant.expect("tenant id"))
}

async fn seed_key(db: &Db, tenant: &Thing, key: &str, expires_at: chrono::DateTime<Utc>) {
    db.query(
        "CREATE idempotency_key SET tenant=$t, key=$k, response_json='{}', \
         status_code=200, expires_at=$e",
    )
    .bind(("t", tenant.clone()))
    .bind(("k", key.to_string()))
    .bind(("e", surrealdb::sql::Datetime::from(expires_at)))
    .await
    .unwrap()
    .check()
    .unwrap();
}

#[tokio::test]
async fn idempotency_purge_drops_only_expired_keys() {
    let (db, tenant) = setup().await;
    let now = Utc::now();

    // 3 expired (expires_at in the past).
    seed_key(&db, &tenant, "exp-1", now - Duration::hours(1)).await;
    seed_key(&db, &tenant, "exp-2", now - Duration::hours(5)).await;
    seed_key(&db, &tenant, "exp-3", now - Duration::days(2)).await;
    // 2 live (expires_at in the future).
    seed_key(&db, &tenant, "live-1", now + Duration::hours(1)).await;
    seed_key(&db, &tenant, "live-2", now + Duration::hours(12)).await;

    // Sanity: 5 rows before the purge.
    assert_eq!(count_keys(&db).await, 5);

    jobs::run_idempotency_purge(&db).await;

    // 2 live rows remain.
    let remaining = remaining_keys(&db).await;
    assert_eq!(
        remaining.len(),
        2,
        "expected 2 surviving keys, got {remaining:?}"
    );
    assert!(remaining.contains(&"live-1".to_string()));
    assert!(remaining.contains(&"live-2".to_string()));

    // Second run is a no-op (nothing left to expire).
    jobs::run_idempotency_purge(&db).await;
    assert_eq!(count_keys(&db).await, 2);
}

async fn count_keys(db: &Db) -> usize {
    remaining_keys(db).await.len()
}

async fn remaining_keys(db: &Db) -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct R {
        key: String,
    }
    let mut q = db
        .query("SELECT key FROM idempotency_key ORDER BY key")
        .await
        .unwrap();
    let rows: Vec<R> = q.take(0).unwrap();
    rows.into_iter().map(|r| r.key).collect()
}

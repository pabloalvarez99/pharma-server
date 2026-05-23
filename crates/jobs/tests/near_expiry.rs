//! Integration test for the near-expiry alert cron body
//! ([`jobs::run_near_expiry_alert`]) against an in-memory SurrealDB.
//!
//! Verifies the opt-in gate + the 30-day / stock / active filter:
//! * Tenant A flips `admin_setting near_expiry_alert_enabled = 'true'` and owns
//!   a mix of batches (2 urgent, 1 far, 1 zero-stock, 1 inactive). It gets
//!   exactly one `notification` row of `kind='near_expiry'` whose payload counts
//!   only the 2 urgent+in-stock+active batches.
//! * Tenant B has urgent batches but no setting → zero notifications (default
//!   OFF, no spam for existing installs).

use chrono::{Duration, Utc};
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

type Db = Surreal<surrealdb::engine::local::Db>;

async fn setup() -> Db {
    let db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations)
        .await
        .expect("migrations");
    db
}

async fn make_tenant(db: &Db, slug: &str) -> Thing {
    let mut r = db
        .query("CREATE tenant SET name = $n, slug = $s RETURN id")
        .bind(("n", format!("Farmacia {slug}")))
        .bind(("s", slug.to_string()))
        .await
        .unwrap();
    let t: Option<Thing> = r.take((0, "id")).unwrap();
    t.expect("tenant id")
}

async fn make_product(db: &Db, tenant: &Thing, slug: &str) -> Thing {
    let mut r = db
        .query("CREATE product SET tenant=$t, name=$n, slug=$s, price=1000 RETURN id")
        .bind(("t", tenant.clone()))
        .bind(("n", format!("Producto {slug}")))
        .bind(("s", slug.to_string()))
        .await
        .unwrap();
    let p: Option<Thing> = r.take((0, "id")).unwrap();
    p.expect("product id")
}

#[allow(clippy::too_many_arguments)]
async fn make_batch(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
    code: &str,
    expiry: chrono::DateTime<Utc>,
    stock: i64,
    active: bool,
) {
    db.query(
        "CREATE product_batch SET tenant=$t, product=$p, batch_code=$c, \
         expiry_date=$e, stock=$s, active=$a",
    )
    .bind(("t", tenant.clone()))
    .bind(("p", product.clone()))
    .bind(("c", code.to_string()))
    .bind(("e", surrealdb::sql::Datetime::from(expiry)))
    .bind(("s", stock))
    .bind(("a", active))
    .await
    .unwrap()
    .check()
    .unwrap();
}

async fn enable_setting(db: &Db, tenant: &Thing) {
    db.query("CREATE admin_setting SET tenant=$t, key='near_expiry_alert_enabled', value='true'")
        .bind(("t", tenant.clone()))
        .await
        .unwrap()
        .check()
        .unwrap();
}

#[derive(serde::Deserialize)]
struct NotifRow {
    kind: String,
    tenant: Thing,
    payload: serde_json::Value,
}

async fn notifications(db: &Db) -> Vec<NotifRow> {
    let mut q = db
        .query("SELECT kind, tenant, payload FROM notification")
        .await
        .unwrap();
    q.take(0).unwrap()
}

#[tokio::test]
async fn near_expiry_alerts_only_urgent_batches_for_opted_in_tenants() {
    let db = setup().await;
    let now = Utc::now();

    // --- Tenant A: opted in, mixed batches.
    let a = make_tenant(&db, "alpha").await;
    let pa = make_product(&db, &a, "alpha-prod").await;
    enable_setting(&db, &a).await;
    // Urgent + qualifying (within 30d, stock > 0, active).
    make_batch(&db, &a, &pa, "A-URG-1", now + Duration::days(5), 10, true).await;
    make_batch(&db, &a, &pa, "A-URG-2", now + Duration::days(20), 3, true).await;
    // Far future (beyond 30d) — must be ignored.
    make_batch(&db, &a, &pa, "A-FAR", now + Duration::days(90), 50, true).await;
    // Urgent window but zero stock — must be ignored.
    make_batch(&db, &a, &pa, "A-ZERO", now + Duration::days(10), 0, true).await;
    // Urgent window but inactive — must be ignored.
    make_batch(&db, &a, &pa, "A-INACT", now + Duration::days(10), 5, false).await;

    // --- Tenant B: urgent batch but NO setting → no notification.
    let b = make_tenant(&db, "bravo").await;
    let pb = make_product(&db, &b, "bravo-prod").await;
    make_batch(&db, &b, &pb, "B-URG", now + Duration::days(2), 7, true).await;

    jobs::run_near_expiry_alert(&db).await;

    let rows = notifications(&db).await;
    assert_eq!(rows.len(), 1, "exactly one tenant (A) should be alerted");

    let n = &rows[0];
    assert_eq!(n.kind, "near_expiry");
    assert_eq!(n.tenant, a, "notification must belong to opted-in tenant A");

    // Payload counts only the 2 qualifying urgent batches.
    let count = n.payload.get("count").and_then(|v| v.as_u64()).unwrap();
    assert_eq!(
        count, 2,
        "only the 2 urgent+in-stock+active batches qualify"
    );

    let batches = n.payload.get("batches").and_then(|v| v.as_array()).unwrap();
    assert_eq!(batches.len(), 2);
    let codes: Vec<&str> = batches
        .iter()
        .filter_map(|b| b.get("batch_code").and_then(|v| v.as_str()))
        .collect();
    assert!(codes.contains(&"A-URG-1"));
    assert!(codes.contains(&"A-URG-2"));
    assert!(!codes.contains(&"A-FAR"));
    assert!(!codes.contains(&"A-ZERO"));
    assert!(!codes.contains(&"A-INACT"));
}

#[tokio::test]
async fn near_expiry_no_optin_writes_nothing() {
    let db = setup().await;
    let now = Utc::now();
    let t = make_tenant(&db, "noopt").await;
    let p = make_product(&db, &t, "noopt-prod").await;
    // Urgent batch, but tenant never enabled the setting.
    make_batch(&db, &t, &p, "URG", now + Duration::days(1), 9, true).await;

    jobs::run_near_expiry_alert(&db).await;

    assert!(
        notifications(&db).await.is_empty(),
        "no setting ⇒ no notification rows"
    );
}

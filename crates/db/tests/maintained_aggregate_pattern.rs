//! Aggregate-maintenance audit + regression guards (milton, 2026-06-14).
//!
//! # Background — the "view-update gotcha"
//!
//! Memory `surrealdb-view-update-gotcha` and PR #194 recorded that SurrealDB
//! pre-computed table views (`DEFINE TABLE x AS SELECT ... GROUP BY ...`)
//! **mis-maintained UPDATE/DELETE**: flipping a field that changes group
//! membership (e.g. `active = true -> false`) dropped the row from the grouped
//! `count()` and diverged `math::sum`, while CREATE worked. That motivated
//! `0029_product_stats_view.surql` (#194) to use a `DEFINE EVENT` delta
//! aggregate instead.
//!
//! # Audit result on `origin/feature/erp-parity` @ e4c002f
//!
//! 1. Swept every `migrations/*.surql`: **ZERO** pre-computed views exist. All
//!    aggregates are computed at query time (`catalog::stats` `GROUP ALL`) or
//!    materialised atomically in a Rust transaction (`product.stock =
//!    SUM(stock_movement.delta)`). Nothing to convert.
//! 2. The gotcha was re-tested against the workspace's **current SurrealDB
//!    2.6.5** (both `surrealkv` file and `kv-mem`), with `GROUP ALL` and
//!    `GROUP BY`, `count()` and `math::sum`. It **no longer reproduces** —
//!    pre-computed views now track ground truth across CREATE / UPDATE
//!    (both flip directions + value change) / DELETE. SurrealDB fixed view
//!    maintenance between the version #194 measured and 2.6.5.
//!
//! # What these tests pin
//!
//! * `define_event_aggregate_stays_exact_across_crud` — the pattern #194 ships
//!   (a delta event) is unconditionally correct and O(1). This is the reference
//!   for any future maintained aggregate.
//! * `precomputed_view_tracks_crud_on_current_surrealdb` — a **regression
//!   guard**: asserts views maintain UPDATE/DELETE correctly on the pinned
//!   SurrealDB. If a future bump reintroduces the old drift this test fails,
//!   flagging that new aggregates must use the event pattern (and that #194's
//!   choice was load-bearing again).

use pharma_core::config::DbConfig;
use serde::Deserialize;

async fn temp_db() -> (db::Db, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = DbConfig {
        path: dir.path().to_string_lossy().into_owned(),
        namespace: "pharma_test".into(),
        database: "main".into(),
    };
    let handle = db::connect(&cfg).await.expect("db connect");
    (handle, dir)
}

async fn exec(db: &db::Db, sql: &str) {
    db.query(sql)
        .await
        .unwrap_or_else(|e| panic!("query failed: {sql}\n{e}"))
        .check()
        .unwrap_or_else(|e| panic!("result check failed: {sql}\n{e}"));
}

// --- 1. sanctioned pattern: DEFINE EVENT delta aggregate --------------------

#[derive(Debug, Deserialize)]
struct Agg {
    total_active: i64,
    sum_qty: i64,
}

async fn read_agg(db: &db::Db) -> (i64, i64) {
    let mut r = db
        .query("SELECT total_active, sum_qty FROM agg_total:current")
        .await
        .expect("read agg")
        .check()
        .expect("agg ok");
    let row: Option<Agg> = r.take(0).expect("agg row");
    let a = row.expect("agg row present");
    (a.total_active, a.sum_qty)
}

/// Mirror of `0029_product_stats_view.surql`: a singleton aggregate row
/// maintained by an `$after - $before` delta event. Must track ground truth
/// through the exact UPDATE that historically broke pre-computed views.
#[tokio::test]
async fn define_event_aggregate_stays_exact_across_crud() {
    let (db, _dir) = temp_db().await;

    exec(
        &db,
        "DEFINE TABLE agg_item SCHEMAFULL; \
         DEFINE FIELD active ON agg_item TYPE bool DEFAULT true; \
         DEFINE FIELD qty    ON agg_item TYPE int  DEFAULT 0; \
         DEFINE TABLE agg_total SCHEMAFULL; \
         DEFINE FIELD total_active ON agg_total TYPE int DEFAULT 0; \
         DEFINE FIELD sum_qty      ON agg_total TYPE int DEFAULT 0; \
         CREATE agg_total:current SET total_active = 0, sum_qty = 0; \
         DEFINE EVENT agg_maintain ON TABLE agg_item \
           WHEN $event IN ['CREATE', 'UPDATE', 'DELETE'] THEN { \
             LET $b_active = IF $before != NONE AND $before.active THEN 1 ELSE 0 END; \
             LET $a_active = IF $after  != NONE AND $after.active  THEN 1 ELSE 0 END; \
             LET $b_qty = IF $before != NONE AND $before.active THEN $before.qty ELSE 0 END; \
             LET $a_qty = IF $after  != NONE AND $after.active  THEN $after.qty  ELSE 0 END; \
             UPDATE agg_total:current SET \
               total_active += ($a_active - $b_active), \
               sum_qty      += ($a_qty - $b_qty); \
           };",
    )
    .await;

    assert_eq!(read_agg(&db).await, (0, 0), "seed");

    exec(&db, "CREATE agg_item:a SET active = true, qty = 10;").await;
    assert_eq!(read_agg(&db).await, (1, 10), "create active a");

    exec(&db, "CREATE agg_item:b SET active = true, qty = 5;").await;
    assert_eq!(read_agg(&db).await, (2, 15), "create active b");

    exec(&db, "CREATE agg_item:c SET active = false, qty = 99;").await;
    assert_eq!(read_agg(&db).await, (2, 15), "inactive c excluded");

    // Group-membership flip false -> true (the historical view-breaker).
    exec(&db, "UPDATE agg_item:c SET active = true;").await;
    assert_eq!(read_agg(&db).await, (3, 114), "flip c -> active");

    exec(&db, "UPDATE agg_item:a SET active = false;").await;
    assert_eq!(read_agg(&db).await, (2, 104), "flip a -> inactive");

    exec(&db, "UPDATE agg_item:b SET qty = 8;").await;
    assert_eq!(read_agg(&db).await, (2, 107), "qty change of active b");

    exec(&db, "DELETE agg_item:c;").await;
    assert_eq!(read_agg(&db).await, (1, 8), "delete active c");
}

// --- 2. regression guard: pre-computed view on the pinned SurrealDB ----------

#[derive(Debug, Deserialize)]
struct Stat {
    total: i64,
    sum_qty: Option<i64>,
}

/// Single-group view row, or `None` once the group has no active members.
async fn read_view(db: &db::Db) -> Option<(i64, i64)> {
    let mut r = db
        .query("SELECT total, sum_qty FROM v_stats")
        .await
        .expect("read view")
        .check()
        .expect("view ok");
    let row: Option<Stat> = r.take(0).expect("view row");
    row.map(|s| (s.total, s.sum_qty.unwrap_or(0)))
}

/// On SurrealDB 2.6.5 a `GROUP BY` pre-computed view with `count()` + `sum`
/// tracks every CRUD op correctly. Verified against `surrealkv` here and
/// `kv-mem` during the audit. If this regresses, the old gotcha is back —
/// switch new aggregates to the DEFINE EVENT pattern above.
#[tokio::test]
async fn precomputed_view_tracks_crud_on_current_surrealdb() {
    let (db, _dir) = temp_db().await;

    exec(
        &db,
        "DEFINE TABLE view_item SCHEMAFULL; \
         DEFINE FIELD grp    ON view_item TYPE string; \
         DEFINE FIELD active ON view_item TYPE bool DEFAULT true; \
         DEFINE FIELD qty    ON view_item TYPE int  DEFAULT 0; \
         DEFINE TABLE v_stats AS \
           SELECT grp, count() AS total, math::sum(qty) AS sum_qty \
           FROM view_item WHERE active = true GROUP BY grp;",
    )
    .await;

    exec(
        &db,
        "CREATE view_item:a SET grp = 'g', active = true, qty = 10;",
    )
    .await;
    exec(
        &db,
        "CREATE view_item:b SET grp = 'g', active = true, qty = 5;",
    )
    .await;
    assert_eq!(read_view(&db).await, Some((2, 15)), "view on create");

    // UPDATE flips group membership — the case that used to drift.
    exec(&db, "UPDATE view_item:a SET active = false;").await;
    assert_eq!(read_view(&db).await, Some((1, 5)), "view after flip out");

    exec(&db, "UPDATE view_item:a SET active = true;").await;
    assert_eq!(read_view(&db).await, Some((2, 15)), "view after flip back");

    exec(&db, "UPDATE view_item:b SET qty = 99;").await;
    assert_eq!(
        read_view(&db).await,
        Some((2, 109)),
        "view after qty change"
    );

    exec(&db, "DELETE view_item:a;").await;
    assert_eq!(read_view(&db).await, Some((1, 99)), "view after delete");

    // Removing the last active member empties the group.
    exec(&db, "UPDATE view_item:b SET active = false;").await;
    assert_eq!(
        read_view(&db).await,
        None,
        "view empty when group has no active"
    );
}

//! Regression guard for BUG-perf-007 (order_item report path).
//!
//! The sales/expense report aggregates (`top_products`, `margins_daily`,
//! `stock_rotation`, customer purchase counts) resolve line items with
//! `SELECT ... FROM order_item WHERE tenant = $t AND order IN $ids`, where
//! `$ids` is the set of orders inside the report's date window.
//!
//! BUG-perf-007 asked whether that `order IN $ids` is a full table scan (the
//! record-id `id IN $ids` antipattern of BUG-perf-002). It is NOT: `order` is a
//! secondary field covered by the compound index `order_item_tenant_order`
//! (migrations/0007_sales.surql), and SurrealDB's planner expands `order IN
//! $ids` into a UNION of index lookups (`operation: 'Iterate Index'`), so the
//! cost is O(matched line items), not O(order_item table). Measured flat across
//! 10k/40k rows, well under the <50ms budget.
//!
//! This test PINS that property: if a future migration drops the index, or a
//! refactor rewrites the query into a table scan, the EXPLAIN plan flips to
//! `Iterate Table` and this guard fails — protecting the reporting numbers the
//! owner trusts from a silent O(catalog) regression.

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
        .query("CREATE tenant SET name = 'T', slug = 't' RETURN id")
        .await
        .unwrap();
    let tenant: Option<Thing> = r.take((0, "id")).unwrap();
    (db, tenant.expect("tenant id"))
}

/// Seed a handful of orders + 1 order_item each. EXPLAIN reports the same plan
/// regardless of row count, so a tiny dataset is enough to pin the plan.
async fn seed(db: &Db, tenant: &Thing, n: usize) {
    let tok = tenant.to_string();
    let mut rows = String::from("[");
    for j in 0..n {
        if j > 0 {
            rows.push(',');
        }
        rows.push_str(&format!(
            "{{tenant:{tok},order:order:o{j:05},product:product:p{j:05},\
             product_name:'P{j}',quantity:1,unit_price:1000,subtotal:1000,\
             created_at:time::now()}}"
        ));
    }
    rows.push(']');
    db.query(format!("INSERT INTO order_item {rows}"))
        .await
        .expect("insert order_item")
        .check()
        .expect("insert ok");
}

async fn explain(db: &Db, tenant: &Thing, sql: &str, ids: &[Thing], one: &Thing) -> String {
    let mut r = db
        .query(sql)
        .bind(("t", tenant.clone()))
        .bind(("ids", ids.to_vec()))
        .bind(("o", one.clone()))
        .await
        .expect("explain query");
    let plan: surrealdb::Value = r.take(0usize).expect("explain plan");
    format!("{plan:#}")
}

#[tokio::test]
async fn order_item_report_query_uses_index_not_scan() {
    let (db, tenant) = setup().await;
    seed(&db, &tenant, 30).await;

    let ids: Vec<Thing> = (0..10)
        .map(|j| Thing::from(("order", format!("o{j:05}").as_str())))
        .collect();
    let one = ids[0].clone();

    // The canonical report line-item resolve (expenses::service top_products /
    // margins_daily / stock_rotation all share this shape).
    let report = explain(
        &db,
        &tenant,
        "SELECT product, quantity, subtotal FROM order_item \
         WHERE tenant = $t AND order IN $ids EXPLAIN",
        &ids,
        &one,
    )
    .await;
    assert!(
        report.contains("Iterate Index"),
        "order IN $ids report query must use the order_item_tenant_order index \
         (BUG-perf-007 guard); plan was:\n{report}"
    );
    assert!(
        !report.contains("Iterate Table"),
        "order IN $ids report query regressed to a table scan \
         (BUG-perf-007 / BUG-perf-002 class); plan was:\n{report}"
    );

    // The customer purchase-count aggregate (customers::repo) shares the index.
    let counts = explain(
        &db,
        &tenant,
        "SELECT order, count() AS c FROM order_item \
         WHERE tenant = $t AND order IN $ids GROUP BY order EXPLAIN",
        &ids,
        &one,
    )
    .await;
    assert!(
        counts.contains("Iterate Index"),
        "customer purchase-count query must use the index (BUG-perf-007 guard); \
         plan was:\n{counts}"
    );
    assert!(
        !counts.contains("Iterate Table"),
        "customer purchase-count query regressed to a table scan; plan was:\n{counts}"
    );
}

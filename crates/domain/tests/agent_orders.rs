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

async fn seed_catalogued_order(
    db: &Db,
    tenant: &Thing,
    barcode: &str,
    stock: i64,
    qty: i64,
) -> (String, Thing) {
    #[derive(serde::Deserialize)]
    struct Row {
        id: Thing,
    }
    let mut p = db
        .query(
            "CREATE product SET tenant=$t, name='X', slug='x', price=1000, \
             stock=$s, active=true RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("s", stock))
        .await
        .unwrap();
    let pid = p.take::<Option<Row>>(0).unwrap().unwrap().id;
    db.query("CREATE product_barcode SET tenant=$t, product=$p, barcode=$b")
        .bind(("t", tenant.clone()))
        .bind(("p", pid.clone()))
        .bind(("b", barcode.to_string()))
        .await
        .unwrap();
    let lines_json =
        format!(r#"[{{"barcode":"{barcode}","qty":{qty},"unit_price_canonical":1000}}]"#);
    let mut o = db
        .query(
            "CREATE agent_order SET tenant=$t, peer_did='did:pharma:b', \
             lines_json=$l, total=$tot, status='accepted', price_adjusted=false \
             RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("l", lines_json))
        .bind(("tot", (qty * 1000) as f64))
        .await
        .unwrap();
    (
        o.take::<Option<Row>>(0).unwrap().unwrap().id.to_string(),
        pid,
    )
}

async fn product_stock(db: &Db, pid: &Thing) -> i64 {
    #[derive(serde::Deserialize)]
    struct S {
        stock: i64,
    }
    db.query("SELECT stock FROM product WHERE id = $p LIMIT 1")
        .bind(("p", pid.clone()))
        .await
        .unwrap()
        .take::<Option<S>>(0)
        .unwrap()
        .unwrap()
        .stock
}

#[tokio::test]
async fn fulfill_decrements_stock_logs_movement_and_marks_fulfilled() {
    let (db, tenant) = setup().await;
    let (id, pid) = seed_catalogued_order(&db, &tenant, "7800000000001", 50, 7).await;
    assert_eq!(product_stock(&db, &pid).await, 50);

    let r = service::fulfill(&db, &tenant, &id).await.unwrap();
    assert_eq!(r.status, "fulfilled");
    assert_eq!(product_stock(&db, &pid).await, 43);

    // Audit movement recorded with the right reason + ref + negative delta.
    #[derive(serde::Deserialize)]
    struct Mov {
        delta: i64,
        reason: String,
        #[serde(rename = "ref")]
        reff: Option<String>,
    }
    let mut q = db
        .query("SELECT delta, reason, ref FROM stock_movement WHERE tenant = $t")
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let rows: Vec<Mov> = q.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].delta, -7);
    assert_eq!(rows[0].reason, "agent_fulfill");
    assert_eq!(rows[0].reff.as_deref(), Some(id.as_str()));
}

#[tokio::test]
async fn fulfill_from_received_is_conflict() {
    let (db, tenant) = setup().await;
    let id = seed_order(&db, &tenant, "did:pharma:b", 1000.0).await;
    let err = service::fulfill(&db, &tenant, &id).await.unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
}

#[tokio::test]
async fn fulfill_refuses_when_stock_insufficient_and_leaves_state() {
    let (db, tenant) = setup().await;
    let (id, pid) = seed_catalogued_order(&db, &tenant, "7800000000002", 3, 10).await;
    let err = service::fulfill(&db, &tenant, &id).await.unwrap_err();
    assert_eq!(err.code(), "INSUFFICIENT_STOCK");
    // Order stays accepted; stock untouched.
    assert_eq!(
        service::get(&db, &tenant, &id).await.unwrap().status,
        "accepted"
    );
    assert_eq!(product_stock(&db, &pid).await, 3);
}

/// BACKLOG #2 remainder — agent fulfillment of a batch-tracked product
/// persists the per-line FEFO breakdown to `agent_order.fulfillment_batches_json`
/// (migration `0014`) and decrements every consumed lot, not just `product.stock`.
#[tokio::test]
async fn fulfill_persists_multi_lot_fefo_breakdown() {
    let (db, tenant) = setup().await;

    // Catalogued product with two lots: A expires sooner (4 units), B later (10).
    // Order asks for 5 → FEFO must drain A (4) then dip into B (1).
    #[derive(serde::Deserialize)]
    struct Row {
        id: Thing,
    }
    let mut p = db
        .query(
            "CREATE product SET tenant=$t, name='X', slug='x', price=1000, \
             stock=14, active=true RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let pid = p.take::<Option<Row>>(0).unwrap().unwrap().id;
    db.query("CREATE product_barcode SET tenant=$t, product=$p, barcode='BC-MULTI'")
        .bind(("t", tenant.clone()))
        .bind(("p", pid.clone()))
        .await
        .unwrap();
    // Lot A (earlier expiry).
    let mut a = db
        .query(
            "CREATE product_batch SET tenant=$t, product=$p, batch_code='A', \
             expiry_date=time::now() + 30d, stock=4, active=true RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", pid.clone()))
        .await
        .unwrap();
    let lot_a = a.take::<Option<Row>>(0).unwrap().unwrap().id;
    // Lot B (later expiry).
    let mut b = db
        .query(
            "CREATE product_batch SET tenant=$t, product=$p, batch_code='B', \
             expiry_date=time::now() + 120d, stock=10, active=true RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", pid.clone()))
        .await
        .unwrap();
    let lot_b = b.take::<Option<Row>>(0).unwrap().unwrap().id;

    let lines_json = r#"[{"barcode":"BC-MULTI","qty":5,"unit_price_canonical":1000}]"#.to_string();
    let mut o = db
        .query(
            "CREATE agent_order SET tenant=$t, peer_did='did:pharma:b', \
             lines_json=$l, total=5000, status='accepted', price_adjusted=false \
             RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("l", lines_json))
        .await
        .unwrap();
    let oid = o.take::<Option<Row>>(0).unwrap().unwrap().id.to_string();

    let r = service::fulfill(&db, &tenant, &oid).await.unwrap();
    assert_eq!(r.status, "fulfilled");
    assert_eq!(product_stock(&db, &pid).await, 14 - 5);

    // Per-line breakdown persisted in FEFO order, sum = line qty, legacy
    // `product.stock` decrement still happened (asserted above).
    let breakdown = r
        .fulfillment_batches
        .expect("batch-tracked fulfillment persists breakdown");
    assert_eq!(breakdown.len(), 1);
    let line = &breakdown[0];
    assert_eq!(line.product, pid.to_string());
    assert_eq!(line.allocations.len(), 2);
    assert_eq!(line.allocations[0].batch, lot_a.to_string());
    assert_eq!(line.allocations[0].qty, 4);
    assert_eq!(line.allocations[1].batch, lot_b.to_string());
    assert_eq!(line.allocations[1].qty, 1);
    assert_eq!(line.allocations.iter().map(|a| a.qty).sum::<i64>(), 5);

    // Lots drained per FEFO; product.stock invariant holds.
    #[derive(serde::Deserialize)]
    struct S {
        stock: i64,
    }
    let mut q = db
        .query("SELECT stock FROM product_batch WHERE id = $b LIMIT 1")
        .bind(("b", lot_a.clone()))
        .await
        .unwrap();
    assert_eq!(q.take::<Option<S>>(0).unwrap().unwrap().stock, 0);
    let mut q = db
        .query("SELECT stock FROM product_batch WHERE id = $b LIMIT 1")
        .bind(("b", lot_b.clone()))
        .await
        .unwrap();
    assert_eq!(q.take::<Option<S>>(0).unwrap().unwrap().stock, 9);
}

/// Non-batch-tracked products keep the legacy `product.stock`-only path —
/// `fulfillment_batches` stays `None`, no migration backfill needed.
#[tokio::test]
async fn fulfill_non_batch_tracked_leaves_fulfillment_batches_none() {
    let (db, tenant) = setup().await;
    let (id, _pid) = seed_catalogued_order(&db, &tenant, "7800000000099", 50, 3).await;
    let r = service::fulfill(&db, &tenant, &id).await.unwrap();
    assert_eq!(r.status, "fulfilled");
    assert!(r.fulfillment_batches.is_none());
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

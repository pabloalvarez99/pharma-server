//! Cuenta corriente / fiado (V1) — ledger inmutable, saldo, abonos.

use rust_decimal::Decimal;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

use domain::credit::{repo, service};

type Db = Surreal<surrealdb::engine::local::Db>;

async fn setup() -> (Db, Thing) {
    let db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations)
        .await
        .expect("migrations apply");
    let mut r = db
        .query("CREATE tenant SET name = 'Alm', slug = 'alm' RETURN id")
        .await
        .unwrap();
    let id: Option<Thing> = r.take((0, "id")).unwrap();
    (db, id.expect("tenant id"))
}

async fn seed_customer(db: &Db, tenant: &Thing, name: &str) -> Thing {
    let mut r = db
        .query("CREATE customer SET tenant = $t, name = $n, loyalty_points = 0, active = true RETURN id")
        .bind(("t", tenant.clone()))
        .bind(("n", name.to_string()))
        .await
        .unwrap();
    let id: Option<Thing> = r.take((0, "id")).unwrap();
    id.expect("customer id")
}

async fn seed_order(db: &Db, tenant: &Thing, total: f64) -> Thing {
    let mut r = db
        .query(
            "CREATE order SET tenant = $t, status = 'paid', payment_method = 'pos_fiado', \
             subtotal = $tot, discount = 0, total = $tot RETURN id",
        )
        .bind(("t", tenant.clone()))
        .bind(("tot", total))
        .await
        .unwrap();
    let id: Option<Thing> = r.take((0, "id")).unwrap();
    id.expect("order id")
}

fn d(s: &str) -> Decimal {
    s.parse().unwrap()
}

#[tokio::test]
async fn cargo_then_abono_computes_balance() {
    let (db, t) = setup().await;
    let c = seed_customer(&db, &t, "Doña Ana").await;
    let o = seed_order(&db, &t, 5000.0).await;

    repo::post_cargo(&db, &t, &c, &o, d("5000"), None)
        .await
        .unwrap();
    let acct = repo::account(&db, &t, &c).await.unwrap();
    assert_eq!(acct.balance, d("5000"));
    assert_eq!(acct.total_charged, d("5000"));
    assert_eq!(acct.total_paid, d("0"));
    assert_eq!(acct.entries.len(), 1);

    // Abono parcial.
    service::record_abono(&db, &t, &c, d("2000"), None, Some("efectivo"), None)
        .await
        .unwrap();
    let acct = repo::account(&db, &t, &c).await.unwrap();
    assert_eq!(acct.balance, d("3000"));
    assert_eq!(acct.total_paid, d("2000"));
    assert_eq!(acct.entries.len(), 2);
}

#[tokio::test]
async fn cargo_is_idempotent_per_order() {
    let (db, t) = setup().await;
    let c = seed_customer(&db, &t, "Pedro").await;
    let o = seed_order(&db, &t, 3000.0).await;

    repo::post_cargo(&db, &t, &c, &o, d("3000"), None)
        .await
        .unwrap();
    // Reintento del POS con la misma orden: NO duplica.
    repo::post_cargo(&db, &t, &c, &o, d("3000"), None)
        .await
        .unwrap();
    let acct = repo::account(&db, &t, &c).await.unwrap();
    assert_eq!(acct.entries.len(), 1);
    assert_eq!(acct.balance, d("3000"));
}

#[tokio::test]
async fn abono_cannot_exceed_debt() {
    let (db, t) = setup().await;
    let c = seed_customer(&db, &t, "Luis").await;
    let o = seed_order(&db, &t, 1000.0).await;
    repo::post_cargo(&db, &t, &c, &o, d("1000"), None)
        .await
        .unwrap();

    let err = service::record_abono(&db, &t, &c, d("1500"), None, None, None)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("supera la deuda"), "got: {err}");
}

#[tokio::test]
async fn abono_without_debt_is_rejected() {
    let (db, t) = setup().await;
    let c = seed_customer(&db, &t, "Sin deuda").await;
    let err = service::record_abono(&db, &t, &c, d("500"), None, None, None)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("no tiene deuda"), "got: {err}");
}

#[tokio::test]
async fn balances_are_isolated_per_customer() {
    let (db, t) = setup().await;
    let a = seed_customer(&db, &t, "A").await;
    let b = seed_customer(&db, &t, "B").await;
    let oa = seed_order(&db, &t, 4000.0).await;
    repo::post_cargo(&db, &t, &a, &oa, d("4000"), None)
        .await
        .unwrap();

    assert_eq!(repo::balance(&db, &t, &a).await.unwrap(), d("4000"));
    assert_eq!(repo::balance(&db, &t, &b).await.unwrap(), d("0"));
}

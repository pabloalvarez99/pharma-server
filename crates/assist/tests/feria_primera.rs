//! Primera venta feria: «vendí tomates a 2000» sin SKU previo.

use std::str::FromStr;

use assist::{build, execute, parse_action, Action, ActionStore, BuildOutcome, Money};
use domain::cash_register::model::OpenSessionInput;
use domain::cash_register::service as caja;
use domain::catalog::model::ProductFilters;
use domain::catalog::service as catalog;
use domain::provisioning::SETTING_VERTICAL;
use domain::sales::model::OrderFilters;
use domain::sales::service as sales;
use rust_decimal::Decimal;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

type Db = Surreal<surrealdb::engine::local::Db>;

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

async fn setup() -> (Db, Thing, Thing) {
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
    let user: Thing = db
        .query("CREATE user SET tenant=$t, email='a@t.l', password='x', roles=['admin'] RETURN id")
        .bind(("t", tenant.clone()))
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    (db, tenant, user)
}

async fn set_feria(db: &Db, tenant: &Thing) {
    sales::set_setting(db, tenant, SETTING_VERTICAL, "feria")
        .await
        .unwrap();
}

#[tokio::test]
async fn feria_sin_producto_con_precio_crea_y_propone() {
    let (db, tenant, _user) = setup().await;
    set_feria(&db, &tenant).await;

    let action = match build(&db, &tenant, parse_action("vendeme 1 tomates a 2000"))
        .await
        .unwrap()
    {
        BuildOutcome::Ready(a) => a,
        BuildOutcome::Reject(m) => panic!("feria con precio debe crear la cosa: {m}"),
        BuildOutcome::NotAnAction => panic!("expected Ready"),
    };
    match action {
        Action::Vender { lines, total, .. } => {
            assert_eq!(lines.len(), 1);
            assert!(
                lines[0].product_name.eq_ignore_ascii_case("tomates"),
                "{}",
                lines[0].product_name
            );
            assert_eq!(lines[0].unit_price, dec("2000"));
            assert_eq!(total, dec("2000"));
        }
        other => panic!("expected Vender, got {other:?}"),
    }

    let found = catalog::list_products(
        &db,
        &tenant,
        ProductFilters {
            search: Some("tomates".into()),
            active: Some(true),
            limit: Some(5),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let p = found
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case("tomates"))
        .expect("tomates created");
    assert!(!p.physical_stock);
    assert_eq!(p.stock, 0);
    assert_eq!(p.price, dec("2000"));
}

#[tokio::test]
async fn feria_confirm_sin_sku_cobra() {
    let (db, tenant, user) = setup().await;
    let store = ActionStore::new();
    set_feria(&db, &tenant).await;

    let action = match build(&db, &tenant, parse_action("vendeme 1 tomates a 2000"))
        .await
        .unwrap()
    {
        BuildOutcome::Ready(a) => a,
        BuildOutcome::Reject(m) => panic!("expected Ready, got reject: {m}"),
        BuildOutcome::NotAnAction => panic!("expected Ready"),
    };
    let proposal = store.propose(action, &tenant, &Money::default());
    let action = store.consume(&proposal.confirm_token, &tenant).unwrap();
    let outcome = execute(&db, &tenant, Some(&user), action).await.unwrap();
    assert_eq!(outcome.action, "vender");
    let orders = sales::list_orders(&db, &tenant, OrderFilters::default())
        .await
        .unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].total, dec("2000"));
}

#[tokio::test]
async fn feria_sin_precio_pide_el_precio() {
    let (db, tenant, _user) = setup().await;
    set_feria(&db, &tenant).await;
    let msg = match build(&db, &tenant, parse_action("vendeme 1 tomates"))
        .await
        .unwrap()
    {
        BuildOutcome::Reject(m) => m,
        BuildOutcome::Ready(_) => panic!("expected Reject, got Ready"),
        BuildOutcome::NotAnAction => panic!("expected Reject"),
    };
    assert!(msg.contains("precio"), "{msg}");
    assert!(msg.contains("tomates"), "{msg}");
}

#[tokio::test]
async fn farmacia_sin_producto_sigue_pidiendo_crearlo() {
    let (db, tenant, user) = setup().await;
    caja::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "caja-1".into(),
            register: None,
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let msg = match build(&db, &tenant, parse_action("vendeme 1 tomates a 2000"))
        .await
        .unwrap()
    {
        BuildOutcome::Reject(m) => m,
        BuildOutcome::Ready(_) => panic!("farmacia no crea SKU al vuelo"),
        BuildOutcome::NotAnAction => panic!("farmacia no crea SKU al vuelo"),
    };
    assert!(msg.contains("créalo") || msg.contains("crealo"), "{msg}");
}

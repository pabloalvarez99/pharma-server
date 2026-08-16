//! Apertura implícita del puesto en feria: cobrar día 1 sin ritual de caja.

use std::str::FromStr;

use assist::{build, execute, parse_action, Action, ActionStore, BuildOutcome, Money};
use domain::cash_register::model::SessionFilters;
use domain::cash_register::service as caja;
use domain::catalog::model::NewProduct;
use domain::catalog::service as catalog;
use domain::customers::model::NewCustomer;
use domain::customers::service as customers;
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

async fn seed_sellable(db: &Db, tenant: &Thing, name: &str, price: &str, stock: i64) {
    catalog::create_product(
        db,
        tenant,
        NewProduct {
            name: name.into(),
            slug: None,
            description: None,
            price: dec(price),
            cost_price: None,
            stock,
            category: None,
            image_url: None,
            external_id: None,
            laboratory: None,
            therapeutic_action: None,
            active_ingredient: None,
            prescription_type: None,
            presentation: None,
            physical_stock: None,
            discount_percent: None,
            attrs: None,
        },
    )
    .await
    .unwrap();
}

async fn set_feria(db: &Db, tenant: &Thing) {
    sales::set_setting(db, tenant, SETTING_VERTICAL, "feria")
        .await
        .unwrap();
}

async fn open_sessions(db: &Db, tenant: &Thing) -> Vec<domain::cash_register::model::CashSessionDto> {
    caja::list_sessions(
        db,
        tenant,
        SessionFilters {
            status: Some("open".into()),
            user: None,
            limit: Some(10),
            offset: None,
        },
    )
    .await
    .unwrap()
}

/// (a) Feria sin caja: «vendeme 1 tomates» → Ready(Vender), no Reject de caja.
#[tokio::test]
async fn feria_sin_caja_propone_vender_sin_reject() {
    let (db, tenant, _user) = setup().await;
    set_feria(&db, &tenant).await;
    seed_sellable(&db, &tenant, "Tomates", "2000", 10).await;

    let action = match build(&db, &tenant, parse_action("vendeme 1 tomates"))
        .await
        .unwrap()
    {
        BuildOutcome::Ready(a) => a,
        BuildOutcome::Reject(m) => panic!("feria no debe pedir abrir caja: {m}"),
        BuildOutcome::NotAnAction => panic!("expected Ready"),
    };
    match action {
        Action::Vender {
            lines,
            total,
            abre_puesto,
            ..
        } => {
            assert_eq!(lines.len(), 1);
            assert_eq!(lines[0].product_name, "Tomates");
            assert_eq!(total, dec("2000"));
            assert!(abre_puesto, "debe avisar que abre el puesto");
        }
        other => panic!("expected Vender, got {other:?}"),
    }
}

/// (b) Confirm con actor → 1 order + 1 sesión open con opening_cash == 0.
#[tokio::test]
async fn feria_confirm_abre_puesto_con_cero_y_cobra() {
    let (db, tenant, user) = setup().await;
    let store = ActionStore::new();
    set_feria(&db, &tenant).await;
    seed_sellable(&db, &tenant, "Tomates", "2000", 10).await;

    let action = match build(&db, &tenant, parse_action("vendeme 1 tomates"))
        .await
        .unwrap()
    {
        BuildOutcome::Ready(a) => a,
        BuildOutcome::Reject(m) => panic!("expected Ready, got reject: {m}"),
        BuildOutcome::NotAnAction => panic!("expected Ready, got NotAnAction"),
    };
    let proposal = store.propose(action, &tenant, &Money::default());
    assert!(
        proposal.summary.contains("puesto") && proposal.summary.contains("$0"),
        "prosa de confirmación: {}",
        proposal.summary
    );

    assert!(open_sessions(&db, &tenant).await.is_empty(), "propose no abre");

    let action = store.consume(&proposal.confirm_token, &tenant).unwrap();
    let outcome = execute(&db, &tenant, Some(&user), action).await.unwrap();
    assert_eq!(outcome.action, "vender");

    let orders = sales::list_orders(&db, &tenant, OrderFilters::default())
        .await
        .unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].total, dec("2000"));

    let sessions = open_sessions(&db, &tenant).await;
    assert!(
        !sessions.is_empty(),
        "debe quedar al menos 1 sesión open"
    );
    assert_eq!(sessions[0].opening_cash, dec("0"));
    assert_eq!(sessions[0].register_name, "puesto");
}

/// (c) Segunda venta sin cerrar: no Conflict fatal, 2 orders, 1 sesión open.
#[tokio::test]
async fn feria_segunda_venta_reusa_la_misma_sesion() {
    let (db, tenant, user) = setup().await;
    let store = ActionStore::new();
    set_feria(&db, &tenant).await;
    seed_sellable(&db, &tenant, "Tomates", "2000", 10).await;

    for q in ["vendeme 1 tomates", "vendeme 1 tomates"] {
        let action = match build(&db, &tenant, parse_action(q)).await.unwrap() {
            BuildOutcome::Ready(a) => a,
            BuildOutcome::Reject(m) => panic!("expected Ready for {q:?}, got reject: {m}"),
            BuildOutcome::NotAnAction => panic!("expected Ready for {q:?}"),
        };
        let proposal = store.propose(action, &tenant, &Money::default());
        let action = store.consume(&proposal.confirm_token, &tenant).unwrap();
        execute(&db, &tenant, Some(&user), action)
            .await
            .expect("segunda venta no debe fallar por Conflict de caja");
    }

    let orders = sales::list_orders(&db, &tenant, OrderFilters::default())
        .await
        .unwrap();
    assert_eq!(orders.len(), 2);

    let sessions = open_sessions(&db, &tenant).await;
    assert_eq!(sessions.len(), 1, "sigue una sola sesión open");
}

/// (d) Sin vertical (farmacia/otro): reject de caja intacto.
#[tokio::test]
async fn no_feria_sin_caja_sigue_pidiendo_abrirla() {
    let (db, tenant, _user) = setup().await;
    seed_sellable(&db, &tenant, "Alcohol Gel", "2500", 4).await;

    match build(&db, &tenant, parse_action("vendeme 1 alcohol gel"))
        .await
        .unwrap()
    {
        BuildOutcome::Reject(msg) => {
            assert!(msg.contains("caja abierta"), "{msg}");
            assert!(msg.contains("abre la caja"), "{msg}");
        }
        BuildOutcome::Ready(_) => panic!("farmacia sin caja debe Reject, got Ready"),
        BuildOutcome::NotAnAction => panic!("farmacia sin caja debe Reject, got NotAnAction"),
    }
}

/// (e) Feria + fiado: no abre caja.
#[tokio::test]
async fn feria_fiado_no_abre_caja() {
    let (db, tenant, user) = setup().await;
    let store = ActionStore::new();
    set_feria(&db, &tenant).await;
    seed_sellable(&db, &tenant, "Tomates", "2000", 10).await;
    customers::create_customer(
        &db,
        &tenant,
        NewCustomer {
            name: "Rosa".into(),
            rut: None,
            phone: None,
            email: None,
        },
    )
    .await
    .unwrap();

    let action = match build(
        &db,
        &tenant,
        parse_action("anota 1 tomates fiado a Rosa"),
    )
    .await
    .unwrap()
    {
        BuildOutcome::Ready(a) => a,
        BuildOutcome::Reject(m) => panic!("expected Ready fiado, got reject: {m}"),
        BuildOutcome::NotAnAction => panic!("expected Ready fiado"),
    };
    assert!(matches!(action, Action::FiarVenta { .. }));

    let proposal = store.propose(action, &tenant, &Money::default());
    let action = store.consume(&proposal.confirm_token, &tenant).unwrap();
    execute(&db, &tenant, Some(&user), action).await.unwrap();

    assert!(
        open_sessions(&db, &tenant).await.is_empty(),
        "fiado no debe abrir el puesto"
    );
    let orders = sales::list_orders(&db, &tenant, OrderFilters::default())
        .await
        .unwrap();
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].payment_method, "pos_fiado");
}

//! Resumen del día en feria: ventas + puesto (sin reposición / FEFO / controlados).

use std::str::FromStr;

use assist::{parse, AssistProvider, AssistQuery, Deterministic};
use domain::cash_register::{model as cmodel, service as cash};
use domain::provisioning::SETTING_VERTICAL;
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

async fn ask(db: &Db, tenant: &Thing, question: &str) -> assist::Answer {
    let intent = parse(question);
    let q = AssistQuery {
        question,
        intent,
        db,
        tenant,
    };
    Deterministic.answer(&q).await.unwrap()
}

#[tokio::test]
async fn feria_sin_sesion_habla_de_puesto() {
    let (db, tenant, _user) = setup().await;
    set_feria(&db, &tenant).await;

    let a = ask(&db, &tenant, "resumen del día").await;
    assert_eq!(a.intent, "resumen_dia");
    let t = a.text.to_lowercase();
    assert!(t.contains("puesto"), "text: {}", a.text);
    assert!(
        !t.contains("caja abierta"),
        "no debe sonar a farmacia: {}",
        a.text
    );
    assert!(!t.contains("controlados"), "text: {}", a.text);
    assert!(!t.contains("por vencer"), "text: {}", a.text);
    assert!(!t.contains("reposición") && !t.contains("reposicion"), "text: {}", a.text);
}

#[tokio::test]
async fn feria_con_sesion_habla_de_puesto() {
    let (db, tenant, user) = setup().await;
    set_feria(&db, &tenant).await;
    cash::open_session(
        &db,
        &tenant,
        &user,
        cmodel::OpenSessionInput {
            register_name: "puesto".into(),
            register: None,
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();

    let a = ask(&db, &tenant, "resumen del día").await;
    assert_eq!(a.intent, "resumen_dia");
    assert!(
        a.text.contains("Puesto:") || a.text.to_lowercase().contains("puesto"),
        "text: {}",
        a.text
    );
    assert!(
        !a.text.contains("Caja «"),
        "no debe citar Caja «…»: {}",
        a.text
    );
    let t = a.text.to_lowercase();
    assert!(!t.contains("controlados"), "text: {}", a.text);
    assert!(!t.contains("por vencer"), "text: {}", a.text);
}

#[tokio::test]
async fn sin_vertical_sigue_caja_de_farmacia() {
    let (db, tenant, _user) = setup().await;

    let a = ask(&db, &tenant, "resumen del día").await;
    assert_eq!(a.intent, "resumen_dia");
    assert!(
        a.text.contains("No hay caja abierta."),
        "pharmacy empty must stay byte-stable: {}",
        a.text
    );
    assert!(
        a.text.contains("Reposición:") || a.text.contains("Por vencer"),
        "pharmacy brief keeps stock bullets: {}",
        a.text
    );
}

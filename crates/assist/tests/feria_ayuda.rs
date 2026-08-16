//! Ayuda / unknown del agente en vertical feria (cuaderno, no farmacia).

use assist::{parse, AssistProvider, AssistQuery, Deterministic};
use domain::provisioning::SETTING_VERTICAL;
use domain::sales::service as sales;
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
async fn feria_ayuda_habla_de_vender_y_fiar() {
    let (db, tenant) = setup().await;
    set_feria(&db, &tenant).await;

    let help = ask(&db, &tenant, "ayuda").await;
    assert_eq!(help.intent, "ayuda");
    let t = help.text.to_lowercase();
    assert!(t.contains("tomates"), "text: {}", help.text);
    assert!(t.contains("vendí") || t.contains("vendi"), "text: {}", help.text);
    assert!(
        t.contains("fío") || t.contains("fio") || t.contains("deben"),
        "text: {}",
        help.text
    );
    assert!(!t.contains("ibuprofeno"), "text: {}", help.text);
    assert!(!t.contains("paracetamol"), "text: {}", help.text);
    assert!(!t.contains("receta"), "text: {}", help.text);
    assert!(!t.contains("controlado"), "text: {}", help.text);
    assert!(!t.contains("abre la caja"), "text: {}", help.text);
    assert!(!t.contains("50.000"), "text: {}", help.text);
}

#[tokio::test]
async fn feria_unknown_sugiere_venta_de_feria() {
    let (db, tenant) = setup().await;
    set_feria(&db, &tenant).await;

    let huh = ask(&db, &tenant, "cuéntame un chiste").await;
    assert_eq!(huh.intent, "desconocido");
    let t = huh.text.to_lowercase();
    assert!(t.contains("no entendí") || t.contains("no entendi"), "text: {}", huh.text);
    assert!(t.contains("tomates") || t.contains("vendí") || t.contains("vendi"), "text: {}", huh.text);
    assert!(!t.contains("ibuprofeno"), "text: {}", huh.text);
    assert!(!t.contains("stock de"), "text: {}", huh.text);
}

#[tokio::test]
async fn sin_vertical_ayuda_sigue_siendo_farmacia() {
    let (db, tenant) = setup().await;

    let help = ask(&db, &tenant, "ayuda").await;
    assert_eq!(help.intent, "ayuda");
    let t = help.text.to_lowercase();
    assert!(
        t.contains("ibuprofeno") || t.contains("abre la caja"),
        "text: {}",
        help.text
    );
    assert!(!t.contains("tomates"), "text: {}", help.text);

    let huh = ask(&db, &tenant, "cuéntame un chiste").await;
    assert_eq!(huh.intent, "desconocido");
    assert!(huh.text.contains("No entendí la pregunta"), "text: {}", huh.text);
}

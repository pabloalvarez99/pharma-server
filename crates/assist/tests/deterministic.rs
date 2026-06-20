//! Integration tests for the deterministic assist provider over a seeded
//! kv-mem SurrealDB (same harness style as `crates/domain/tests`). Verifies the
//! executors return answers grounded in real tenant data and that tenant
//! isolation holds.

use std::str::FromStr;

use assist::{parse, AssistProvider, AssistQuery, Deterministic, Intent};
use chrono::{Duration, Utc};
use domain::cash_register::{model as cmodel, service as cash};
use domain::catalog::{model::*, service as catalog};
use domain::inventory::{model as imodel, service as inventory};
use domain::sales::{model as smodel, service as sales};
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

fn new_product(name: &str, price: &str, cost: &str, stock: i64) -> NewProduct {
    NewProduct {
        name: name.into(),
        slug: None,
        description: None,
        price: dec(price),
        cost_price: Some(dec(cost)),
        stock,
        category: None,
        image_url: None,
        external_id: None,
        laboratory: None,
        therapeutic_action: None,
        active_ingredient: None,
        prescription_type: None,
        presentation: None,
        discount_percent: None,
    }
}

fn cash_sale(product_id: &str, name: &str, price: &str, qty: i64) -> smodel::PosSaleRequest {
    smodel::PosSaleRequest {
        items: vec![smodel::PosSaleItem {
            product: product_id.into(),
            product_name: name.into(),
            quantity: qty,
            unit_price: dec(price),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec(price) * Decimal::from(qty)),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
    }
}

/// Seed two products + two cash sales today. Returns the product ids.
async fn seed_sales(db: &Db, tenant: &Thing, user: &Thing) -> (String, String) {
    let para = catalog::create_product(
        db,
        tenant,
        new_product("Paracetamol 500", "1500", "600", 50),
    )
    .await
    .unwrap();
    let ibu = catalog::create_product(db, tenant, new_product("Ibuprofeno 400", "2000", "900", 30))
        .await
        .unwrap();
    // 2x paracetamol, 1x ibuprofeno -> 3 units, revenue 1500*2 + 2000 = 5000
    sales::post_sale(
        db,
        tenant,
        Some(user),
        Some("admin"),
        None,
        cash_sale(&para.id, &para.name, "1500", 2),
    )
    .await
    .unwrap();
    sales::post_sale(
        db,
        tenant,
        Some(user),
        Some("admin"),
        None,
        cash_sale(&ibu.id, &ibu.name, "2000", 1),
    )
    .await
    .unwrap();
    (para.id, ibu.id)
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
async fn ventas_hoy_reports_seeded_totals() {
    let (db, tenant, user) = setup().await;
    seed_sales(&db, &tenant, &user).await;

    let a = ask(&db, &tenant, "¿cuánto vendí hoy?").await;
    assert_eq!(a.intent, "ventas_hoy");
    let data = a.data.unwrap();
    assert_eq!(data["orders"], 2);
    assert_eq!(data["revenue"], "5000");
    assert_eq!(data["cash"], "5000");
    assert!(a.text.contains("5.000"), "text was: {}", a.text);
}

#[tokio::test]
async fn ventas_hoy_empty_is_graceful() {
    let (db, tenant, _user) = setup().await;
    let a = ask(&db, &tenant, "ventas hoy").await;
    assert_eq!(a.intent, "ventas_hoy");
    assert_eq!(a.data.unwrap()["orders"], 0);
    assert!(a.text.to_lowercase().contains("no registras"));
}

#[tokio::test]
async fn stock_producto_finds_match() {
    let (db, tenant, user) = setup().await;
    seed_sales(&db, &tenant, &user).await;

    let a = ask(&db, &tenant, "stock de paracetamol").await;
    assert_eq!(a.intent, "stock_producto");
    // 50 initial - 2 sold = 48
    let matches = a.data.unwrap();
    assert_eq!(matches["matches"][0]["name"], "Paracetamol 500");
    assert_eq!(matches["matches"][0]["stock"], 48);
    assert!(a.text.contains("48"), "text: {}", a.text);
}

#[tokio::test]
async fn stock_producto_no_match_is_graceful() {
    let (db, tenant, _user) = setup().await;
    let a = ask(&db, &tenant, "stock de aspirina").await;
    assert_eq!(a.intent, "stock_producto");
    assert!(a.text.to_lowercase().contains("no encontré"));
}

#[tokio::test]
async fn caja_actual_reports_expected_drawer() {
    let (db, tenant, user) = setup().await;
    cash::open_session(
        &db,
        &tenant,
        &user,
        cmodel::OpenSessionInput {
            register_name: "caja-1".into(),
            opening_cash: dec("10000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    seed_sales(&db, &tenant, &user).await;

    let a = ask(&db, &tenant, "cuánto hay en caja").await;
    assert_eq!(a.intent, "caja_actual");
    let data = a.data.unwrap();
    assert_eq!(data["open"], true);
    // opening 10000 + cash sales 5000 = 15000 expected
    assert_eq!(data["expected"], "15000");
    assert!(a.text.contains("15.000"), "text: {}", a.text);
}

#[tokio::test]
async fn caja_actual_no_session_is_graceful() {
    let (db, tenant, _user) = setup().await;
    let a = ask(&db, &tenant, "efectivo en caja").await;
    assert_eq!(a.intent, "caja_actual");
    assert_eq!(a.data.unwrap()["open"], false);
    assert!(a.text.to_lowercase().contains("no hay"));
}

#[tokio::test]
async fn por_vencer_lists_near_expiry_batches() {
    let (db, tenant, _user) = setup().await;
    let para = catalog::create_product(&db, &tenant, new_product("Amoxicilina", "3000", "1200", 0))
        .await
        .unwrap();
    inventory::create_batch(
        &db,
        &tenant,
        imodel::NewBatch {
            product: para.id.clone(),
            batch_code: "L1".into(),
            expiry_date: Utc::now() + Duration::days(10),
            stock: 20,
            cost: Some(dec("1200")),
            notes: None,
        },
        None,
    )
    .await
    .unwrap();

    let a = ask(&db, &tenant, "qué se vence?").await;
    assert_eq!(a.intent, "por_vencer");
    assert_eq!(a.data.unwrap()["count"], 1);
    assert!(a.text.contains("Amoxicilina"), "text: {}", a.text);
}

#[tokio::test]
async fn por_vencer_empty_is_graceful() {
    let (db, tenant, _user) = setup().await;
    let a = ask(&db, &tenant, "productos por vencer").await;
    assert_eq!(a.intent, "por_vencer");
    assert_eq!(a.data.unwrap()["count"], 0);
}

#[tokio::test]
async fn top_productos_ranks_sales() {
    let (db, tenant, user) = setup().await;
    seed_sales(&db, &tenant, &user).await;

    let a = ask(&db, &tenant, "top productos").await;
    assert_eq!(a.intent, "top_productos");
    let top = a.data.unwrap();
    // Paracetamol sold 2 units, Ibuprofeno 1 -> paracetamol ranks first.
    assert_eq!(top["top"][0]["name"], "Paracetamol 500");
    assert_eq!(top["top"][0]["qty"], 2);
}

#[tokio::test]
async fn margen_mes_computes_margin() {
    let (db, tenant, user) = setup().await;
    seed_sales(&db, &tenant, &user).await;

    let a = ask(&db, &tenant, "margen del mes").await;
    assert_eq!(a.intent, "margen_mes");
    let data = a.data.unwrap();
    // revenue 5000, cost 600*2 + 900*1 = 2100, margin 2900
    assert_eq!(data["revenue"], "5000");
    assert_eq!(data["cost"], "2100");
    assert_eq!(data["margin"], "2900");
}

#[tokio::test]
async fn resumen_inventario_reports_stats() {
    let (db, tenant, user) = setup().await;
    seed_sales(&db, &tenant, &user).await;

    let a = ask(&db, &tenant, "resumen de inventario").await;
    assert_eq!(a.intent, "resumen_inventario");
    assert_eq!(a.data.unwrap()["total"], 2);
}

#[tokio::test]
async fn ayuda_and_unknown_are_graceful() {
    let (db, tenant, _user) = setup().await;
    let help = ask(&db, &tenant, "ayuda").await;
    assert_eq!(help.intent, "ayuda");
    assert!(help.text.contains("vendí hoy"));

    let huh = ask(&db, &tenant, "cuéntame un chiste").await;
    assert_eq!(huh.intent, "desconocido");
    assert!(huh.text.to_lowercase().contains("no entendí"));
}

#[tokio::test]
async fn tenant_isolation_blocks_cross_reads() {
    let (db, tenant_a, user_a) = setup().await;
    seed_sales(&db, &tenant_a, &user_a).await;

    // Second tenant in the same db, no sales.
    let tenant_b: Thing = db
        .query("CREATE tenant SET name='B', slug='b' RETURN id")
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();

    let a = ask(&db, &tenant_b, "ventas hoy").await;
    assert_eq!(
        a.data.unwrap()["orders"],
        0,
        "tenant B must not see tenant A sales"
    );

    // And A still sees its own.
    let a2 = ask(&db, &tenant_a, "ventas hoy").await;
    assert_eq!(a2.data.unwrap()["orders"], 2);

    assert_eq!(parse("ventas hoy"), Intent::VentasHoy);
}

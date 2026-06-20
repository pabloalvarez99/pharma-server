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
async fn ventas_ayer_is_empty_when_only_today_seeded() {
    let (db, tenant, user) = setup().await;
    seed_sales(&db, &tenant, &user).await; // today
    let a = ask(&db, &tenant, "ventas de ayer").await;
    assert_eq!(a.intent, "ventas_ayer");
    // Sales were posted today, so yesterday's window sees none.
    assert_eq!(a.data.unwrap()["orders"], 0);
    assert!(a.text.to_lowercase().contains("ayer"), "text: {}", a.text);
}

#[tokio::test]
async fn comparativa_dia_contrasts_today_and_yesterday() {
    let (db, tenant, user) = setup().await;
    seed_sales(&db, &tenant, &user).await; // 5000 today, 0 yesterday
    let a = ask(&db, &tenant, "ventas de hoy vs ayer").await;
    assert_eq!(a.intent, "ventas_vs_ayer");
    let data = a.data.unwrap();
    assert_eq!(data["current_revenue"], "5000");
    assert_eq!(data["previous_revenue"], "0");
    assert_eq!(data["delta"], "5000");
}

#[tokio::test]
async fn ventas_por_metodo_breaks_down_cash_card() {
    let (db, tenant, user) = setup().await;
    seed_sales(&db, &tenant, &user).await; // all cash
    let a = ask(&db, &tenant, "ventas por método de pago").await;
    assert_eq!(a.intent, "ventas_metodo_pago");
    let data = a.data.unwrap();
    assert_eq!(data["cash"], "5000");
    assert_eq!(data["card"], "0");
    assert_eq!(data["cash_pct"], "100");
}

#[tokio::test]
async fn ventas_por_metodo_empty_is_graceful() {
    let (db, tenant, _user) = setup().await;
    let a = ask(&db, &tenant, "cómo pagan mis clientes").await;
    assert_eq!(a.intent, "ventas_metodo_pago");
    assert_eq!(a.data.unwrap()["orders"], 0);
}

#[tokio::test]
async fn margen_producto_computes_unit_margin() {
    let (db, tenant, user) = setup().await;
    seed_sales(&db, &tenant, &user).await;
    let a = ask(&db, &tenant, "margen de paracetamol").await;
    assert_eq!(a.intent, "margen_producto");
    let data = a.data.unwrap();
    // price 1500, cost 600 -> margin 900, 60%
    assert_eq!(data["price"], "1500");
    assert_eq!(data["cost"], "600");
    assert_eq!(data["margin"], "900");
    assert_eq!(data["margin_pct"], "60.0");
}

#[tokio::test]
async fn margen_producto_no_match_is_graceful() {
    let (db, tenant, _user) = setup().await;
    let a = ask(&db, &tenant, "margen de aspirina").await;
    assert_eq!(a.intent, "margen_producto");
    assert!(a.text.to_lowercase().contains("no encontré"));
}

#[tokio::test]
async fn gastos_mes_sums_and_breaks_down() {
    use domain::expenses::model::NewExpense;
    use domain::expenses::service as expenses;
    let (db, tenant, _user) = setup().await;
    for (cat, amt) in [("arriendo", "300000"), ("luz", "50000"), ("luz", "20000")] {
        expenses::create_expense(
            &db,
            &tenant,
            None,
            NewExpense {
                category: cat.into(),
                description: format!("{cat} test"),
                amount: dec(amt),
                payment_method: "cash".into(),
                cash_session: None,
                supplier: None,
                note: None,
                incurred_at: None,
            },
        )
        .await
        .unwrap();
    }
    let a = ask(&db, &tenant, "gastos del mes").await;
    assert_eq!(a.intent, "gastos_mes");
    let data = a.data.unwrap();
    assert_eq!(data["count"], 3);
    assert_eq!(data["total"], "370000");
}

#[tokio::test]
async fn gastos_mes_empty_is_graceful() {
    let (db, tenant, _user) = setup().await;
    let a = ask(&db, &tenant, "cuánto gasté este mes").await;
    assert_eq!(a.intent, "gastos_mes");
    assert_eq!(a.data.unwrap()["count"], 0);
}

#[tokio::test]
async fn clientes_top_empty_is_graceful() {
    let (db, tenant, _user) = setup().await;
    let a = ask(&db, &tenant, "mejores clientes").await;
    assert_eq!(a.intent, "clientes_top");
    // No loyalty-bearing customers seeded -> friendly empty.
    assert_eq!(a.data.unwrap()["count"], 0);
}

#[tokio::test]
async fn por_vencer_semana_uses_seven_day_window() {
    let (db, tenant, _user) = setup().await;
    let p = catalog::create_product(&db, &tenant, new_product("Amoxicilina", "3000", "1200", 0))
        .await
        .unwrap();
    // Expires in 20 days: inside the 30-day window, OUTSIDE the 7-day one.
    inventory::create_batch(
        &db,
        &tenant,
        imodel::NewBatch {
            product: p.id.clone(),
            batch_code: "L1".into(),
            expiry_date: Utc::now() + Duration::days(20),
            stock: 10,
            cost: Some(dec("1200")),
            notes: None,
        },
        None,
    )
    .await
    .unwrap();

    let week = ask(&db, &tenant, "qué se vence esta semana").await;
    assert_eq!(week.intent, "por_vencer_semana");
    assert_eq!(week.data.unwrap()["count"], 0, "20d batch is outside 7d");

    let month = ask(&db, &tenant, "qué se vence").await;
    assert_eq!(month.intent, "por_vencer");
    assert_eq!(month.data.unwrap()["count"], 1, "20d batch is inside 30d");
}

#[tokio::test]
async fn contract_shape_is_stable() {
    // ye builds the UI against {intent, text, data?}. Lock that shape.
    let (db, tenant, user) = setup().await;
    seed_sales(&db, &tenant, &user).await;
    let a = ask(&db, &tenant, "ventas hoy").await;
    let json = serde_json::to_value(&a).unwrap();
    assert!(json.get("intent").is_some(), "intent field present");
    assert!(json.get("text").is_some(), "text field present");
    assert!(json.get("data").is_some(), "data field present");
    // Unknown carries no data -> the field is omitted, never null.
    let huh = ask(&db, &tenant, "asdf qwer").await;
    let hjson = serde_json::to_value(&huh).unwrap();
    assert_eq!(hjson["intent"], "desconocido");
    assert!(hjson.get("data").is_none(), "data omitted when absent");
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

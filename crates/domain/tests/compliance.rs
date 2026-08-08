//! Libro de compras + resumen IVA (V3).

use rust_decimal::Decimal;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

use domain::compliance::{model::InvoiceInput, repo};

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

async fn seed_supplier(db: &Db, tenant: &Thing, name: &str, rut: &str) -> Thing {
    let mut r = db
        .query("CREATE supplier SET tenant = $t, name = $n, rut = $r RETURN id")
        .bind(("t", tenant.clone()))
        .bind(("n", name.to_string()))
        .bind(("r", rut.to_string()))
        .await
        .unwrap();
    let id: Option<Thing> = r.take((0, "id")).unwrap();
    id.expect("supplier id")
}

/// OC recepcionada con `updated_at` dentro del período pedido.
async fn seed_received_po(db: &Db, tenant: &Thing, supplier: &Thing, total: f64) -> Thing {
    let mut r = db
        .query(
            "CREATE purchase_order SET tenant = $t, supplier = $s, status = 'received', \
             currency = 'CLP', total = $tot RETURN id",
        )
        .bind(("t", tenant.clone()))
        .bind(("s", supplier.clone()))
        .bind(("tot", total))
        .await
        .unwrap();
    let id: Option<Thing> = r.take((0, "id")).unwrap();
    id.expect("po id")
}

async fn seed_sale(db: &Db, tenant: &Thing, total: f64) {
    db.query(
        "CREATE order SET tenant = $t, status = 'paid', payment_method = 'pos_cash', \
         subtotal = $tot, discount = 0, total = $tot",
    )
    .bind(("t", tenant.clone()))
    .bind(("tot", total))
    .await
    .unwrap();
}

fn d(s: &str) -> Decimal {
    s.parse().unwrap()
}

/// Período del "ahora" (las filas se crean con time::now()).
fn current_period() -> String {
    chrono::Utc::now().format("%Y-%m").to_string()
}

#[tokio::test]
async fn period_bounds_rejects_garbage() {
    assert!(repo::period_bounds("2026-13").is_err());
    assert!(repo::period_bounds("nope").is_err());
    assert!(repo::period_bounds("2026-07").is_ok());
}

#[tokio::test]
async fn book_derives_iva_when_invoice_not_declared() {
    let (db, t) = setup().await;
    let s = seed_supplier(&db, &t, "Distribuidora Sur", "76.086.428-5").await;
    seed_received_po(&db, &t, &s, 11900.0).await;

    let book = repo::purchase_book(&db, &t, &current_period())
        .await
        .unwrap();
    assert_eq!(book.rows.len(), 1);
    let row = &book.rows[0];
    // 11900 IVA-incluido → neto 10000, IVA 1900.
    assert_eq!(row.neto, d("10000"));
    assert_eq!(row.iva, d("1900"));
    assert_eq!(row.total, d("11900"));
    assert!(!row.declared, "sin factura capturada debe quedar derivada");
    assert_eq!(row.tipo, 33, "por defecto factura afecta");
    assert_eq!(row.supplier_name, "Distribuidora Sur");
    assert_eq!(book.pending_declaration, 1);
    assert_eq!(book.total_iva, d("1900"));
}

#[tokio::test]
async fn declared_invoice_overrides_derived_amounts() {
    let (db, t) = setup().await;
    let s = seed_supplier(&db, &t, "Prov", "1-9").await;
    let po = seed_received_po(&db, &t, &s, 11900.0).await;

    repo::set_invoice(
        &db,
        &t,
        &po,
        &InvoiceInput {
            folio: Some("A-123".into()),
            tipo: Some(33),
            neto: Some(d("9000")),
            iva: Some(d("1710")),
            total: Some(d("10710")),
            date: None,
        },
    )
    .await
    .unwrap();

    let book = repo::purchase_book(&db, &t, &current_period())
        .await
        .unwrap();
    let row = &book.rows[0];
    assert!(row.declared);
    assert_eq!(row.folio.as_deref(), Some("A-123"));
    assert_eq!(row.neto, d("9000"));
    assert_eq!(row.iva, d("1710"));
    assert_eq!(row.total, d("10710"));
    assert_eq!(book.pending_declaration, 0);
}

#[tokio::test]
async fn draft_purchase_orders_are_not_in_the_book() {
    let (db, t) = setup().await;
    let s = seed_supplier(&db, &t, "Prov", "1-9").await;
    db.query(
        "CREATE purchase_order SET tenant = $t, supplier = $s, status = 'draft', \
         currency = 'CLP', total = 5000",
    )
    .bind(("t", t.clone()))
    .bind(("s", s.clone()))
    .await
    .unwrap();

    let book = repo::purchase_book(&db, &t, &current_period())
        .await
        .unwrap();
    assert!(book.rows.is_empty(), "una OC en borrador no es una compra");
}

#[tokio::test]
async fn iva_summary_nets_debito_against_credito() {
    let (db, t) = setup().await;
    let s = seed_supplier(&db, &t, "Prov", "1-9").await;
    seed_received_po(&db, &t, &s, 11900.0).await; // crédito 1900
    seed_sale(&db, &t, 23800.0).await; // débito 3800

    let sum = repo::iva_summary(&db, &t, &current_period()).await.unwrap();
    assert_eq!(sum.iva_credito, d("1900"));
    assert_eq!(sum.iva_debito, d("3800"));
    assert_eq!(sum.iva_a_pagar, d("1900"));
    assert_eq!(sum.ventas_neto, d("20000"));
    assert_eq!(sum.compras_neto, d("10000"));
}

#[tokio::test]
async fn other_period_is_empty() {
    let (db, t) = setup().await;
    let s = seed_supplier(&db, &t, "Prov", "1-9").await;
    seed_received_po(&db, &t, &s, 11900.0).await;

    let book = repo::purchase_book(&db, &t, "2001-01").await.unwrap();
    assert!(book.rows.is_empty());
    assert_eq!(book.total_iva, d("0"));
}

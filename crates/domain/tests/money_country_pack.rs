//! Country pack — la moneda y la tasa de impuesto salen del tenant, no de Chile.
//!
//! Dos ejes que estos tests cuidan, y que son donde se rompen los ERPs mal
//! portados:
//!
//! 1. **Decimales por moneda.** El MISMO carrito, con los mismos números,
//!    calculado en CLP (0 decimales) y en USD (2 decimales), tiene que dar el
//!    total correcto en cada una — que no es el mismo número.
//! 2. **Cero regresión para Chile.** Un tenant que ya existe y nunca configuró
//!    `money.currency` ni `money.tax_rate` se comporta exactamente igual que
//!    antes del country pack: CLP, 19%, peso entero.

use rust_decimal::Decimal;
use std::str::FromStr;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

use domain::catalog::{model::NewProduct, service as catalog};
use domain::compliance::repo as compliance;
use domain::money::{Currency, MoneyConfig};
use domain::sales::{model::*, service as sales};
use domain::settings;

type Db = Surreal<surrealdb::engine::local::Db>;

async fn setup() -> (Db, Thing, Thing) {
    let db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations)
        .await
        .expect("migrations");
    let mut r = db
        .query("CREATE tenant SET name = 'Negocio Test', slug = 'test' RETURN id")
        .await
        .unwrap();
    let tenant: Option<Thing> = r.take((0, "id")).unwrap();
    let tenant = tenant.expect("tenant id");
    let mut r = db
        .query(
            "CREATE user SET tenant=$t, email='admin@test.local', \
             password='x', roles=['admin'] RETURN id",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let admin: Option<Thing> = r.take((0, "id")).unwrap();
    (db, tenant, admin.expect("admin id"))
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn np(name: &str, price: &str, stock: i64) -> NewProduct {
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
        discount_percent: None,
        attrs: None,
    }
}

/// Vende `qty` unidades a `unit_price`, pagando `cash` en efectivo.
async fn sell(
    db: &Db,
    tenant: &Thing,
    admin: &Thing,
    unit_price: &str,
    qty: i64,
    cash: &str,
) -> (OrderDto, ReceiptDto) {
    let p = catalog::create_product(db, tenant, np("Producto", unit_price, 100))
        .await
        .unwrap();
    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: p.id.clone(),
            product_name: p.name.clone(),
            quantity: qty,
            unit_price: dec(unit_price),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec(cash)),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
        branch: None,
    };
    let resp = sales::post_sale(db, tenant, Some(admin), Some("admin"), None, req)
        .await
        .unwrap();
    let receipt = sales::get_receipt(db, tenant, &resp.order.id).await.unwrap();
    (resp.order, receipt)
}

// --- eje 1: el mismo carrito en dos monedas --------------------------------

#[tokio::test]
async fn el_mismo_carrito_en_clp_redondea_a_peso_entero() {
    let (db, tenant, admin) = setup().await;
    // Sin settings: es el tenant chileno de siempre.
    let (order, receipt) = sell(&db, &tenant, &admin, "12.50", 3, "50").await;

    // 12.50 × 3 = 37.50 → en CLP no existe el medio peso.
    assert_eq!(order.total, dec("38"), "CLP debe quedar en peso entero");
    assert_eq!(receipt.change, Some(dec("12")), "vuelto en pesos enteros");
}

#[tokio::test]
async fn el_mismo_carrito_en_usd_conserva_los_centavos() {
    let (db, tenant, admin) = setup().await;
    settings::set_currency(&db, &tenant, "USD").await.unwrap();

    let (order, receipt) = sell(&db, &tenant, &admin, "12.50", 3, "50.00").await;

    // Mismo carrito, misma aritmética, otra moneda: 37.50, NO 38.
    assert_eq!(order.total, dec("37.50"), "USD debe conservar centavos");
    assert_ne!(
        order.total,
        dec("38"),
        "redondear USD a entero es el bug clásico del ERP mal portado"
    );
    assert_eq!(receipt.change, Some(dec("12.50")), "vuelto con centavos");
}

#[tokio::test]
async fn una_moneda_de_dos_decimales_no_hereda_el_redondeo_de_chile() {
    for code in ["USD", "EUR", "MXN"] {
        let (db, tenant, admin) = setup().await;
        settings::set_currency(&db, &tenant, code).await.unwrap();
        let (order, _) = sell(&db, &tenant, &admin, "0.99", 3, "5.00").await;
        assert_eq!(order.total, dec("2.97"), "{code} perdió centavos");
    }
}

// --- eje 2: cero regresión para un tenant chileno --------------------------

#[tokio::test]
async fn tenant_sin_settings_es_clp_19_igual_que_antes() {
    let (db, tenant, _admin) = setup().await;
    let cfg = settings::money_config(&db, &tenant).await.unwrap();

    assert_eq!(cfg, MoneyConfig::default());
    assert_eq!(cfg.currency.code(), "CLP");
    assert_eq!(cfg.tax_percent, Decimal::from(19));
    assert_eq!(cfg.decimals(), 0);
}

#[tokio::test]
async fn venta_chilena_da_exactamente_los_mismos_numeros_que_antes() {
    let (db, tenant, admin) = setup().await;
    let (order, receipt) = sell(&db, &tenant, &admin, "1500", 3, "5000").await;

    assert_eq!(order.subtotal, dec("4500"));
    assert_eq!(order.discount, Decimal::ZERO);
    assert_eq!(order.total, dec("4500"));
    assert_eq!(receipt.change, Some(dec("500")));
}

#[tokio::test]
async fn el_desglose_de_impuesto_chileno_no_se_movio() {
    let cfg = MoneyConfig::default();
    // 11.900 CLP IVA-incluido al 19% = 10.000 neto + 1.900 de IVA.
    let (neto, iva) = cfg.tax_breakdown(dec("11900"));
    assert_eq!(neto, dec("10000"));
    assert_eq!(iva, dec("1900"));
    // Y el legacy `iva_breakdown` sigue dando lo mismo, byte por byte.
    assert_eq!(
        domain::invariants::iva_breakdown(dec("11900"), 19),
        (neto, iva)
    );
}

// --- impuesto: tasa y decimales por tenant ---------------------------------

#[tokio::test]
async fn el_impuesto_respeta_los_decimales_de_la_moneda() {
    let clp = MoneyConfig::new(Currency::parse("CLP").unwrap(), Decimal::from(19));
    let usd = MoneyConfig::new(Currency::parse("USD").unwrap(), Decimal::from(19));

    assert_eq!(clp.tax_breakdown(dec("119")), (dec("100"), dec("19")));
    assert_eq!(
        usd.tax_breakdown(dec("119.00")),
        (dec("100.00"), dec("19.00"))
    );

    // 100 USD al 19% no divide exacto en centavos; el neto se redondea y el
    // impuesto sale por diferencia, así `neto + impuesto == total` SIEMPRE.
    let (neto, tax) = usd.tax_breakdown(dec("100.00"));
    assert_eq!(neto + tax, dec("100.00"));
    assert_eq!(neto, dec("84.03"));
}

#[tokio::test]
async fn tasas_fraccionarias_funcionan() {
    // 8.25% no se puede expresar con el `u8` que tenía `IVA_DEFAULT_PERCENT`.
    let usd = MoneyConfig::new(Currency::parse("USD").unwrap(), dec("8.25"));
    let (neto, tax) = usd.tax_breakdown(dec("108.25"));
    assert_eq!(neto + tax, dec("108.25"));
    assert_eq!(neto, dec("100.00"));
    assert_eq!(tax, dec("8.25"));
}

#[tokio::test]
async fn tenant_exento_no_paga_impuesto() {
    let (db, tenant, _admin) = setup().await;
    settings::set_tax_percent(&db, &tenant, "0").await.unwrap();
    let cfg = settings::money_config(&db, &tenant).await.unwrap();

    assert_eq!(cfg.tax_percent, Decimal::ZERO);
    assert_eq!(cfg.tax_breakdown(dec("5000")), (dec("5000"), Decimal::ZERO));
}

#[tokio::test]
async fn el_resumen_de_impuesto_usa_la_tasa_del_tenant() {
    let (db, tenant, _admin) = setup().await;
    settings::set_currency(&db, &tenant, "ARS").await.unwrap();
    settings::set_tax_percent(&db, &tenant, "21").await.unwrap();

    let period = format!("{}", chrono::Utc::now().format("%Y-%m"));
    db.query(
        "CREATE order SET tenant = $t, status = 'paid', payment_method = 'pos_cash', \
         subtotal = 121.00, discount = 0, total = 121.00",
    )
    .bind(("t", tenant.clone()))
    .await
    .unwrap();

    let summary = compliance::iva_summary(&db, &tenant, &period).await.unwrap();

    // Al 21% sobre 121.00: neto 100.00, impuesto 21.00. Con el 19% hardcodeado
    // de antes habría dado 101.68 / 19.32.
    assert_eq!(summary.ventas_neto, dec("100.00"));
    assert_eq!(summary.iva_debito, dec("21.00"));
}

// --- lecturas defensivas ---------------------------------------------------

#[tokio::test]
async fn escribir_una_moneda_invalida_falla_en_la_escritura() {
    let (db, tenant, _admin) = setup().await;
    let err = settings::set_currency(&db, &tenant, "pesos")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
    assert!(err.to_string().contains("ISO-4217"), "{err}");

    let err = settings::set_tax_percent(&db, &tenant, "mucho")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");

    // Y el k/v genérico (el que expone `PUT /api/v1/settings/{key}`) valida
    // igual: si no, la moneda mal escrita se descubre recién en la venta.
    let err = sales::set_setting(&db, &tenant, "money.currency", "pesos")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
    let err = sales::set_setting(&db, &tenant, "money.tax_rate", "999")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
}

#[tokio::test]
async fn un_setting_corrupto_cae_al_default_y_no_voltea_la_venta() {
    let (db, tenant, admin) = setup().await;
    // Escrito por fuera del setter validado: import viejo, edición a mano sobre
    // la base, o una fila que quedó de antes de que la validación existiera.
    domain::sales::repo::upsert_setting(&db, &tenant, "money.currency", "???")
        .await
        .unwrap();
    domain::sales::repo::upsert_setting(&db, &tenant, "money.tax_rate", "-5")
        .await
        .unwrap();

    let cfg = settings::money_config(&db, &tenant).await.unwrap();
    assert_eq!(cfg, MoneyConfig::default());

    // Y la venta pasa igual: el mostrador no se cae por un setting mal escrito.
    let (order, _) = sell(&db, &tenant, &admin, "1000", 2, "2000").await;
    assert_eq!(order.total, dec("2000"));
}

#[tokio::test]
async fn la_moneda_se_normaliza_al_guardarla() {
    let (db, tenant, _admin) = setup().await;
    settings::set_currency(&db, &tenant, " usd ").await.unwrap();
    let cfg = settings::money_config(&db, &tenant).await.unwrap();
    assert_eq!(cfg.currency.code(), "USD");
    assert_eq!(cfg.decimals(), 2);
}

//! Prueba de fuego del core agnóstico: vender un **servicio** (rubro
//! belleza/servicios) end-to-end, SIN bien físico, y emitir su boleta 39.
//!
//! La vitrina RutBusiness ofrece 8 rubros pero sólo farmacia/minimarket eran
//! reales. Este test prueba en vivo que el seed de servicios + el path POS +
//! el renderer DTE funcionan para una línea de servicio:
//!
//! 1. `seed_demo("servicios")` siembra el catálogo de servicios.
//! 2. Una venta POS de un servicio tiene éxito (el servicio se vende como
//!    cualquier ítem: el stock alto `SERVICE_STOCK` deja pasar el chequeo
//!    incondicional `stock < qty` del server).
//! 3. El invariante del ledger se mantiene tras la venta:
//!    `product.stock == Σ stock_movement.delta`.
//! 4. La boleta 39 (`dte::xml::render_unsigned`) produce un XML válido para esa
//!    línea de servicio.

use chrono::Utc;
use domain::catalog::service as catalog;
use domain::sales::{model::*, service as sales};
use domain::seed;
use rust_decimal::Decimal;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

type Db = Surreal<surrealdb::engine::local::Db>;

async fn setup() -> (Db, Thing, Thing) {
    let db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations)
        .await
        .expect("migrations");
    let mut r = db
        .query("CREATE tenant SET name = 'Salón Test', slug = 'test' RETURN id")
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

/// Σ de los `delta` de los movimientos de stock de un producto. El invariante
/// del ledger exige `product.stock == Σ delta`.
async fn movement_sum(db: &Db, tenant: &Thing, product_id: &str) -> i64 {
    let product = surrealdb::sql::thing(product_id).unwrap();
    let mut r = db
        .query(
            "SELECT VALUE delta FROM stock_movement \
             WHERE tenant = $t AND product = $p",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", product))
        .await
        .unwrap();
    let deltas: Vec<i64> = r.take(0).unwrap();
    deltas.iter().sum()
}

#[tokio::test]
async fn sells_a_service_end_to_end_and_emits_boleta() {
    let (db, tenant, admin) = setup().await;

    // 1) Sembrar el rubro servicio.
    let summary = seed::seed_demo(&db, &tenant, "servicios", false)
        .await
        .expect("seed servicios");
    assert_eq!(summary.vertical, "servicios");
    assert!(
        summary.products_created >= 10,
        "catálogo de servicios creíble"
    );
    assert_eq!(
        summary.products_created, summary.movements_emitted,
        "cada servicio entra por un lote → un movimiento (invariante del ledger)"
    );

    // 2) Tomar un servicio sembrado (por su external_id demo determinístico).
    let mut r = db
        .query(
            "SELECT VALUE id FROM product \
             WHERE tenant = $t AND external_id = 'DEMO-corte-de-pelo-varon'",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let ids: Vec<Thing> = r.take(0).unwrap();
    let service_id = ids.first().expect("servicio sembrado").to_string();

    let before = catalog::get_product(&db, &tenant, &service_id)
        .await
        .unwrap();
    assert!(before.stock > 0, "servicio con stock-proxy ilimitado");
    // El invariante del ledger ya se cumple post-seed.
    assert_eq!(
        before.stock,
        movement_sum(&db, &tenant, &service_id).await,
        "post-seed: product.stock == Σ delta"
    );

    // 3) Venta POS del servicio (sin bien físico, pero el path es el mismo).
    let qty = 2;
    let unit_price = before.price;
    let req = PosSaleRequest {
        items: vec![PosSaleItem {
            product: service_id.clone(),
            product_name: before.name.clone(),
            quantity: qty,
            unit_price,
        }],
        payment_method: "pos_debit".into(),
        cash_amount: None,
        card_amount: Some(unit_price * Decimal::from(qty)),
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
    };
    let resp = sales::post_sale(&db, &tenant, Some(&admin), Some("admin"), None, req)
        .await
        .expect("la venta de un servicio debe tener éxito");
    assert_eq!(resp.items.len(), 1);
    assert_eq!(resp.items[0].quantity, qty);
    assert_eq!(resp.order.total, unit_price * Decimal::from(qty));

    // 4) Invariante del ledger tras la venta: stock bajó qty y sigue cuadrando
    //    con el ledger de movimientos.
    let after = catalog::get_product(&db, &tenant, &service_id)
        .await
        .unwrap();
    assert_eq!(after.stock, before.stock - qty, "el servicio descontó qty");
    assert_eq!(
        after.stock,
        movement_sum(&db, &tenant, &service_id).await,
        "post-venta: product.stock == Σ delta"
    );

    // 5) Boleta 39 para la línea de servicio → XML válido.
    let emisor = dte::EmisorConfig {
        rut: "76.123.456-7".into(),
        razon_social: "Salón Demo SpA".into(),
        giro: "Servicios de peluquería y estética".into(),
        direccion: "Av. Siempre Viva 100".into(),
        comuna: "Providencia".into(),
        ciudad: Some("Santiago".into()),
        acteco: Some(960200),
    };
    let line_total = unit_price * Decimal::from(qty);
    let doc = dte::Dte {
        id: uuid::Uuid::new_v4(),
        tipo: dte::DteTipo::BoletaElectronica,
        folio: 1,
        fecha_emision: Utc::now(),
        rut_emisor: emisor.rut.clone(),
        rut_receptor: "66666666-6".into(),
        razon_social_receptor: "SIN INFORMACION".into(),
        giro_receptor: None,
        direccion_receptor: None,
        comuna_receptor: None,
        ind_traslado: None,
        referencias: vec![],
        descuentos_globales: vec![],
        monto_neto: Decimal::ZERO,
        iva: Decimal::ZERO,
        monto_exento: Decimal::ZERO,
        monto_total: line_total,
        items: vec![dte::DteItem {
            nro_linea: 1,
            nombre: before.name.clone(),
            cantidad: Decimal::from(qty),
            precio_unitario: unit_price,
            descuento_pct: None,
            monto_item: line_total,
            codigo_sku: None,
            unidad_medida: None,
            exento: false,
        }],
        estado: dte::DteEstado::Draft,
        xml_firmado: None,
        timbre: None,
        track_id: None,
        sii_glosa: None,
        metadata: None,
    };
    let xml = dte::xml::render_unsigned(&doc, &emisor).expect("render boleta servicio");
    assert!(xml.contains("<TipoDTE>39</TipoDTE>"), "es boleta 39");
    assert!(
        xml.contains(&before.name),
        "la línea de servicio aparece en la boleta"
    );
    assert!(
        xml.contains(&format!("<MntTotal>{}</MntTotal>", line_total.trunc())),
        "el total de la boleta es el del servicio"
    );
}

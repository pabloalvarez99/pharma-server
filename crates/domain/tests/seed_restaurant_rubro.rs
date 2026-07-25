//! Prueba de fuego del rubro **mixto** restaurant: en un mismo catálogo conviven
//! insumos físicos (stock + lote + vencimiento) y platos preparados vendibles
//! SIN inventario (`physical_stock = false`, migración 0031). Este test corre el
//! caso mixto end-to-end:
//!
//! 1. `seed_demo("restaurant")` siembra insumos (con lote + movimiento) y platos
//!    (stock 0, sin lote, sin movimiento) en el mismo tenant.
//! 2. Una venta POS mezcla un **plato** (sin stock) y un **insumo** (con stock):
//!    el plato salta el chequeo de stock; el insumo lo descuenta.
//! 3. La boleta 39 (`dte::xml::render_unsigned`) produce un XML válido con ambas
//!    líneas.
//! 4. Invariantes intactos: el insumo cumple `product.stock == Σ lote == Σ
//!    movimiento`; el plato no movió inventario (stock 0, sin movimientos).

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
        .query("CREATE tenant SET name = 'Restaurant Test', slug = 'test' RETURN id")
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

/// Σ de los `delta` de los movimientos de stock de un producto (ledger).
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

/// Σ del stock de los lotes de un producto.
async fn batch_sum(db: &Db, tenant: &Thing, product_id: &str) -> i64 {
    let product = surrealdb::sql::thing(product_id).unwrap();
    let mut r = db
        .query(
            "SELECT VALUE stock FROM product_batch \
             WHERE tenant = $t AND product = $p",
        )
        .bind(("t", tenant.clone()))
        .bind(("p", product))
        .await
        .unwrap();
    let stocks: Vec<i64> = r.take(0).unwrap();
    stocks.iter().sum()
}

/// Id del producto demo por su `external_id` determinístico.
async fn product_by_external(db: &Db, tenant: &Thing, external_id: &str) -> String {
    let mut r = db
        .query("SELECT VALUE id FROM product WHERE tenant = $t AND external_id = $e")
        .bind(("t", tenant.clone()))
        .bind(("e", external_id.to_string()))
        .await
        .unwrap();
    let ids: Vec<Thing> = r.take(0).unwrap();
    ids.first().expect("producto sembrado").to_string()
}

#[tokio::test]
async fn restaurant_sells_plato_and_insumo_end_to_end() {
    let (db, tenant, admin) = setup().await;

    // 1) Sembrar el rubro restaurant (mixto: insumos físicos + platos sin stock).
    let summary = seed::seed_demo(&db, &tenant, "restaurant", false)
        .await
        .expect("seed restaurant");
    assert_eq!(summary.vertical, "restaurant");
    assert!(
        summary.products_created >= 10,
        "catálogo de restaurant creíble"
    );
    // Sólo los insumos (con lote) emiten movimiento; los platos no.
    assert_eq!(
        summary.batches_created, summary.movements_emitted,
        "cada lote con stock emite un movimiento"
    );
    assert!(
        summary.batches_created >= 3,
        "los insumos físicos entran su stock por lote"
    );
    assert!(
        summary.products_created > summary.batches_created,
        "hay platos sin lote además de los insumos con lote"
    );

    // 2) Tomar un PLATO (sin stock físico) y un INSUMO (con stock).
    let plato_id = product_by_external(&db, &tenant, "DEMO-churrasco-italiano").await;
    let insumo_id = product_by_external(&db, &tenant, "DEMO-aceite-vegetal-5l").await;

    let plato = catalog::get_product(&db, &tenant, &plato_id).await.unwrap();
    let insumo = catalog::get_product(&db, &tenant, &insumo_id)
        .await
        .unwrap();
    assert!(
        !plato.physical_stock,
        "el plato se siembra como no físico (physical_stock = false)"
    );
    assert_eq!(plato.stock, 0, "plato sin inventario → stock 0");
    assert!(insumo.physical_stock, "el insumo es un bien físico");
    let insumo_stock_inicial = insumo.stock;
    assert!(
        insumo_stock_inicial > 0,
        "el insumo entró su stock por lote"
    );
    // Invariante post-seed del insumo: stock == Σ lote == Σ movimiento.
    assert_eq!(
        insumo_stock_inicial,
        batch_sum(&db, &tenant, &insumo_id).await
    );
    assert_eq!(
        insumo_stock_inicial,
        movement_sum(&db, &tenant, &insumo_id).await
    );

    // 3) Venta POS mixta: 1 plato + 2 insumos.
    let plato_qty = 1;
    let insumo_qty = 2;
    let plato_total = plato.price * Decimal::from(plato_qty);
    let insumo_total = insumo.price * Decimal::from(insumo_qty);
    let grand_total = plato_total + insumo_total;
    let req = PosSaleRequest {
        items: vec![
            PosSaleItem {
                product: plato_id.clone(),
                product_name: plato.name.clone(),
                quantity: plato_qty,
                unit_price: plato.price,
            },
            PosSaleItem {
                product: insumo_id.clone(),
                product_name: insumo.name.clone(),
                quantity: insumo_qty,
                unit_price: insumo.price,
            },
        ],
        payment_method: "pos_debit".into(),
        cash_amount: None,
        card_amount: Some(grand_total),
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
        branch: None,
    };
    let resp = sales::post_sale(&db, &tenant, Some(&admin), Some("admin"), None, req)
        .await
        .expect("la venta mixta debe tener éxito");
    assert_eq!(resp.items.len(), 2);
    assert_eq!(resp.order.total, grand_total);
    // Sólo el insumo mueve inventario; el plato no.
    assert_eq!(
        resp.stock_movements.len(),
        1,
        "sólo el insumo físico emite movimiento de venta"
    );

    // 4) Invariantes tras la venta.
    // Plato: sin inventario, stock sigue 0, sin movimientos.
    let plato_after = catalog::get_product(&db, &tenant, &plato_id).await.unwrap();
    assert_eq!(plato_after.stock, 0, "el plato no descuenta stock");
    assert_eq!(
        movement_sum(&db, &tenant, &plato_id).await,
        0,
        "el plato no emitió movimientos de inventario"
    );
    // Insumo: descuenta qty, invariante stock == Σ lote == Σ movimiento intacto.
    let insumo_after = catalog::get_product(&db, &tenant, &insumo_id)
        .await
        .unwrap();
    assert_eq!(
        insumo_after.stock,
        insumo_stock_inicial - insumo_qty,
        "el insumo descuenta la cantidad vendida"
    );
    assert_eq!(
        insumo_after.stock,
        batch_sum(&db, &tenant, &insumo_id).await,
        "invariante: product.stock == Σ lote"
    );
    assert_eq!(
        insumo_after.stock,
        movement_sum(&db, &tenant, &insumo_id).await,
        "invariante: product.stock == Σ movimiento"
    );

    // 5) Boleta 39 con ambas líneas → XML válido.
    let emisor = dte::EmisorConfig {
        rut: "76.123.456-7".into(),
        razon_social: "Restaurant Demo SpA".into(),
        giro: "Restaurantes y servicios de comida".into(),
        direccion: "Av. Siempre Viva 200".into(),
        comuna: "Santiago".into(),
        ciudad: Some("Santiago".into()),
        acteco: Some(561000),
    };
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
        monto_total: grand_total,
        items: vec![
            dte::DteItem {
                nro_linea: 1,
                nombre: plato.name.clone(),
                cantidad: Decimal::from(plato_qty),
                precio_unitario: plato.price,
                descuento_pct: None,
                monto_item: plato_total,
                codigo_sku: None,
                unidad_medida: None,
                exento: false,
            },
            dte::DteItem {
                nro_linea: 2,
                nombre: insumo.name.clone(),
                cantidad: Decimal::from(insumo_qty),
                precio_unitario: insumo.price,
                descuento_pct: None,
                monto_item: insumo_total,
                codigo_sku: None,
                unidad_medida: None,
                exento: false,
            },
        ],
        estado: dte::DteEstado::Draft,
        xml_firmado: None,
        timbre: None,
        track_id: None,
        sii_glosa: None,
        metadata: None,
    };
    let xml = dte::xml::render_unsigned(&doc, &emisor).expect("render boleta restaurant");
    assert!(xml.contains("<TipoDTE>39</TipoDTE>"), "es boleta 39");
    assert!(xml.contains(&plato.name), "la línea del plato aparece");
    assert!(xml.contains(&insumo.name), "la línea del insumo aparece");
    assert!(
        xml.contains(&format!("<MntTotal>{}</MntTotal>", grand_total.trunc())),
        "el total de la boleta es el de la venta mixta"
    );
}

//! Auditoría de los AGREGADOS DE PLATA, contra la ley del dominio.
//!
//! Carril hermano del que arregló el arqueo (migración 0046). La regla de este
//! archivo es una sola: **cada test tiene que fallar sin el arreglo**. Un test
//! que queda verde con y sin el fix no es un detector, es decoración.
//!
//! Se auditan las dos capas, porque el bug de 0046 vivía en el `DEFINE EVENT`
//! de SurrealDB y no en la función pura: todo lo de acá va contra la base con
//! las migraciones corridas y compara el número guardado contra un escaneo
//! independiente.
//!
//! Los cuatro olores que se persiguen:
//!   1. sumar lo ENTREGADO donde va lo COBRADO;
//!   2. tratar `None` como `0` cuando significa "no se registró";
//!   3. un total corrido que nadie comparó nunca contra un escaneo completo;
//!   4. plata que se mueve del cajón sin que el arqueo se entere (y al revés).

use std::str::FromStr;

use domain::cash_register::{model::*, service as caja};
use domain::catalog::{model::NewProduct, service as catalog};
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

fn producto(name: &str, price: &str, stock: i64) -> NewProduct {
    NewProduct {
        name: name.into(),
        slug: None,
        description: None,
        price: dec(price),
        cost_price: Some(dec("100")),
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

async fn abrir_caja(db: &Db, tenant: &Thing, user: &Thing, apertura: &str) -> CashSessionDto {
    caja::open_session(
        db,
        tenant,
        user,
        OpenSessionInput {
            register_name: "caja-1".into(),
            register: None,
            branch: None,
            opening_cash: dec(apertura),
            notes: None,
        },
    )
    .await
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn venta(
    product_id: &str,
    name: &str,
    price: &str,
    qty: i64,
    metodo: &str,
    cash: Option<&str>,
    customer: Option<&str>,
) -> smodel::PosSaleRequest {
    smodel::PosSaleRequest {
        items: vec![smodel::PosSaleItem {
            product: product_id.into(),
            product_name: name.into(),
            quantity: qty,
            unit_price: dec(price),
        }],
        payment_method: metodo.into(),
        cash_amount: cash.map(dec),
        card_amount: None,
        discount: None,
        customer: customer.map(str::to_string),
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
        branch: None,
    }
}

// ===========================================================================
// Olor 4 — plata que entra o sale del cajón sin que el arqueo se entere.
//
// El gasto en efectivo contra una caja abierta YA postea su `retiro`
// (`expenses::service::create_expense`). Estos dos son los hermanos que
// quedaron sin postear nada.
// ===========================================================================

/// **Un abono de fiado en efectivo es plata que ENTRA al cajón.**
///
/// `POST /customers/{id}/abono` acepta `cash_session` justamente para atarlo a
/// la caja abierta, pero no postea ningún `cash_movement`: el esperado del
/// arqueo se queda corto por el monto del abono y al cajero le SOBRA plata al
/// cerrar. Es el gasto en efectivo con el signo dado vuelta, y le pasa todos
/// los días al que fía — que es el caso normal del feriante.
#[tokio::test]
async fn abono_de_fiado_en_efectivo_entra_al_esperado_del_cajon() {
    let (db, tenant, user) = setup().await;
    let s = abrir_caja(&db, &tenant, &user, "10000").await;
    let p = catalog::create_product(&db, &tenant, producto("Palta kilo", "5000", 50))
        .await
        .unwrap();
    let cliente = domain::customers::service::create_customer(
        &db,
        &tenant,
        domain::customers::model::NewCustomer {
            name: "Doña Rosa".into(),
            rut: None,
            phone: None,
            email: None,
        },
    )
    .await
    .unwrap();

    // Venta fiada: NO mueve caja (no hay plata todavía).
    sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta(
            &p.id,
            &p.name,
            "5000",
            1,
            "pos_fiado",
            None,
            Some(&cliente.id),
        ),
    )
    .await
    .unwrap();
    let solo_venta = caja::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(
        solo_venta.session.closing_cash_expected,
        Some(dec("10000")),
        "la venta fiada no pone plata en el cajón"
    );

    // Al rato el cliente viene y paga $3.000 en efectivo, contra esta caja.
    let sid = surrealdb::sql::thing(&s.id).unwrap();
    let cid = surrealdb::sql::thing(&cliente.id).unwrap();
    domain::credit::service::record_abono(
        &db,
        &tenant,
        &cid,
        dec("3000"),
        Some(&sid),
        Some("abono en efectivo"),
        Some(&user),
    )
    .await
    .unwrap();

    let live = caja::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(
        live.movements_in,
        dec("3000"),
        "el abono en efectivo es un ingreso de caja: tiene que estar en los movimientos"
    );
    assert_eq!(
        live.session.closing_cash_expected,
        Some(dec("13000")),
        "esperado = apertura 10000 + 3000 del abono; sin esto al cajero le SOBRAN 3000"
    );

    // Y el cierre contando la plata real cuadra en cero.
    let close = caja::close_session(
        &db,
        &tenant,
        &s.id,
        CloseSessionInput {
            closing_cash_counted: dec("13000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        close.session.discrepancia,
        Some(Decimal::ZERO),
        "el cajón cuadra: los 3000 del abono están adentro"
    );
}

/// **Un pago a proveedor en efectivo es plata que SALE del cajón.**
///
/// `NewPurchasePayment.cash_session` se documenta como "surfaces the payment in
/// the drawer arqueo" y el servicio como "so arqueo can include them later" —
/// pero nadie postea el `retiro`. El esperado queda alto por el monto pagado y
/// aparece un faltante que nunca existió: exactamente el bug que
/// `create_expense` ya arregló para el gasto en efectivo.
#[tokio::test]
async fn pago_de_compra_en_efectivo_sale_del_esperado_del_cajon() {
    use domain::purchasing::{model as pmodel, service as compras};

    let (db, tenant, user) = setup().await;
    let s = abrir_caja(&db, &tenant, &user, "50000").await;
    let prov = compras::create_supplier(
        &db,
        &tenant,
        pmodel::NewSupplier {
            name: "Distribuidora".into(),
            rut: None,
            contact_name: None,
            contact_email: None,
            contact_phone: None,
            default_invoice_format: None,
        },
    )
    .await
    .unwrap();
    let oc = compras::create_purchase_order(
        &db,
        &tenant,
        pmodel::NewPurchaseOrder {
            supplier: prov.id.clone(),
            branch: None,
            currency: None,
            notes: None,
            external_ref: None,
            items: vec![pmodel::NewPurchaseOrderItem {
                product: None,
                product_name: "Cajón de paltas".into(),
                quantity: 1,
                unit_cost: dec("20000"),
            }],
        },
    )
    .await
    .unwrap();

    compras::create_purchase_payment(
        &db,
        &tenant,
        &oc.id,
        pmodel::NewPurchasePayment {
            amount: dec("20000"),
            currency: None,
            payment_method: Some("cash".into()),
            cash_session: Some(s.id.clone()),
            reference: None,
            note: None,
            paid_at: None,
        },
        Some(&user.to_string()),
    )
    .await
    .unwrap();

    let live = caja::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(
        live.movements_out,
        dec("20000"),
        "pagarle al proveedor con la plata de la caja es un retiro"
    );
    assert_eq!(
        live.session.closing_cash_expected,
        Some(dec("30000")),
        "esperado = apertura 50000 − 20000 pagados; sin esto aparece un faltante de 20000"
    );
}

/// Un pago que NO es en efectivo (transferencia desde el banco) no toca el
/// cajón, aunque el request traiga la sesión. Fija el borde del arreglo de
/// arriba: el mismo criterio que `create_expense` (sólo `cash` mueve el cajón).
#[tokio::test]
async fn pago_de_compra_por_transferencia_no_toca_el_cajon() {
    use domain::purchasing::{model as pmodel, service as compras};

    let (db, tenant, user) = setup().await;
    let s = abrir_caja(&db, &tenant, &user, "50000").await;
    let prov = compras::create_supplier(
        &db,
        &tenant,
        pmodel::NewSupplier {
            name: "Distribuidora".into(),
            rut: None,
            contact_name: None,
            contact_email: None,
            contact_phone: None,
            default_invoice_format: None,
        },
    )
    .await
    .unwrap();
    let oc = compras::create_purchase_order(
        &db,
        &tenant,
        pmodel::NewPurchaseOrder {
            supplier: prov.id.clone(),
            branch: None,
            currency: None,
            notes: None,
            external_ref: None,
            items: vec![pmodel::NewPurchaseOrderItem {
                product: None,
                product_name: "Cajón de paltas".into(),
                quantity: 1,
                unit_cost: dec("20000"),
            }],
        },
    )
    .await
    .unwrap();

    compras::create_purchase_payment(
        &db,
        &tenant,
        &oc.id,
        pmodel::NewPurchasePayment {
            amount: dec("20000"),
            currency: None,
            payment_method: Some("transfer".into()),
            cash_session: Some(s.id.clone()),
            reference: None,
            note: None,
            paid_at: None,
        },
        Some(&user.to_string()),
    )
    .await
    .unwrap();

    let live = caja::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(
        live.movements_out,
        Decimal::ZERO,
        "una transferencia sale del banco, no del cajón"
    );
    assert_eq!(
        live.session.closing_cash_expected,
        Some(dec("50000")),
        "el cajón no se movió"
    );
}

// ===========================================================================
// Olor 1 — restar un monto por otro: la devolución PARCIAL.
// ===========================================================================

/// **Devolver 1 de 3 unidades saca del cajón las 3.**
///
/// `apply_refund` marca la orden entera `status='refunded'` en cuanto hay una
/// devolución, aunque sea parcial (el dominio soporta devoluciones parciales
/// sucesivas: para eso existen `sum_prior_refunds_by_product` y
/// `refund_exceeds_sold`). El evento `cash_sales_running_maint` ve la orden
/// salir del filtro `status NOT IN ['refunded','cancelled']` y le resta el
/// efectivo NETO COMPLETO — pero por la caja sólo salió lo devuelto.
///
/// Es el mismo defecto que 0046 con el signo dado vuelta: ahí se sumaba de más
/// lo entregado, acá se resta de más lo vendido.
#[tokio::test]
async fn devolucion_parcial_resta_del_cajon_solo_lo_devuelto() {
    let (db, tenant, user) = setup().await;
    let s = abrir_caja(&db, &tenant, &user, "0").await;
    let p = catalog::create_product(&db, &tenant, producto("Palta kilo", "5000", 50))
        .await
        .unwrap();

    // 3 kilos en efectivo, pago exacto: entran 15.000 al cajón.
    let v = sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta(&p.id, &p.name, "5000", 3, "pos_cash", Some("15000"), None),
    )
    .await
    .unwrap();
    assert_eq!(
        caja::arqueo(&db, &tenant, &s.id).await.unwrap().cash_sales,
        dec("15000")
    );

    // El cliente devuelve UN kilo: por el cajón salen 5.000, no 15.000.
    sales::create_refund(
        &db,
        &tenant,
        Some(&user),
        smodel::NewDevolucion {
            order: Some(v.order.id.clone()),
            tipo: "venta".into(),
            motivo: "una estaba pasada".into(),
            notas: None,
            items: vec![smodel::NewDevolucionItem {
                product: Some(p.id.clone()),
                product_name: p.name.clone(),
                quantity: 1,
                unit_price: dec("5000"),
                restock: true,
            }],
            metodo_reembolso: Some("efectivo".into()),
        },
    )
    .await
    .unwrap();

    let live = caja::arqueo(&db, &tenant, &s.id).await.unwrap();
    // El cajón es libro de caja puro (0049): la venta aportó lo que entró al
    // cobrarla y se queda BRUTA; la devolución salió por un retiro con fecha
    // propia. Los dos números se muestran, nunca uno restado.
    assert_eq!(
        live.cash_sales,
        dec("15000"),
        "la venta entró completa: 15000. Devolver no reescribe lo que ya entró"
    );
    assert_eq!(
        live.movements_out,
        dec("5000"),
        "salieron 5000 por el kilo devuelto, no los 15000 de la venta entera"
    );
    let close = caja::close_session(
        &db,
        &tenant,
        &s.id,
        CloseSessionInput {
            closing_cash_counted: dec("10000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        close.session.discrepancia,
        Some(Decimal::ZERO),
        "el cajón tiene 10000 y el arqueo pide 10000"
    );
}

/// La contraparte: devolver TODO sí saca la venta entera del cajón. Sin este
/// test el arreglo de arriba se podría "pasar" no restando nunca nada.
#[tokio::test]
async fn devolucion_total_saca_la_venta_entera_del_cajon() {
    let (db, tenant, user) = setup().await;
    let s = abrir_caja(&db, &tenant, &user, "0").await;
    let p = catalog::create_product(&db, &tenant, producto("Palta kilo", "5000", 50))
        .await
        .unwrap();
    let v = sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta(&p.id, &p.name, "5000", 3, "pos_cash", Some("15000"), None),
    )
    .await
    .unwrap();
    sales::create_refund(
        &db,
        &tenant,
        Some(&user),
        smodel::NewDevolucion {
            order: Some(v.order.id.clone()),
            tipo: "venta".into(),
            motivo: "estaban todas pasadas".into(),
            notas: None,
            items: vec![smodel::NewDevolucionItem {
                product: Some(p.id.clone()),
                product_name: p.name.clone(),
                quantity: 3,
                unit_price: dec("5000"),
                restock: true,
            }],
            metodo_reembolso: Some("efectivo".into()),
        },
    )
    .await
    .unwrap();
    let live = caja::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(live.cash_sales, dec("15000"), "lo que entró al cobrar");
    assert_eq!(
        live.movements_out,
        dec("15000"),
        "se devolvió todo: por el cajón salieron los 15000"
    );
    // Lo que importa de verdad: el cajón quedó en cero y el arqueo lo pide en
    // cero. Bruto y neto son dos líneas, el esperado es una sola.
    let close = caja::close_session(
        &db,
        &tenant,
        &s.id,
        CloseSessionInput {
            closing_cash_counted: Decimal::ZERO,
            notes: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(close.session.discrepancia, Some(Decimal::ZERO));
    assert_eq!(
        close.session.closing_cash_expected,
        Some(Decimal::ZERO),
        "vendí 15000 y devolví 15000: el cajón espera 0"
    );
}

/// Devolución parcial y el reporte del día: la venta no puede desaparecer
/// entera del ingreso. `sales_daily` filtra `status NOT IN
/// ['refunded','cancelled']`, así que la orden parcialmente devuelta se borra
/// del reporte completa — el dueño ve $0 de venta en un día que vendió.
#[tokio::test]
async fn devolucion_parcial_deja_la_venta_no_devuelta_en_el_reporte_del_dia() {
    let (db, tenant, user) = setup().await;
    abrir_caja(&db, &tenant, &user, "0").await;
    let p = catalog::create_product(&db, &tenant, producto("Palta kilo", "5000", 50))
        .await
        .unwrap();
    let v = sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta(&p.id, &p.name, "5000", 3, "pos_cash", Some("15000"), None),
    )
    .await
    .unwrap();
    sales::create_refund(
        &db,
        &tenant,
        Some(&user),
        smodel::NewDevolucion {
            order: Some(v.order.id.clone()),
            tipo: "venta".into(),
            motivo: "una estaba pasada".into(),
            notas: None,
            items: vec![smodel::NewDevolucionItem {
                product: Some(p.id.clone()),
                product_name: p.name.clone(),
                quantity: 1,
                unit_price: dec("5000"),
                restock: true,
            }],
            metodo_reembolso: Some("efectivo".into()),
        },
    )
    .await
    .unwrap();

    let dias = domain::expenses::service::sales_daily(
        &db,
        &tenant,
        domain::expenses::model::SalesReportFilters::default(),
    )
    .await
    .unwrap();
    let efectivo: Decimal = dias.iter().map(|d| d.cash).sum();
    let bruto: Decimal = dias.iter().map(|d| d.revenue).sum();
    let devuelto: Decimal = dias.iter().map(|d| d.refunds).sum();
    assert_eq!(
        bruto,
        dec("15000"),
        "la venta ocurrió: devolver una parte no la borra del día"
    );
    assert_eq!(
        efectivo,
        dec("15000"),
        "el efectivo del día es bruto, igual que el del cajón"
    );
    assert_eq!(devuelto, dec("5000"), "la línea de devoluciones del día");
    assert_eq!(
        bruto - devuelto,
        dec("10000"),
        "vendió 15000 y devolvió 5000: el día cerró con 10000, no con 0"
    );
}

// ===========================================================================
// Olor 1 (fiado) — la deuda que sobrevive a la devolución.
// ===========================================================================

/// **Devolver una venta fiada no le baja la deuda al cliente.**
///
/// `post_cargo` escribe el cargo en `customer_ledger` cuando la venta es
/// `pos_fiado`, pero el camino de devolución no escribe nada: la mercadería
/// vuelve al stock y el cliente sigue debiendo. En un puesto de feria eso es
/// cobrarle dos veces a la vecina.
#[tokio::test]
async fn devolucion_de_venta_fiada_baja_la_deuda_del_cliente() {
    let (db, tenant, user) = setup().await;
    let p = catalog::create_product(&db, &tenant, producto("Palta kilo", "5000", 50))
        .await
        .unwrap();
    let cliente = domain::customers::service::create_customer(
        &db,
        &tenant,
        domain::customers::model::NewCustomer {
            name: "Doña Rosa".into(),
            rut: None,
            phone: None,
            email: None,
        },
    )
    .await
    .unwrap();
    let cid = surrealdb::sql::thing(&cliente.id).unwrap();

    let v = sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta(
            &p.id,
            &p.name,
            "5000",
            3,
            "pos_fiado",
            None,
            Some(&cliente.id),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        domain::credit::repo::balance(&db, &tenant, &cid)
            .await
            .unwrap(),
        dec("15000"),
        "quedó debiendo los 3 kilos"
    );

    // Devuelve un kilo.
    sales::create_refund(
        &db,
        &tenant,
        Some(&user),
        smodel::NewDevolucion {
            order: Some(v.order.id.clone()),
            tipo: "venta".into(),
            motivo: "una estaba pasada".into(),
            notas: None,
            items: vec![smodel::NewDevolucionItem {
                product: Some(p.id.clone()),
                product_name: p.name.clone(),
                quantity: 1,
                unit_price: dec("5000"),
                restock: true,
            }],
            metodo_reembolso: Some("fiado".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        domain::credit::repo::balance(&db, &tenant, &cid)
            .await
            .unwrap(),
        dec("10000"),
        "devolvió un kilo: debe 2, no 3"
    );
}

// ===========================================================================
// Olor 3 — totales corridos contra un escaneo completo.
//
// `cash_sales_running` (0030/0046) ya tenía su comparación. Los otros tres
// mantenedores de agregado vivos nunca se compararon contra nada:
// `product_stats_maint` (0029/0031), `product_branch_stock_maint` y
// `product_branch_stock_seed` (0041).
// ===========================================================================

/// Suma independiente del ledger: `Σ stock_movement.delta` por (producto,
/// sucursal). Es el oráculo del agregado `product_branch_stock`, salvo por el
/// stock inicial que nace en el CREATE del producto (ver más abajo).
async fn ledger_por_sucursal(db: &Db, tenant: &Thing, product: &str) -> Decimal {
    let pt = surrealdb::sql::thing(product).unwrap();
    let mut r = db
        .query("SELECT VALUE delta FROM stock_movement WHERE tenant=$t AND product=$p")
        .bind(("t", tenant.clone()))
        .bind(("p", pt))
        .await
        .unwrap();
    let deltas: Vec<i64> = r.take(0).unwrap();
    Decimal::from(deltas.iter().sum::<i64>())
}

async fn suma_buckets(db: &Db, tenant: &Thing, product: &str) -> i64 {
    let pt = surrealdb::sql::thing(product).unwrap();
    let mut r = db
        .query("SELECT VALUE stock FROM product_branch_stock WHERE tenant=$t AND product=$p")
        .bind(("t", tenant.clone()))
        .bind(("p", pt))
        .await
        .unwrap();
    let filas: Vec<i64> = r.take(0).unwrap();
    filas.iter().sum()
}

async fn stock_global(db: &Db, tenant: &Thing, product: &str) -> i64 {
    let pt = surrealdb::sql::thing(product).unwrap();
    let mut r = db
        .query("SELECT VALUE stock FROM product WHERE id=$p AND tenant=$t")
        .bind(("p", pt))
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let filas: Vec<i64> = r.take(0).unwrap();
    filas[0]
}

/// `product_branch_stock` (eventos `_maint` + `_seed`, migración 0041) contra
/// el escaneo completo, después de un día entero: alta con stock, venta,
/// devolución parcial, ajuste manual y recepción de lote.
///
/// Las dos identidades que el agregado promete:
///   * `Σ_sucursal product_branch_stock.stock == product.stock`
///   * `Σ product_branch_stock.stock == stock_inicial + Σ stock_movement.delta`
///
/// La segunda es la que separa el agregado del ledger: si el evento `_seed` y
/// el evento `_maint` contaran los dos la misma alta, esta suma daría el doble
/// y la primera igual pasaría.
#[tokio::test]
async fn product_branch_stock_cuadra_con_el_ledger_despues_del_dia_entero() {
    let (db, tenant, user) = setup().await;
    // Caja abierta: el día entero incluye una devolución en efectivo, y desde
    // la 0049 eso emite un retiro — sin caja abierta se rechaza. El arqueo no
    // es lo que mide este test, pero el día real tiene la caja abierta.
    abrir_caja(&db, &tenant, &user, "0").await;
    // Alta con stock inicial > 0: dispara `product_branch_stock_seed`.
    let p = catalog::create_product(&db, &tenant, producto("Palta kilo", "5000", 40))
        .await
        .unwrap();
    let inicial = Decimal::from(40i64);

    // Venta de 3 (movimiento -3).
    let v = sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta(&p.id, &p.name, "5000", 3, "pos_cash", Some("15000"), None),
    )
    .await
    .unwrap();
    // Devolución de 1 (movimiento +1).
    sales::create_refund(
        &db,
        &tenant,
        Some(&user),
        smodel::NewDevolucion {
            order: Some(v.order.id.clone()),
            tipo: "venta".into(),
            motivo: "pasada".into(),
            notas: None,
            items: vec![smodel::NewDevolucionItem {
                product: Some(p.id.clone()),
                product_name: p.name.clone(),
                quantity: 1,
                unit_price: dec("5000"),
                restock: true,
            }],
            metodo_reembolso: Some("efectivo".into()),
        },
    )
    .await
    .unwrap();
    // Ajuste manual (movimiento -2: se pudrieron dos).
    domain::inventory::service::add_movement(
        &db,
        &tenant,
        &p.id,
        -2,
        "merma",
        Some(&user.to_string()),
        None,
    )
    .await
    .unwrap();

    let buckets = suma_buckets(&db, &tenant, &p.id).await;
    let global = stock_global(&db, &tenant, &p.id).await;
    assert_eq!(
        Decimal::from(buckets),
        Decimal::from(global),
        "Σ product_branch_stock != product.stock"
    );
    assert_eq!(
        Decimal::from(buckets),
        inicial + ledger_por_sucursal(&db, &tenant, &p.id).await,
        "el agregado por sucursal no es el stock inicial + el ledger: \
         alguien está contando un movimiento dos veces o ninguna"
    );
    assert_eq!(global, 40 - 3 + 1 - 2, "cuenta a mano del día");
}

/// `product_stats` (evento `product_stats_maint`, 0029 redefinido por 0031)
/// contra `stats_scan`, después de que la PLATA y el STOCK se muevan por los
/// caminos que el test de catálogo no ejerce: una venta, una devolución y un
/// ajuste manual — todos hacen `UPDATE product SET stock = …`, que es la rama
/// donde el evento aplica su delta `(after − before)`.
///
/// `stats_view_matches_scan` (tests/catalog.rs) ya compara el agregado contra
/// el escaneo, pero sólo mueve el catálogo con create/update/delete de
/// producto. El stock que se mueve VENDIENDO nunca se comparó.
#[tokio::test]
async fn product_stats_cuadra_con_el_escaneo_despues_de_vender_y_devolver() {
    let (db, tenant, user) = setup().await;
    // Caja abierta: la devolución en efectivo emite un retiro desde la 0049.
    abrir_caja(&db, &tenant, &user, "0").await;
    // Uno que va a cruzar el umbral de stock bajo (5) vendiendo, uno que se
    // agota del todo, y uno que se queda quieto.
    let cruza = catalog::create_product(&db, &tenant, producto("Cruza", "1000", 7))
        .await
        .unwrap();
    let agota = catalog::create_product(&db, &tenant, producto("Agota", "1000", 2))
        .await
        .unwrap();
    let quieto = catalog::create_product(&db, &tenant, producto("Quieto", "1000", 50))
        .await
        .unwrap();

    // Vender 3 del primero (7 → 4: entra a stock bajo) y 2 del segundo (2 → 0:
    // entra a agotado y sigue contando como bajo).
    let v = sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta(&cruza.id, &cruza.name, "1000", 3, "pos_cash", Some("3000"), None),
    )
    .await
    .unwrap();
    sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta(&agota.id, &agota.name, "1000", 2, "pos_cash", Some("2000"), None),
    )
    .await
    .unwrap();

    let comparar = |db: &Db, tenant: &Thing| {
        let db = db.clone();
        let tenant = tenant.clone();
        async move {
            let vista = catalog::stats(&db, &tenant).await.unwrap();
            let scan = domain::catalog::repo::stats_scan_for_test(
                &db,
                &tenant,
                domain::catalog::model::LOW_STOCK_DEFAULT,
            )
            .await
            .unwrap();
            (vista, scan)
        }
    };

    let (vista, scan) = comparar(&db, &tenant).await;
    assert_eq!(vista.low_stock, scan.low_stock, "low_stock tras vender");
    assert_eq!(
        vista.out_of_stock, scan.out_of_stock,
        "out_of_stock tras vender"
    );
    assert_eq!(
        vista.inventory_value, scan.inventory_value,
        "inventory_value tras vender"
    );
    assert_eq!(vista.low_stock, 2, "cruza (4) y agota (0) quedaron bajos");
    assert_eq!(vista.out_of_stock, 1, "agota quedó en 0");

    // Devolver una del primero (4 → 5: sigue bajo) y ajustar el tercero a la
    // baja hasta cruzar el umbral.
    sales::create_refund(
        &db,
        &tenant,
        Some(&user),
        smodel::NewDevolucion {
            order: Some(v.order.id.clone()),
            tipo: "venta".into(),
            motivo: "pasada".into(),
            notas: None,
            items: vec![smodel::NewDevolucionItem {
                product: Some(cruza.id.clone()),
                product_name: cruza.name.clone(),
                quantity: 1,
                unit_price: dec("1000"),
                restock: true,
            }],
            metodo_reembolso: Some("efectivo".into()),
        },
    )
    .await
    .unwrap();
    domain::inventory::service::add_movement(
        &db,
        &tenant,
        &quieto.id,
        -46,
        "merma",
        Some(&user.to_string()),
        None,
    )
    .await
    .unwrap();

    let (vista, scan) = comparar(&db, &tenant).await;
    assert_eq!(
        vista.low_stock, scan.low_stock,
        "low_stock tras devolver y ajustar"
    );
    assert_eq!(
        vista.out_of_stock, scan.out_of_stock,
        "out_of_stock tras devolver y ajustar"
    );
    assert_eq!(
        vista.inventory_value, scan.inventory_value,
        "inventory_value tras devolver y ajustar"
    );
    assert_eq!(vista.total, scan.total);
    assert_eq!(vista.active, scan.active);
}

// ===========================================================================
// Idempotencia, borde sin caja, y el defecto que queda anotado.
// ===========================================================================

/// Cuántos `cash_movement(tipo='retiro')` tiene la sesión. Cuenta FILAS y no
/// suma montos: dos retiros de la mitad cada uno sumarían lo mismo que uno
/// correcto y el test no vería nada.
async fn retiros(db: &Db, tenant: &Thing, session_id: &str) -> i64 {
    let sid = surrealdb::sql::thing(session_id).unwrap();
    let mut r = db
        .query(
            "SELECT count() AS c FROM cash_movement \
             WHERE tenant = $t AND session = $s AND tipo = 'retiro' GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .bind(("s", sid))
        .await
        .unwrap();
    let c: Vec<i64> = r.take((0, "c")).unwrap();
    c.first().copied().unwrap_or(0)
}

/// **Devolver dos veces la misma línea no puede sacar la plata dos veces.**
///
/// El guard acumulado (`invariants::refund_exceeds_sold` +
/// `repo::sum_prior_refunds_by_product`) ya rechaza la segunda devolución por
/// cantidad. Desde la 0049 esa misma barrera es la que impide el segundo
/// retiro: el `cash_movement` se emite DENTRO de la transacción de la
/// devolución, así que si la devolución no ocurre, el retiro tampoco.
///
/// Detecta el retiro emitido fuera de la transacción, y detecta un guard que
/// deje pasar la segunda: en los dos casos salen 10000 de un cajón al que le
/// entraron 5000.
#[tokio::test]
async fn devolver_dos_veces_la_misma_linea_no_saca_la_plata_dos_veces() {
    let (db, tenant, user) = setup().await;
    let s = abrir_caja(&db, &tenant, &user, "0").await;
    let p = catalog::create_product(&db, &tenant, producto("Palta kilo", "5000", 50))
        .await
        .unwrap();
    let v = sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta(&p.id, &p.name, "5000", 1, "pos_cash", Some("5000"), None),
    )
    .await
    .unwrap();

    let devolver = || smodel::NewDevolucion {
        order: Some(v.order.id.clone()),
        tipo: "venta".into(),
        motivo: "pasada".into(),
        notas: None,
        items: vec![smodel::NewDevolucionItem {
            product: Some(p.id.clone()),
            product_name: p.name.clone(),
            quantity: 1,
            unit_price: dec("5000"),
            restock: true,
        }],
        metodo_reembolso: Some("efectivo".into()),
    };

    sales::create_refund(&db, &tenant, Some(&user), devolver())
        .await
        .unwrap();
    let err = sales::create_refund(&db, &tenant, Some(&user), devolver())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT", "la segunda se rechaza");

    let live = caja::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(
        live.movements_out,
        dec("5000"),
        "un solo retiro: la devolución rechazada no movió plata"
    );
    assert_eq!(retiros(&db, &tenant, &s.id).await, 1, "un solo movimiento");
}

/// **Una devolución en efectivo sin caja abierta se rechaza, no se pierde.**
///
/// Era la decisión que quedaba abierta. La plata sale igual del bolsillo del
/// que atiende; sin sesión no hay dónde asentarla, y un reembolso sin asiento
/// es exactamente el agujero que este carril cierra. Se rechaza diciendo qué
/// hacer.
///
/// La devolución con TARJETA sí procede sin caja: esa plata vuelve por el
/// procesador y nunca tocó el cajón.
#[tokio::test]
async fn devolucion_en_efectivo_sin_caja_abierta_se_rechaza_con_motivo() {
    let (db, tenant, user) = setup().await;
    let p = catalog::create_product(&db, &tenant, producto("Palta kilo", "5000", 50))
        .await
        .unwrap();
    let v = sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta(&p.id, &p.name, "5000", 2, "pos_cash", Some("10000"), None),
    )
    .await
    .unwrap();
    let dev = |metodo: &str| smodel::NewDevolucion {
        order: Some(v.order.id.clone()),
        tipo: "venta".into(),
        motivo: "pasada".into(),
        notas: None,
        items: vec![smodel::NewDevolucionItem {
            product: Some(p.id.clone()),
            product_name: p.name.clone(),
            quantity: 1,
            unit_price: dec("5000"),
            restock: true,
        }],
        metodo_reembolso: Some(metodo.into()),
    };
    let err = sales::create_refund(&db, &tenant, Some(&user), dev("efectivo"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
    assert!(
        err.to_string().contains("caja"),
        "el mensaje tiene que decir qué hacer, no sólo que no se pudo: {err}"
    );
    // La mercadería tampoco volvió: se rechazó la operación entera.
    let stock: Vec<i64> = db
        .query("SELECT VALUE stock FROM product WHERE tenant=$t")
        .bind(("t", tenant.clone()))
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert_eq!(stock[0], 48, "sin caja no se devolvió nada, ni la palta");

    // Con tarjeta procede: el cajón no tiene nada que ver.
    sales::create_refund(&db, &tenant, Some(&user), dev("tarjeta"))
        .await
        .unwrap();
}

/// **Los reportes por ítem descuentan las unidades devueltas, no la venta
/// entera.**
///
/// Antes de la 0049 `apply_refund` marcaba la orden `refunded` con cualquier
/// devolución parcial, y los tres reportes filtran `status NOT IN
/// ['refunded','cancelled']`: devolver 1 kilo de 3 borraba los 3 del ranking.
/// Ahora la orden se queda, así que hay que descontar las unidades que
/// volvieron — ni las 3 ni 0.
///
/// Falla en las dos direcciones: sin descontar da 3, filtrando la orden entera
/// da 0 (o el producto ni aparece).
#[tokio::test]
async fn los_reportes_por_item_descuentan_solo_lo_devuelto() {
    let (db, tenant, user) = setup().await;
    abrir_caja(&db, &tenant, &user, "0").await;
    let p = catalog::create_product(&db, &tenant, producto("Palta kilo", "5000", 50))
        .await
        .unwrap();
    let v = sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta(&p.id, &p.name, "5000", 3, "pos_cash", Some("15000"), None),
    )
    .await
    .unwrap();
    sales::create_refund(
        &db,
        &tenant,
        Some(&user),
        smodel::NewDevolucion {
            order: Some(v.order.id.clone()),
            tipo: "venta".into(),
            motivo: "una estaba pasada".into(),
            notas: None,
            items: vec![smodel::NewDevolucionItem {
                product: Some(p.id.clone()),
                product_name: p.name.clone(),
                quantity: 1,
                unit_price: dec("5000"),
                restock: true,
            }],
            metodo_reembolso: Some("efectivo".into()),
        },
    )
    .await
    .unwrap();

    let top = domain::expenses::service::top_products(
        &db,
        &tenant,
        domain::expenses::model::TopProductsFilters::default(),
    )
    .await
    .unwrap();
    assert_eq!(top.len(), 1, "el producto sigue en el ranking");
    assert_eq!(top[0].qty_sold, 2, "se vendieron 3 y volvió 1: quedan 2");
    assert_eq!(top[0].revenue, dec("10000"), "el ingreso se prorratea");

    let margenes = domain::expenses::service::margins_daily(
        &db,
        &tenant,
        domain::expenses::model::SalesReportFilters::default(),
    )
    .await
    .unwrap();
    let revenue: Decimal = margenes.iter().map(|m| m.revenue).sum();
    let cost: Decimal = margenes.iter().map(|m| m.cost).sum();
    assert_eq!(revenue, dec("10000"), "margen sobre lo que quedó vendido");
    assert_eq!(cost, dec("200"), "2 unidades a costo 100, no 3");

    let rot = domain::expenses::service::stock_rotation(
        &db,
        &tenant,
        domain::expenses::model::SalesReportFilters::default(),
    )
    .await
    .unwrap();
    assert_eq!(rot.len(), 1);
    assert_eq!(
        rot[0].qty_sold, 2,
        "la unidad que volvió a la góndola no rotó"
    );
}

/// **Una venta en efectivo le suma a TODAS las cajas abiertas del tenant.**
///
/// El evento `cash_sales_running_maint` (0030 → 0046 → 0049) aplica su delta
/// con `WHERE tenant = $tenant AND status = 'open' AND opened_at <= $created`:
/// sin filtro por cajero ni por sucursal. El chequeo de "una sola caja abierta"
/// de `open_session` es POR USUARIO (`... AND user=$u`), así que dos cajeros
/// con caja abierta son un estado alcanzable desde la API pública, y esa única
/// venta de $5.000 le entra a las dos.
///
/// `#[ignore]` a propósito, y no borrado: el arreglo es scopear el evento por
/// caja/sucursal, territorio del carril de sucursales — moverlo desde acá
/// pisaría trabajo ajeno. Queda corriendo con `cargo test -- --ignored` para
/// que el defecto no se pierda en la próxima ronda. Hoy FALLA, y eso es lo que
/// documenta.
#[tokio::test]
#[ignore = "defecto conocido: el evento no filtra por caja. Arreglo en el carril de sucursales"]
async fn una_venta_no_le_puede_sumar_a_dos_cajas_a_la_vez() {
    let (db, tenant, user_a) = setup().await;
    let user_b: Thing = db
        .query("CREATE user SET tenant=$t, email='b@t.l', password='x', roles=['admin'] RETURN id")
        .bind(("t", tenant.clone()))
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    let a = abrir_caja(&db, &tenant, &user_a, "0").await;
    let b = abrir_caja(&db, &tenant, &user_b, "0").await;
    let p = catalog::create_product(&db, &tenant, producto("Palta kilo", "5000", 50))
        .await
        .unwrap();
    // UNA venta, cobrada por el cajero A.
    sales::post_sale(
        &db,
        &tenant,
        Some(&user_a),
        Some("admin"),
        None,
        venta(&p.id, &p.name, "5000", 1, "pos_cash", Some("5000"), None),
    )
    .await
    .unwrap();

    let ca = caja::arqueo(&db, &tenant, &a.id).await.unwrap().cash_sales;
    let cb = caja::arqueo(&db, &tenant, &b.id).await.unwrap().cash_sales;
    assert_eq!(
        ca + cb,
        dec("5000"),
        "entraron 5000 al negocio, no 10000: la venta es de una sola caja"
    );
}

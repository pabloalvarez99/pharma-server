//! LOTES / FEFO por sucursal (migración 0042).
//!
//! La V2 (0041) hizo el stock por sucursal un ledger de CANTIDADES: la sucursal
//! no podía vender más de lo suyo, pero la elección de LOTE seguía siendo
//! global — vender en A podía descontar el frasco que está en B. Estos tests
//! sellan que el lote tiene domicilio:
//!
//! 1. **FEFO acotado**: vender en A consume el lote de A que vence primero,
//!    NUNCA uno de B, aunque el de B venza antes.
//! 2. **Aislamiento de lotes**: los lotes de B quedan intactos tras vender en A.
//! 3. **Recepción por sucursal**: recibir en B sube sólo el stock de B, y el
//!    lote nace en B.
//! 4. **Mismo `batch_code` en dos locales** = dos lotes físicos distintos.
//! 5. **Invariante 0042**: `Σ product_batch.stock[X] == product_branch_stock[X]`.
//! 6. **Transferencia mueve el lote**, no sólo la cantidad (si no, el destino
//!    tendría stock invendible por falta de lotes).

use chrono::{Duration, Utc};
use domain::catalog::{model::*, service as catalog};
use domain::inventory::{model as imodel, service as inventory};
use domain::purchasing::{model as pmodel, service as purchasing};
use domain::sales::{model as smodel, service as sales};
use domain::stock::{model as stock_model, service as stock};
use rust_decimal::Decimal;
use std::str::FromStr;
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

/// Producto SIN stock inicial: en estos tests el stock entra siempre por lotes,
/// así `product.stock` y `Σ batch.stock` arrancan alineados.
fn np(name: &str) -> NewProduct {
    NewProduct {
        name: name.into(),
        slug: None,
        description: None,
        price: dec("1000"),
        cost_price: Some(dec("400")),
        stock: 0,
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

async fn new_branch(db: &Db, tenant: &Thing, name: &str) -> String {
    let b = domain::branches::service::create_branch(
        db,
        tenant,
        domain::branches::model::NewBranch {
            name: name.into(),
            code: None,
            address: None,
            comuna: None,
            phone: None,
        },
    )
    .await
    .expect("crear sucursal");
    b.id
}

/// Crea un lote de `qty` unidades que vence en `days_ahead` días, en `branch`.
async fn new_batch(
    db: &Db,
    tenant: &Thing,
    product: &str,
    code: &str,
    days_ahead: i64,
    qty: i64,
    branch: Option<&str>,
) -> imodel::BatchDto {
    inventory::create_batch(
        db,
        tenant,
        imodel::NewBatch {
            product: product.into(),
            branch: branch.map(str::to_string),
            batch_code: code.into(),
            expiry_date: Utc::now() + Duration::days(days_ahead),
            stock: qty,
            cost: Some(dec("400")),
            notes: None,
        },
        None,
    )
    .await
    .expect("crear lote")
}

fn sale_req(product: &str, name: &str, qty: i64, branch: Option<&str>) -> smodel::PosSaleRequest {
    smodel::PosSaleRequest {
        items: vec![smodel::PosSaleItem {
            product: product.into(),
            product_name: name.into(),
            quantity: qty,
            unit_price: dec("1000"),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("100000")),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
        branch: branch.map(str::to_string),
    }
}

/// Stock actual de UN lote puntual.
async fn batch_stock(db: &Db, tenant: &Thing, batch_id: &str) -> i64 {
    inventory::get_batch(db, tenant, batch_id)
        .await
        .expect("leer lote")
        .stock
}

/// On-hand del bucket de una sucursal (`None` = casa matriz).
async fn qty_at(db: &Db, tenant: &Thing, product: &str, branch: Option<&str>) -> i64 {
    stock::list_branch_stock(
        db,
        tenant,
        stock_model::BranchStockFilters {
            product: Some(product.into()),
            branch: Some(branch.unwrap_or("none").to_string()),
            non_zero: false,
        },
    )
    .await
    .expect("stock por sucursal")
    .first()
    .map(|r| r.stock)
    .unwrap_or(0)
}

/// Σ de los lotes de un producto EN una sucursal.
async fn batch_sum_at(db: &Db, tenant: &Thing, product: &str, branch: Option<&str>) -> i64 {
    inventory::list_batches(
        db,
        tenant,
        imodel::BatchFilters {
            product: Some(product.into()),
            branch: Some(branch.unwrap_or("none").to_string()),
            only_available: None,
            expiring_within_days: None,
            limit: Some(500),
            offset: None,
        },
    )
    .await
    .expect("listar lotes")
    .iter()
    .map(|b| b.stock)
    .sum()
}

/// El invariante que agrega 0042: por sucursal, la suma de los lotes es
/// exactamente el on-hand de esa sucursal.
async fn assert_lotes_cuadran(db: &Db, tenant: &Thing, product: &str, branch: Option<&str>) {
    assert_eq!(
        batch_sum_at(db, tenant, product, branch).await,
        qty_at(db, tenant, product, branch).await,
        "Σ product_batch.stock != product_branch_stock en {branch:?}",
    );
}

// --- 1. FEFO acotado a la sucursal -----------------------------------------

/// EL TEST DE LA LANE. La sucursal B tiene el lote que vence ANTES. Vender en A
/// debe consumir el lote de A igual — el frasco que el cajero de A tiene en la
/// mano — y no tocar el de B.
#[tokio::test]
async fn fefo_no_cruza_de_sucursal_aunque_la_otra_venza_antes() {
    let (db, tenant, _admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Amoxicilina 500mg"))
        .await
        .unwrap();
    let sucursal_b = new_branch(&db, &tenant, "Local Centro").await;

    // Casa matriz (A): vence en 90 días. Sucursal B: vence en 10 (mucho antes).
    let lote_a = new_batch(&db, &tenant, &p.id, "L-CASA", 90, 20, None).await;
    let lote_b = new_batch(&db, &tenant, &p.id, "L-CENTRO", 10, 20, Some(&sucursal_b)).await;

    sales::post_sale(
        &db,
        &tenant,
        None,
        None,
        None,
        sale_req(&p.id, "Amoxicilina 500mg", 5, None),
    )
    .await
    .expect("venta en casa matriz");

    // El lote de casa matriz bajó; el de la sucursal B quedó INTACTO pese a
    // vencer antes. Sin el filtro por sucursal, FEFO global habría comido de B.
    assert_eq!(batch_stock(&db, &tenant, &lote_a.id).await, 15);
    assert_eq!(batch_stock(&db, &tenant, &lote_b.id).await, 20);

    assert_lotes_cuadran(&db, &tenant, &p.id, None).await;
    assert_lotes_cuadran(&db, &tenant, &p.id, Some(&sucursal_b)).await;
}

/// Dentro de UNA sucursal el orden FEFO sigue mandando: se consume primero el
/// que vence antes, y recién al agotarlo se pasa al siguiente.
#[tokio::test]
async fn fefo_ordena_por_vencimiento_dentro_de_la_sucursal() {
    let (db, tenant, _admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Ibuprofeno"))
        .await
        .unwrap();
    let sucursal_b = new_branch(&db, &tenant, "Local Centro").await;

    // Los dos lotes viven en B. El "viejo" vence antes → se consume primero.
    let viejo = new_batch(&db, &tenant, &p.id, "L-VIEJO", 15, 10, Some(&sucursal_b)).await;
    let nuevo = new_batch(&db, &tenant, &p.id, "L-NUEVO", 120, 10, Some(&sucursal_b)).await;

    // 12 unidades: agota el viejo (10) y toma 2 del nuevo.
    sales::post_sale(
        &db,
        &tenant,
        None,
        None,
        None,
        sale_req(&p.id, "Ibuprofeno", 12, Some(&sucursal_b)),
    )
    .await
    .expect("venta en sucursal B");

    assert_eq!(batch_stock(&db, &tenant, &viejo.id).await, 0);
    assert_eq!(batch_stock(&db, &tenant, &nuevo.id).await, 8);
    assert_lotes_cuadran(&db, &tenant, &p.id, Some(&sucursal_b)).await;
}

/// Una sucursal NO puede vender contra lotes de otra: aunque el negocio entero
/// tenga unidades de sobra, si el local no tiene lote propio la venta se
/// rechaza. Es el mismo aislamiento de 0041, ahora a nivel de lote.
#[tokio::test]
async fn no_vende_en_una_sucursal_los_lotes_de_otra() {
    let (db, tenant, _admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Paracetamol"))
        .await
        .unwrap();
    let sucursal_b = new_branch(&db, &tenant, "Local Centro").await;

    // TODO el stock (y el único lote) está en casa matriz.
    new_batch(&db, &tenant, &p.id, "L-CASA", 60, 50, None).await;

    let err = sales::post_sale(
        &db,
        &tenant,
        None,
        None,
        None,
        sale_req(&p.id, "Paracetamol", 1, Some(&sucursal_b)),
    )
    .await
    .expect_err("vender en B sin stock en B debe fallar");

    assert!(
        matches!(err, domain::errors::DomainError::InsufficientStock),
        "esperaba InsufficientStock, recibí {err:?}",
    );
    // Y el lote de casa matriz quedó intacto: el intento no tocó nada.
    assert_eq!(qty_at(&db, &tenant, &p.id, None).await, 50);
}

// --- 2. lotes homónimos en dos locales -------------------------------------

/// El mismo `batch_code` del proveedor en dos locales son DOS lotes físicos.
/// Colapsarlos en una fila pondría en un solo bucket cajas que están en dos
/// lugares distintos.
#[tokio::test]
async fn mismo_codigo_en_dos_locales_son_dos_lotes() {
    let (db, tenant, _admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Suero"))
        .await
        .unwrap();
    let sucursal_b = new_branch(&db, &tenant, "Local Centro").await;

    let en_casa = new_batch(&db, &tenant, &p.id, "L-777", 60, 10, None).await;
    let en_b = new_batch(&db, &tenant, &p.id, "L-777", 60, 7, Some(&sucursal_b)).await;
    assert_ne!(en_casa.id, en_b.id, "deben ser filas distintas");

    // Vender en casa matriz consume sólo su propia fila.
    sales::post_sale(
        &db,
        &tenant,
        None,
        None,
        None,
        sale_req(&p.id, "Suero", 4, None),
    )
    .await
    .expect("venta casa matriz");

    assert_eq!(batch_stock(&db, &tenant, &en_casa.id).await, 6);
    assert_eq!(batch_stock(&db, &tenant, &en_b.id).await, 7);
    assert_lotes_cuadran(&db, &tenant, &p.id, None).await;
    assert_lotes_cuadran(&db, &tenant, &p.id, Some(&sucursal_b)).await;
}

// --- 3. recepción de mercadería por sucursal --------------------------------

/// Recibir una OC de la sucursal B sube SÓLO el stock de B, y el lote creado
/// queda domiciliado en B (listo para el FEFO de ese local).
#[tokio::test]
async fn recepcion_apunta_a_una_sola_sucursal() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Alcohol gel"))
        .await
        .unwrap();
    let sucursal_b = new_branch(&db, &tenant, "Local Centro").await;

    let sup = purchasing::create_supplier(
        &db,
        &tenant,
        pmodel::NewSupplier {
            name: "Droguería Sur".into(),
            rut: None,
            contact_name: None,
            contact_email: None,
            contact_phone: None,
            default_invoice_format: None,
        },
    )
    .await
    .expect("crear proveedor");

    // La OC se emite PARA la sucursal B.
    let po = purchasing::create_purchase_order(
        &db,
        &tenant,
        pmodel::NewPurchaseOrder {
            supplier: sup.id.clone(),
            branch: Some(sucursal_b.clone()),
            currency: None,
            notes: None,
            external_ref: None,
            items: vec![pmodel::NewPurchaseOrderItem {
                product: Some(p.id.clone()),
                product_name: "Alcohol gel".into(),
                quantity: 30,
                unit_cost: dec("400"),
            }],
        },
    )
    .await
    .expect("crear OC");
    assert_eq!(po.branch.as_deref(), Some(sucursal_b.as_str()));

    purchasing::send_purchase_order(&db, &tenant, &po.id)
        .await
        .expect("enviar OC");

    let line_id = po.items[0].id.clone();
    purchasing::receive_purchase_order_lines(
        &db,
        &tenant,
        &po.id,
        pmodel::ReceivePurchaseOrder {
            lines: vec![pmodel::ReceivePurchaseOrderLine {
                po_line_id: line_id,
                qty_received: 30,
                lot: Some("L-RECIBIDO".into()),
                expiry_date: Some(Utc::now() + Duration::days(200)),
            }],
            notes: None,
        },
        Some(&admin.to_string()),
    )
    .await
    .expect("recibir mercadería");

    // Todo entró a B; la casa matriz sigue en cero.
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&sucursal_b)).await, 30);
    assert_eq!(qty_at(&db, &tenant, &p.id, None).await, 0);

    // Y el lote quedó en B, no en casa matriz.
    assert_eq!(
        batch_sum_at(&db, &tenant, &p.id, Some(&sucursal_b)).await,
        30
    );
    assert_eq!(batch_sum_at(&db, &tenant, &p.id, None).await, 0);
    assert_lotes_cuadran(&db, &tenant, &p.id, Some(&sucursal_b)).await;

    // El local que recibió puede vender de inmediato contra ese lote.
    sales::post_sale(
        &db,
        &tenant,
        None,
        None,
        None,
        sale_req(&p.id, "Alcohol gel", 3, Some(&sucursal_b)),
    )
    .await
    .expect("vender lo recién recibido");
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&sucursal_b)).await, 27);
    assert_lotes_cuadran(&db, &tenant, &p.id, Some(&sucursal_b)).await;
}

// --- 4. transferencia mueve el lote ----------------------------------------

/// Transferir no puede mover sólo la cantidad: si el lote se queda en el
/// origen, el destino ve stock que su FEFO no puede consumir (dead-end). La
/// transferencia mueve el lote y el destino queda habilitado para vender.
#[tokio::test]
async fn transferencia_mueve_el_lote_y_el_destino_puede_vender() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Jeringas"))
        .await
        .unwrap();
    let sucursal_b = new_branch(&db, &tenant, "Local Centro").await;

    let lote = new_batch(&db, &tenant, &p.id, "L-JER", 45, 40, None).await;

    stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None,
            to_branch: Some(sucursal_b.clone()),
            qty: 15,
            notes: None,
        },
    )
    .await
    .expect("transferir a la sucursal B");

    // Cantidades repartidas...
    assert_eq!(qty_at(&db, &tenant, &p.id, None).await, 25);
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&sucursal_b)).await, 15);
    // ...y los LOTES acompañaron el movimiento en ambos lados.
    assert_eq!(batch_stock(&db, &tenant, &lote.id).await, 25);
    assert_lotes_cuadran(&db, &tenant, &p.id, None).await;
    assert_lotes_cuadran(&db, &tenant, &p.id, Some(&sucursal_b)).await;

    // El destino puede vender: tiene lote propio, no sólo un número.
    sales::post_sale(
        &db,
        &tenant,
        None,
        None,
        None,
        sale_req(&p.id, "Jeringas", 15, Some(&sucursal_b)),
    )
    .await
    .expect("el destino vende contra el lote transferido");
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&sucursal_b)).await, 0);
    assert_lotes_cuadran(&db, &tenant, &p.id, Some(&sucursal_b)).await;
}

/// La transferencia elige por FEFO qué lote mandar: se rota primero lo que
/// vence antes.
#[tokio::test]
async fn transferencia_manda_el_lote_que_vence_primero() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Gasas"))
        .await
        .unwrap();
    let sucursal_b = new_branch(&db, &tenant, "Local Centro").await;

    let pronto = new_batch(&db, &tenant, &p.id, "L-PRONTO", 12, 10, None).await;
    let lejano = new_batch(&db, &tenant, &p.id, "L-LEJANO", 300, 10, None).await;

    stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None,
            to_branch: Some(sucursal_b.clone()),
            qty: 10,
            notes: None,
        },
    )
    .await
    .expect("transferir");

    // Viajó el que vence primero; el lejano no se tocó.
    assert_eq!(batch_stock(&db, &tenant, &pronto.id).await, 0);
    assert_eq!(batch_stock(&db, &tenant, &lejano.id).await, 10);
    assert_lotes_cuadran(&db, &tenant, &p.id, None).await;
    assert_lotes_cuadran(&db, &tenant, &p.id, Some(&sucursal_b)).await;
}

// --- 5. compatibilidad hacia atrás -----------------------------------------

/// Un negocio de UN solo local (todo en casa matriz, sin `branch` en ninguna
/// request) se comporta igual que antes de 0042: el FEFO ordena por
/// vencimiento y consume normal. Es el camino de todo instalado hoy.
#[tokio::test]
async fn negocio_de_un_solo_local_no_cambia() {
    let (db, tenant, _admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Vendas"))
        .await
        .unwrap();

    let viejo = new_batch(&db, &tenant, &p.id, "L-A", 20, 6, None).await;
    let nuevo = new_batch(&db, &tenant, &p.id, "L-B", 200, 6, None).await;

    sales::post_sale(
        &db,
        &tenant,
        None,
        None,
        None,
        sale_req(&p.id, "Vendas", 8, None),
    )
    .await
    .expect("venta sin sucursal");

    assert_eq!(batch_stock(&db, &tenant, &viejo.id).await, 0);
    assert_eq!(batch_stock(&db, &tenant, &nuevo.id).await, 4);
    assert_lotes_cuadran(&db, &tenant, &p.id, None).await;
}

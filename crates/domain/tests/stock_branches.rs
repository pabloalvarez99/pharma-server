//! Stock OPERATIVO por sucursal + transferencias (V2, migración 0041).
//!
//! Lo que estos tests defienden, en orden de importancia:
//!
//! 1. **Aislamiento**: vender en la sucursal A no toca el stock de la B, y no
//!    se puede vender en A lo que está físicamente en B (aunque el stock global
//!    alcance de sobra).
//! 2. **Conservación**: una transferencia mueve la distribución, nunca el total
//!    (`Σ product_branch_stock == product.stock` antes y después).
//! 3. **No sobre-girar**: no se puede transferir más de lo que la sucursal
//!    origen tiene, aunque el negocio entero lo tenga.
//! 4. **Concurrencia (BUG-003/004)**: ventas paralelas y ventas contra
//!    transferencias siguen serializadas por el lock por tenant — ni contadores
//!    corruptos ni sobre-venta.

use domain::catalog::{model::*, service as catalog};
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

fn np(name: &str, stock: i64) -> NewProduct {
    NewProduct {
        name: name.into(),
        slug: None,
        description: None,
        price: dec("1000"),
        cost_price: Some(dec("400")),
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

/// Crea una sucursal y devuelve su record id como string (`branch:<key>`).
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

/// Venta de contado de `qty` unidades del producto en `branch`.
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

/// On-hand de un producto en una sucursal (`None` = casa matriz).
async fn qty_at(db: &Db, tenant: &Thing, product: &str, branch: Option<&str>) -> i64 {
    let rows = stock::list_branch_stock(
        db,
        tenant,
        stock_model::BranchStockFilters {
            product: Some(product.into()),
            branch: Some(branch.unwrap_or("none").to_string()),
            non_zero: false,
        },
    )
    .await
    .expect("listar stock por sucursal");
    rows.first().map(|r| r.stock).unwrap_or(0)
}

/// `product.stock` global (el total del negocio).
async fn global_stock(db: &Db, tenant: &Thing, product: &str) -> i64 {
    let pid = surrealdb::sql::thing(product).unwrap();
    let mut r = db
        .query("SELECT stock FROM product WHERE id = $p AND tenant = $t")
        .bind(("p", pid))
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let s: Option<i64> = r.take((0, "stock")).unwrap();
    s.unwrap_or(0)
}

/// Σ del stock por sucursal de un producto — debe ser SIEMPRE `product.stock`.
async fn sum_branches(db: &Db, tenant: &Thing, product: &str) -> i64 {
    stock::list_branch_stock(
        db,
        tenant,
        stock_model::BranchStockFilters {
            product: Some(product.into()),
            branch: None,
            non_zero: false,
        },
    )
    .await
    .expect("listar stock por sucursal")
    .iter()
    .map(|r| r.stock)
    .sum()
}

// --- backfill / seed -------------------------------------------------------

/// Un producto creado con stock inicial arranca todo en la casa matriz, y el
/// invariante V2 vale desde el minuto cero.
#[tokio::test]
async fn producto_nuevo_arranca_en_casa_matriz() {
    let (db, tenant, _admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Arroz 1kg", 40))
        .await
        .unwrap();

    assert_eq!(qty_at(&db, &tenant, &p.id, None).await, 40);
    assert_eq!(global_stock(&db, &tenant, &p.id).await, 40);
    assert_eq!(sum_branches(&db, &tenant, &p.id).await, 40);
}

// --- transferencia ---------------------------------------------------------

/// La transferencia mueve la distribución y CONSERVA el total.
#[tokio::test]
async fn transferencia_conserva_el_total() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Fideos", 100))
        .await
        .unwrap();
    let sucursal_b = new_branch(&db, &tenant, "Local Centro").await;

    let total_antes = global_stock(&db, &tenant, &p.id).await;
    assert_eq!(sum_branches(&db, &tenant, &p.id).await, total_antes);

    let res = stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None, // casa matriz
            to_branch: Some(sucursal_b.clone()),
            qty: 30,
            notes: Some("reposición al centro".into()),
        },
    )
    .await
    .expect("transferencia");

    assert_eq!(res.from_stock, 70);
    assert_eq!(res.to_stock, 30);
    assert_eq!(qty_at(&db, &tenant, &p.id, None).await, 70);
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&sucursal_b)).await, 30);

    // Lo que importa: el negocio sigue teniendo lo mismo, sólo cambió DÓNDE.
    assert_eq!(global_stock(&db, &tenant, &p.id).await, total_antes);
    assert_eq!(sum_branches(&db, &tenant, &p.id).await, total_antes);
}

/// Los dos movimientos de auditoría existen y son de suma cero.
#[tokio::test]
async fn transferencia_deja_auditoria_de_suma_cero() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Aceite", 50))
        .await
        .unwrap();
    let b = new_branch(&db, &tenant, "Local 2").await;

    let res = stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None,
            to_branch: Some(b.clone()),
            qty: 12,
            notes: None,
        },
    )
    .await
    .unwrap();
    assert!(res.movement_out.starts_with("stock_movement:"));
    assert!(res.movement_in.starts_with("stock_movement:"));

    #[derive(serde::Deserialize)]
    struct Mov {
        delta: i64,
        reason: String,
    }
    let mut r = db
        .query(
            "SELECT delta, reason FROM stock_movement \
             WHERE tenant = $t AND reason IN ['transfer_out', 'transfer_in']",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let movs: Vec<Mov> = r.take(0).unwrap();
    assert_eq!(movs.len(), 2, "una transferencia = dos movimientos");
    assert_eq!(movs.iter().map(|m| m.delta).sum::<i64>(), 0);
    assert!(movs
        .iter()
        .any(|m| m.reason == "transfer_out" && m.delta == -12));
    assert!(movs
        .iter()
        .any(|m| m.reason == "transfer_in" && m.delta == 12));
}

/// No se puede transferir más de lo que la sucursal origen tiene, aunque el
/// negocio entero lo tenga de sobra.
#[tokio::test]
async fn no_se_puede_transferir_mas_de_lo_que_hay_en_el_origen() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Detergente", 100))
        .await
        .unwrap();
    let b = new_branch(&db, &tenant, "Local 2").await;

    // El local 2 tiene 0: no puede mandar nada a la casa matriz aunque el
    // negocio tenga 100 unidades.
    let err = stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: Some(b.clone()),
            to_branch: None,
            qty: 1,
            notes: None,
        },
    )
    .await
    .expect_err("debe rechazar: el origen no tiene stock");
    assert!(matches!(err, domain::DomainError::InsufficientStock));

    // Y nada se movió.
    assert_eq!(qty_at(&db, &tenant, &p.id, None).await, 100);
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&b)).await, 0);
    assert_eq!(sum_branches(&db, &tenant, &p.id).await, 100);
}

/// Origen == destino no es una transferencia.
#[tokio::test]
async fn transferencia_al_mismo_local_se_rechaza() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Azúcar", 10))
        .await
        .unwrap();
    let b = new_branch(&db, &tenant, "Local 2").await;

    for (from, to) in [(None, None), (Some(b.clone()), Some(b.clone()))] {
        let err = stock::transfer(
            &db,
            &tenant,
            Some(&admin.to_string()),
            stock_model::TransferInput {
                product: p.id.clone(),
                from_branch: from,
                to_branch: to,
                qty: 1,
                notes: None,
            },
        )
        .await
        .expect_err("origen y destino iguales");
        assert!(matches!(err, domain::DomainError::Invalid(_)));
    }
}

/// Una sucursal de otro tenant no existe para este tenant.
#[tokio::test]
async fn no_se_puede_transferir_a_sucursal_de_otro_tenant() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Sal", 10))
        .await
        .unwrap();
    let mut r = db
        .query("CREATE tenant SET name = 'Otro', slug = 'otro' RETURN id")
        .await
        .unwrap();
    let otro: Option<Thing> = r.take((0, "id")).unwrap();
    let otro = otro.unwrap();
    let ajena = new_branch(&db, &otro, "Local Ajeno").await;

    let err = stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None,
            to_branch: Some(ajena),
            qty: 1,
            notes: None,
        },
    )
    .await
    .expect_err("sucursal de otro tenant");
    assert!(matches!(err, domain::DomainError::Invalid(_)));
}

// --- aislamiento de la venta ----------------------------------------------

/// La venta descuenta SÓLO de su sucursal. El otro local queda intacto.
#[tokio::test]
async fn venta_en_una_sucursal_no_toca_la_otra() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Shampoo", 100))
        .await
        .unwrap();
    let a = new_branch(&db, &tenant, "Local A").await;
    let b = new_branch(&db, &tenant, "Local B").await;

    // Repartir: 40 al A, 25 al B, resto en casa matriz.
    for (dest, qty) in [(&a, 40), (&b, 25)] {
        stock::transfer(
            &db,
            &tenant,
            Some(&admin.to_string()),
            stock_model::TransferInput {
                product: p.id.clone(),
                from_branch: None,
                to_branch: Some(dest.clone()),
                qty,
                notes: None,
            },
        )
        .await
        .unwrap();
    }
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&a)).await, 40);
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&b)).await, 25);

    // Vender 10 en el A.
    sales::post_sale(
        &db,
        &tenant,
        Some(&admin),
        Some("admin"),
        None,
        sale_req(&p.id, "Shampoo", 10, Some(&a)),
    )
    .await
    .expect("venta en A");

    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&a)).await, 30, "A baja 10");
    assert_eq!(
        qty_at(&db, &tenant, &p.id, Some(&b)).await,
        25,
        "B no se entera de la venta del A"
    );
    assert_eq!(
        qty_at(&db, &tenant, &p.id, None).await,
        35,
        "casa matriz igual"
    );
    assert_eq!(global_stock(&db, &tenant, &p.id).await, 90);
    assert_eq!(sum_branches(&db, &tenant, &p.id).await, 90);
}

/// El chequeo que hace que "multi-sucursal" signifique algo: NO se puede vender
/// en un local lo que está en otro, aunque el stock global alcance.
#[tokio::test]
async fn no_se_puede_vender_en_un_local_el_stock_de_otro() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Jabón", 100))
        .await
        .unwrap();
    let a = new_branch(&db, &tenant, "Local A").await;

    // Todo el stock quedó en la casa matriz; el local A tiene 0.
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&a)).await, 0);
    assert_eq!(global_stock(&db, &tenant, &p.id).await, 100);

    let err = sales::post_sale(
        &db,
        &tenant,
        Some(&admin),
        Some("admin"),
        None,
        sale_req(&p.id, "Jabón", 1, Some(&a)),
    )
    .await
    .expect_err("el local A no tiene stock aunque el negocio sí");
    assert!(matches!(err, domain::DomainError::InsufficientStock));

    // Nada cambió en ningún lado.
    assert_eq!(global_stock(&db, &tenant, &p.id).await, 100);
    assert_eq!(sum_branches(&db, &tenant, &p.id).await, 100);
}

/// Dos líneas del mismo producto en la misma venta se suman contra el saldo de
/// la sucursal (no se chequea línea por línea).
#[tokio::test]
async fn lineas_repetidas_no_burlan_el_saldo_de_la_sucursal() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Café", 100))
        .await
        .unwrap();
    let a = new_branch(&db, &tenant, "Local A").await;
    stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None,
            to_branch: Some(a.clone()),
            qty: 5,
            notes: None,
        },
    )
    .await
    .unwrap();

    let mut req = sale_req(&p.id, "Café", 3, Some(&a));
    req.items.push(smodel::PosSaleItem {
        product: p.id.clone(),
        product_name: "Café".into(),
        quantity: 3,
        unit_price: dec("1000"),
    });

    let err = sales::post_sale(&db, &tenant, Some(&admin), Some("admin"), None, req)
        .await
        .expect_err("3 + 3 > 5 en el local A");
    assert!(matches!(err, domain::DomainError::InsufficientStock));
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&a)).await, 5);
}

/// Sin sucursal en el request y sin caja abierta, la venta cae a la casa matriz
/// — un negocio de un solo local se comporta igual que antes de V2.
#[tokio::test]
async fn venta_sin_sucursal_descuenta_de_casa_matriz() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Pan", 20))
        .await
        .unwrap();

    sales::post_sale(
        &db,
        &tenant,
        Some(&admin),
        Some("admin"),
        None,
        sale_req(&p.id, "Pan", 5, None),
    )
    .await
    .expect("venta sin sucursal");

    assert_eq!(qty_at(&db, &tenant, &p.id, None).await, 15);
    assert_eq!(global_stock(&db, &tenant, &p.id).await, 15);
    assert_eq!(sum_branches(&db, &tenant, &p.id).await, 15);
}

/// La venta hereda la sucursal de la caja abierta del cajero sin que el POS
/// mande nada: "la venta descuenta de la sucursal de la caja".
#[tokio::test]
async fn venta_hereda_la_sucursal_de_la_caja_abierta() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Leche", 60))
        .await
        .unwrap();
    let a = new_branch(&db, &tenant, "Local A").await;
    stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None,
            to_branch: Some(a.clone()),
            qty: 20,
            notes: None,
        },
    )
    .await
    .unwrap();

    // Caja abierta EN el local A (vía la caja física, que es lo que hace el POS).
    let caja = domain::branches::service::create_register(
        &db,
        &tenant,
        domain::branches::model::NewRegister {
            name: "Caja 1".into(),
            branch: Some(a.clone()),
            code: None,
        },
    )
    .await
    .unwrap();
    let sesion = domain::cash_register::service::open_session(
        &db,
        &tenant,
        &admin,
        domain::cash_register::model::OpenSessionInput {
            register_name: "caja-1".into(),
            register: Some(caja.id.clone()),
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .expect("abrir caja");
    assert_eq!(sesion.branch.as_deref(), Some(a.as_str()));

    // Venta SIN `branch` en el request.
    sales::post_sale(
        &db,
        &tenant,
        Some(&admin),
        Some("admin"),
        None,
        sale_req(&p.id, "Leche", 4, None),
    )
    .await
    .expect("venta con caja abierta en A");

    assert_eq!(
        qty_at(&db, &tenant, &p.id, Some(&a)).await,
        16,
        "descontó del local de la caja"
    );
    assert_eq!(
        qty_at(&db, &tenant, &p.id, None).await,
        40,
        "la casa matriz no se tocó"
    );
    assert_eq!(sum_branches(&db, &tenant, &p.id).await, 56);
}

// --- reporte ---------------------------------------------------------------

/// El reporte agrupa por producto y su total coincide con `product.stock`.
#[tokio::test]
async fn reporte_por_sucursal_cuadra_con_el_stock_global() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Yerba", 90))
        .await
        .unwrap();
    let a = new_branch(&db, &tenant, "Local A").await;
    stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None,
            to_branch: Some(a.clone()),
            qty: 30,
            notes: None,
        },
    )
    .await
    .unwrap();

    let rows = stock::branch_stock_report(&db, &tenant, stock_model::BranchStockFilters::default())
        .await
        .unwrap();
    let row = rows
        .iter()
        .find(|r| r.product == p.id)
        .expect("el producto está en el reporte");
    assert_eq!(row.product_name, "Yerba");
    assert_eq!(row.total, 90);
    assert_eq!(row.total, global_stock(&db, &tenant, &p.id).await);
    assert_eq!(row.by_branch.len(), 2, "casa matriz + local A");
    let en_a = row
        .by_branch
        .iter()
        .find(|s| s.branch.as_deref() == Some(a.as_str()))
        .unwrap();
    assert_eq!(en_a.stock, 30);
    assert_eq!(en_a.branch_name.as_deref(), Some("Local A"));
}

// --- servicios -------------------------------------------------------------

/// Un servicio no tiene inventario: no se transfiere ni entra al stock por
/// sucursal.
#[tokio::test]
async fn un_servicio_no_se_transfiere() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Corte de pelo", 0))
        .await
        .unwrap();
    let pid = surrealdb::sql::thing(&p.id).unwrap();
    domain::catalog::repo::set_physical_stock(&db, &tenant, &pid, false)
        .await
        .unwrap();
    let b = new_branch(&db, &tenant, "Local 2").await;

    let err = stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None,
            to_branch: Some(b),
            qty: 1,
            notes: None,
        },
    )
    .await
    .expect_err("un servicio no tiene stock que mover");
    assert!(matches!(err, domain::DomainError::Invalid(_)));
}

// --- concurrencia (BUG-003 / BUG-004 no debe reabrirse) --------------------

/// Ventas concurrentes EN LA MISMA sucursal: el lock por tenant las serializa;
/// el saldo del local baja exactamente lo vendido y nunca queda negativo.
#[tokio::test]
async fn ventas_concurrentes_en_la_misma_sucursal_no_corrompen_el_saldo() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Agua", 100))
        .await
        .unwrap();
    let a = new_branch(&db, &tenant, "Local A").await;
    stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None,
            to_branch: Some(a.clone()),
            qty: 60,
            notes: None,
        },
    )
    .await
    .unwrap();

    let mut tasks = Vec::new();
    for _ in 0..20 {
        let db = db.clone();
        let tenant = tenant.clone();
        let admin = admin.clone();
        let req = sale_req(&p.id, "Agua", 2, Some(&a));
        tasks.push(tokio::spawn(async move {
            sales::post_sale(&db, &tenant, Some(&admin), Some("admin"), None, req).await
        }));
    }
    let mut ok = 0;
    for t in tasks {
        if t.await.unwrap().is_ok() {
            ok += 1;
        }
    }
    assert_eq!(ok, 20, "las 20 ventas caben en las 60 unidades del local");
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&a)).await, 20);
    assert_eq!(qty_at(&db, &tenant, &p.id, None).await, 40);
    assert_eq!(global_stock(&db, &tenant, &p.id).await, 60);
    assert_eq!(sum_branches(&db, &tenant, &p.id).await, 60);
}

/// Ventas y transferencias concurrentes sobre el MISMO producto comparten el
/// lock por tenant: no hay conflicto write-write ni contadores corruptos, y el
/// invariante V2 sobrevive.
#[tokio::test]
async fn ventas_y_transferencias_concurrentes_conservan_el_invariante() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Harina", 200))
        .await
        .unwrap();
    let a = new_branch(&db, &tenant, "Local A").await;
    stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None,
            to_branch: Some(a.clone()),
            qty: 100,
            notes: None,
        },
    )
    .await
    .unwrap();

    let mut tasks = Vec::new();
    for i in 0..20 {
        let db = db.clone();
        let tenant = tenant.clone();
        let admin = admin.clone();
        let a = a.clone();
        let pid = p.id.clone();
        tasks.push(tokio::spawn(async move {
            if i % 2 == 0 {
                // Venta en el local A.
                sales::post_sale(
                    &db,
                    &tenant,
                    Some(&admin),
                    Some("admin"),
                    None,
                    sale_req(&pid, "Harina", 2, Some(&a)),
                )
                .await
                .map(|_| ())
            } else {
                // Transferencia casa matriz → local A, en paralelo.
                stock::transfer(
                    &db,
                    &tenant,
                    Some(&admin.to_string()),
                    stock_model::TransferInput {
                        product: pid,
                        from_branch: None,
                        to_branch: Some(a),
                        qty: 3,
                        notes: None,
                    },
                )
                .await
                .map(|_| ())
            }
        }));
    }
    for t in tasks {
        t.await
            .unwrap()
            .expect("ni ventas ni transferencias deben fallar");
    }

    // 10 ventas de 2 = 20 unidades salieron del negocio.
    let total = global_stock(&db, &tenant, &p.id).await;
    assert_eq!(total, 180, "sólo las ventas reducen el total");
    assert_eq!(sum_branches(&db, &tenant, &p.id).await, total);
    // Local A: 100 iniciales + 10 transferencias de 3 − 10 ventas de 2 = 110.
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&a)).await, 110);
    assert_eq!(qty_at(&db, &tenant, &p.id, None).await, 70);
}

// --- devolución ------------------------------------------------------------

/// La devolución repone en la sucursal donde se vendió, no en la casa matriz.
#[tokio::test]
async fn devolucion_repone_en_la_sucursal_de_la_venta() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Té", 50))
        .await
        .unwrap();
    let a = new_branch(&db, &tenant, "Local A").await;
    stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None,
            to_branch: Some(a.clone()),
            qty: 20,
            notes: None,
        },
    )
    .await
    .unwrap();

    let venta = sales::post_sale(
        &db,
        &tenant,
        Some(&admin),
        Some("admin"),
        None,
        sale_req(&p.id, "Té", 6, Some(&a)),
    )
    .await
    .unwrap();
    assert_eq!(qty_at(&db, &tenant, &p.id, Some(&a)).await, 14);

    sales::create_refund(
        &db,
        &tenant,
        Some(&admin),
        smodel::NewDevolucion {
            order: Some(venta.order.id.clone()),
            tipo: "parcial".into(),
            motivo: "cliente se arrepintió".into(),
            notas: None,
            metodo_reembolso: Some("efectivo".into()),
            items: vec![smodel::NewDevolucionItem {
                product: Some(p.id.clone()),
                product_name: "Té".into(),
                quantity: 2,
                unit_price: dec("1000"),
                restock: true,
            }],
        },
    )
    .await
    .expect("devolución");

    assert_eq!(
        qty_at(&db, &tenant, &p.id, Some(&a)).await,
        16,
        "vuelve al local donde se vendió"
    );
    assert_eq!(
        qty_at(&db, &tenant, &p.id, None).await,
        30,
        "la casa matriz no se infla con la devolución del local A"
    );
    assert_eq!(sum_branches(&db, &tenant, &p.id).await, 46);
    assert_eq!(global_stock(&db, &tenant, &p.id).await, 46);
}

// --- aislamiento multi-tenant ---------------------------------------------

/// El stock por sucursal de un tenant no se ve desde otro.
#[tokio::test]
async fn el_stock_por_sucursal_es_tenant_scoped() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, np("Galletas", 10))
        .await
        .unwrap();
    let a = new_branch(&db, &tenant, "Local A").await;
    stock::transfer(
        &db,
        &tenant,
        Some(&admin.to_string()),
        stock_model::TransferInput {
            product: p.id.clone(),
            from_branch: None,
            to_branch: Some(a),
            qty: 4,
            notes: None,
        },
    )
    .await
    .unwrap();

    let mut r = db
        .query("CREATE tenant SET name = 'Otro', slug = 'otro' RETURN id")
        .await
        .unwrap();
    let otro: Option<Thing> = r.take((0, "id")).unwrap();
    let otro = otro.unwrap();

    let filas = stock::list_branch_stock(&db, &otro, stock_model::BranchStockFilters::default())
        .await
        .unwrap();
    assert!(
        filas.is_empty(),
        "el otro tenant no ve nada del stock por sucursal ajeno"
    );
}

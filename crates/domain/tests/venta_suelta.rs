//! El centinela de los montos sueltos, punta a punta contra la base migrada.
//!
//! **El caso.** "Son $2.000, una bolsa." Es la venta más común de un puesto de
//! feria y no tiene producto detrás. El server no acepta líneas sin producto
//! (`PosSaleItem.product` es obligatorio), así que la app cuelga esas ventas de
//! un producto centinela: uno solo por negocio, con el monto en `unit_price`.
//!
//! Hasta la migración 0031 + este campo, ese centinela tenía que nacer con un
//! colchón de stock enorme y reponerse cada tanto, porque cada venta le
//! descontaba una unidad. Ahora nace `physical_stock = false`, y lo que estos
//! tests defienden es exactamente eso:
//!
//! 1. **El campo llega a la base por la API pública** — no sólo por el seed.
//! 2. **El stock del centinela no se mueve nunca.** Es el assert falsable: si se
//!    mueve, `physical_stock` no llegó, y el colchón hacía falta después de todo.
//! 3. **No ensucia las alertas ni el tablero.** `product_stats` lo mantiene un
//!    EVENT vivo de SurrealDB (migración 0031), no código Rust: un test de
//!    función pura pasaría en verde con el bug intacto. Un centinela en `stock 0`
//!    contado como "sin stock" sería la app gritando un problema que no existe,
//!    sobre un producto que la dueña nunca cargó.
//! 4. **Un servicio no puede nacer con stock**, porque ese stock no emitiría
//!    `product_branch_stock` (migración 0041) y después nadie podría moverlo.

use domain::catalog::{model::*, service as catalog};
use domain::sales::{model::*, service as sales};
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
        .query("CREATE tenant SET name = 'Puesto de feria', slug = 'feria' RETURN id")
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

/// Lo que manda la app para el centinela: sin inventario, sin stock, precio 0
/// (el precio de catálogo no se usa nunca — cada línea lleva el monto dicho).
fn centinela() -> NewProduct {
    NewProduct {
        name: "Venta suelta".into(),
        slug: None,
        description: None,
        price: dec("0"),
        cost_price: None,
        stock: 0,
        physical_stock: Some(false),
        category: None,
        image_url: None,
        external_id: None,
        laboratory: None,
        therapeutic_action: None,
        active_ingredient: None,
        prescription_type: None,
        presentation: None,
        discount_percent: None,
        attrs: Some(serde_json::json!({ "rb_venta_suelta": true })),
    }
}

fn producto_fisico(name: &str, stock: i64) -> NewProduct {
    NewProduct {
        name: name.into(),
        price: dec("1000"),
        stock,
        physical_stock: None, // ausente = el DEFAULT de la base: bien físico
        ..centinela()
    }
}

/// Una venta en efectivo de una sola línea, con el monto en `unit_price`.
fn cobro(producto: &ProductDto, monto: &str) -> PosSaleRequest {
    PosSaleRequest {
        items: vec![PosSaleItem {
            product: producto.id.clone(),
            product_name: producto.name.clone(),
            quantity: 1,
            unit_price: dec(monto),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec(monto)),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: vec![],
        branch: None,
    }
}

/// El campo viaja por el camino público (`service::create_product`), que es el
/// que atiende `POST /api/v1/products`. Antes sólo lo podía pedir el seed.
#[tokio::test]
async fn el_centinela_nace_sin_inventario_por_la_api_publica() {
    let (db, tenant, _) = setup().await;

    let p = catalog::create_product(&db, &tenant, centinela())
        .await
        .unwrap();
    assert!(!p.physical_stock, "nace como servicio");
    assert_eq!(p.stock, 0, "y sin colchón de stock");

    // Releído de la base, no el DTO que devolvió el CREATE.
    let leido = catalog::get_product(&db, &tenant, &p.id).await.unwrap();
    assert!(!leido.physical_stock);

    // Y lo de siempre sigue siendo lo de siempre: sin pedir nada, físico.
    let normal = catalog::create_product(&db, &tenant, producto_fisico("Tomate", 12))
        .await
        .unwrap();
    assert!(
        normal.physical_stock,
        "un producto que no pide nada sigue siendo un bien físico"
    );
}

/// **El assert falsable.** Antes cada venta suelta le descontaba una unidad al
/// centinela; por eso hacía falta el colchón de 100.000 y la reposición. Si esto
/// se pone rojo, `physical_stock` no llegó a la base.
#[tokio::test]
async fn cien_ventas_sueltas_no_mueven_el_stock_del_centinela() {
    let (db, tenant, admin) = setup().await;
    let p = catalog::create_product(&db, &tenant, centinela())
        .await
        .unwrap();

    for i in 0..100 {
        let monto = format!("{}", 500 + i * 10);
        let resp = sales::post_sale(
            &db,
            &tenant,
            Some(&admin),
            Some("admin"),
            None,
            cobro(&p, &monto),
        )
        .await
        .unwrap();

        // El total sale del `unit_price` de la línea, no del precio de catálogo
        // (que es 0): es lo que hace posible cobrar un monto suelto.
        assert_eq!(resp.order.total, dec(&monto), "venta {i}");
        assert!(
            resp.stock_movements.is_empty(),
            "venta {i}: un servicio no emite movimiento de inventario"
        );

        let ahora = catalog::get_product(&db, &tenant, &p.id).await.unwrap();
        assert_eq!(ahora.stock, 0, "el stock del centinela se movió en la venta {i}");
    }
}

/// La razón por la que el campo no es cosmético: con `stock 0` y sin él, el
/// centinela aparecería como "sin stock" en las alertas y en el tablero. Lo
/// mantiene un EVENT de SurrealDB, así que hay que leerlo de la base.
#[tokio::test]
async fn el_centinela_no_ensucia_las_alertas_ni_el_tablero() {
    let (db, tenant, _) = setup().await;

    // Un negocio con una sola cosa cargada y bien surtida: cero alertas.
    catalog::create_product(&db, &tenant, producto_fisico("Tomate", 40))
        .await
        .unwrap();
    let antes = catalog::stats(&db, &tenant).await.unwrap();
    assert_eq!((antes.low_stock, antes.out_of_stock), (0, 0));

    let p = catalog::create_product(&db, &tenant, centinela())
        .await
        .unwrap();

    let despues = catalog::stats(&db, &tenant).await.unwrap();
    assert_eq!(
        (despues.low_stock, despues.out_of_stock),
        (0, 0),
        "el centinela no puede aparecer como agotado: la dueña nunca lo cargó"
    );
    // Sigue siendo un producto vendible, así que sí cuenta en total/activos.
    assert_eq!((despues.total, despues.active), (2, 2));
    assert_eq!(
        despues.inventory_value, antes.inventory_value,
        "un servicio no vale inventario"
    );

    // Y tampoco entra a la lista de stock bajo, que es otra consulta distinta
    // (`catalog::repo::list_products_opts`, no el agregado).
    let bajos = catalog::list_products(
        &db,
        &tenant,
        ProductFilters {
            low_stock: Some(5),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(
        bajos.iter().all(|x| x.id != p.id),
        "el centinela no puede salir en «reponer»"
    );
}

/// Vender contra el centinela no le inventa estadísticas de producto: la
/// migración 0031 redefine `product_stats_maint` atado a `physical_stock`, y
/// una venta de servicio no toca `product.stock`, que es lo que ese EVENT mira.
#[tokio::test]
async fn vender_suelto_no_ensucia_las_estadisticas() {
    let (db, tenant, admin) = setup().await;
    catalog::create_product(&db, &tenant, producto_fisico("Tomate", 40))
        .await
        .unwrap();
    let p = catalog::create_product(&db, &tenant, centinela())
        .await
        .unwrap();
    let antes = catalog::stats(&db, &tenant).await.unwrap();

    for _ in 0..5 {
        sales::post_sale(
            &db,
            &tenant,
            Some(&admin),
            Some("admin"),
            None,
            cobro(&p, "2000"),
        )
        .await
        .unwrap();
    }

    let despues = catalog::stats(&db, &tenant).await.unwrap();
    assert_eq!(
        (
            despues.total,
            despues.active,
            despues.low_stock,
            despues.out_of_stock
        ),
        (antes.total, antes.active, antes.low_stock, antes.out_of_stock),
        "cinco ventas sueltas movieron el agregado de productos"
    );
    assert_eq!(despues.inventory_value, antes.inventory_value);
}

/// El borde que abre exponer el campo: un servicio con stock quedaría con
/// `product.stock` sin `product_branch_stock` detrás (la migración 0041 sólo lo
/// emite para bienes físicos) y `stock::service` se niega a moverlo — o sea, una
/// cifra que nadie podría corregir después. Se rechaza al crear.
#[tokio::test]
async fn un_servicio_no_puede_nacer_con_stock() {
    let (db, tenant, _) = setup().await;
    let mut malo = centinela();
    malo.stock = 100_000;

    let err = catalog::create_product(&db, &tenant, malo).await.unwrap_err();
    assert!(
        err.to_string().contains("physical_stock"),
        "el mensaje tiene que decir cuál es el campo en conflicto: {err}"
    );
}

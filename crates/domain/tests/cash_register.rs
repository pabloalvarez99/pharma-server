//! Cash register integration tests (kv-mem).
//! Covers open/close lifecycle, arqueo math with cash sales + movements,
//! single-open invariant, and tenant isolation.

use std::str::FromStr;

use domain::cash_register::{model::*, service};
use domain::catalog::{model::*, service as catalog};
use domain::sales::{model::*, service as sales};
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
    let mut r = db
        .query("CREATE tenant SET name='T', slug='t' RETURN id")
        .await
        .unwrap();
    let tenant: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();
    let mut r = db
        .query("CREATE user SET tenant=$t, email='a@t.l', password='x', roles=['admin'] RETURN id")
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let user: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().unwrap();
    (db, tenant, user)
}

fn new_product(name: &str, price: &str, stock: i64) -> NewProduct {
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
        physical_stock: None,
        discount_percent: None,
        attrs: None,
    }
}

#[tokio::test]
async fn open_then_close_with_sales_and_movements_computes_expected_and_discrepancia() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "caja-1".into(),
            register: None,
            branch: None,
            opening_cash: dec("10000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(s.status, "open");
    assert_eq!(s.opening_cash, dec("10000"));

    // Two cash sales of 1500 each (total 3000).
    let p = catalog::create_product(&db, &tenant, new_product("Para 500", "1500", 50))
        .await
        .unwrap();
    for _ in 0..2 {
        let req = PosSaleRequest {
            items: vec![PosSaleItem {
                product: p.id.clone(),
                product_name: p.name.clone(),
                quantity: 1,
                unit_price: dec("1500"),
            }],
            payment_method: "pos_cash".into(),
            cash_amount: Some(dec("1500")),
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
        sales::post_sale(&db, &tenant, Some(&user), Some("admin"), None, req)
            .await
            .unwrap();
    }

    // +2000 ingreso, -500 retiro.
    service::add_movement(
        &db,
        &tenant,
        Some(&user),
        &s.id,
        CashMovementInput {
            tipo: "ingreso".into(),
            amount: dec("2000"),
            reason: "vuelto extra".into(),
        },
    )
    .await
    .unwrap();
    service::add_movement(
        &db,
        &tenant,
        Some(&user),
        &s.id,
        CashMovementInput {
            tipo: "retiro".into(),
            amount: dec("500"),
            reason: "depósito caja fuerte".into(),
        },
    )
    .await
    .unwrap();

    // Expected = 10000 + 3000 + 2000 - 500 = 14500
    let live = service::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(live.cash_sales, dec("3000"));
    assert_eq!(live.movements_in, dec("2000"));
    assert_eq!(live.movements_out, dec("500"));
    assert_eq!(live.session.closing_cash_expected, Some(dec("14500")));

    // Close with counted = 14450 → discrepancia = -50 (short).
    let close = service::close_session(
        &db,
        &tenant,
        &s.id,
        CloseSessionInput {
            closing_cash_counted: dec("14450"),
            notes: Some("falta 50".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(close.session.status, "closed");
    assert_eq!(close.session.closing_cash_expected, Some(dec("14500")));
    assert_eq!(close.session.closing_cash_counted, Some(dec("14450")));
    assert_eq!(close.session.discrepancia, Some(dec("-50")));
    assert!(close.session.closed_at.is_some());
}

#[tokio::test]
async fn cannot_open_second_session_for_same_user() {
    let (db, tenant, user) = setup().await;
    service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "caja-1".into(),
            register: None,
            branch: None,
            opening_cash: dec("5000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let err = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "caja-2".into(),
            register: None,
            branch: None,
            opening_cash: dec("5000"),
            notes: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
}

/// Concurrent `open_session` for the SAME cashier must yield exactly ONE open
/// drawer — the check-then-act (`SELECT count` then `CREATE`) is a TOCTOU race
/// without a per-(tenant,user) lock: two tasks both read count=0 and both
/// CREATE, leaving the cashier with two open sessions that split
/// `cash_sales_running` and corrupt arqueo/cierre. Same race class SALE_LOCKS
/// closed for the POS write path (BUG-003/004).
#[tokio::test]
async fn concurrent_open_same_user_yields_single_session() {
    let (db, tenant, user) = setup().await;
    let mut tasks = Vec::new();
    for i in 0..8 {
        let db = db.clone();
        let tenant = tenant.clone();
        let user = user.clone();
        tasks.push(async move {
            service::open_session(
                &db,
                &tenant,
                &user,
                OpenSessionInput {
                    register_name: format!("caja-{i}"),
                    register: None,
                    branch: None,
                    opening_cash: dec("5000"),
                    notes: None,
                },
            )
            .await
        });
    }
    let results = futures::future::join_all(tasks).await;
    let ok = results.iter().filter(|r| r.is_ok()).count();
    let conflicts = results
        .iter()
        .filter(|r| matches!(r, Err(e) if e.code() == "CONFLICT"))
        .count();
    assert_eq!(ok, 1, "exactly one concurrent open must win");
    assert_eq!(
        conflicts, 7,
        "the other 7 must get CONFLICT, not a 2nd drawer"
    );

    // And the DB holds exactly one open session for the cashier.
    #[derive(serde::Deserialize)]
    struct C {
        count: i64,
    }
    let mut r = db
        .query(
            "SELECT count() AS count FROM cash_register_session \
             WHERE tenant=$t AND user=$u AND status='open' GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .bind(("u", user.clone()))
        .await
        .unwrap();
    let c: Option<C> = r.take(0).unwrap();
    assert_eq!(c.map(|x| x.count).unwrap_or(0), 1);
}

#[tokio::test]
async fn movement_on_closed_session_rejected() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "c1".into(),
            register: None,
            branch: None,
            opening_cash: dec("1000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    service::close_session(
        &db,
        &tenant,
        &s.id,
        CloseSessionInput {
            closing_cash_counted: dec("1000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let err = service::add_movement(
        &db,
        &tenant,
        Some(&user),
        &s.id,
        CashMovementInput {
            tipo: "ingreso".into(),
            amount: dec("100"),
            reason: "tarde".into(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
}

#[tokio::test]
async fn close_already_closed_is_conflict() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "c1".into(),
            register: None,
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();
    service::close_session(
        &db,
        &tenant,
        &s.id,
        CloseSessionInput {
            closing_cash_counted: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let err = service::close_session(
        &db,
        &tenant,
        &s.id,
        CloseSessionInput {
            closing_cash_counted: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
}

/// Concurrent `close_session` on the SAME session must close it exactly once —
/// the check-then-act (`compute_summary` reads `status='open'`, then UPDATE) is a
/// TOCTOU race without the per-(tenant,session) lock: two tasks both pass the
/// open-check and both UPDATE, the second silently overwriting the first's
/// counted/discrepancia. Exactly one must win; the rest get CONFLICT.
#[tokio::test]
async fn concurrent_close_same_session_closes_once() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "c1".into(),
            register: None,
            branch: None,
            opening_cash: dec("1000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let mut tasks = Vec::new();
    for i in 0..8 {
        let db = db.clone();
        let tenant = tenant.clone();
        let id = s.id.clone();
        tasks.push(async move {
            service::close_session(
                &db,
                &tenant,
                &id,
                CloseSessionInput {
                    // Distinct counted per task so a double-write would be visible.
                    closing_cash_counted: dec(&format!("{}", 1000 + i)),
                    notes: None,
                },
            )
            .await
        });
    }
    let results = futures::future::join_all(tasks).await;
    let ok = results.iter().filter(|r| r.is_ok()).count();
    let conflicts = results
        .iter()
        .filter(|r| matches!(r, Err(e) if e.code() == "CONFLICT"))
        .count();
    assert_eq!(ok, 1, "exactly one concurrent close must win");
    assert_eq!(
        conflicts, 7,
        "the other 7 must get CONFLICT, not a re-close"
    );

    // The frozen row reflects the winner only (one closed, counted unchanged after).
    let final_s = service::get_session(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(final_s.status, "closed");
    assert!(final_s.closing_cash_counted.is_some());
}

/// A `cash_movement` must not slip in between a close's `compute_summary` snapshot
/// and its freeze (it would be excluded from `expected` → phantom discrepancia).
/// The shared per-session lock makes movement and close mutually exclusive: under
/// concurrency every movement that succeeds is included in the close's expected,
/// and any that loses the race to the close is rejected as CONFLICT (closed).
#[tokio::test]
async fn concurrent_movement_and_close_keep_drawer_consistent() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "c1".into(),
            register: None,
            branch: None,
            opening_cash: dec("1000"),
            notes: None,
        },
    )
    .await
    .unwrap();

    let mv = {
        let db = db.clone();
        let tenant = tenant.clone();
        let user = user.clone();
        let id = s.id.clone();
        async move {
            service::add_movement(
                &db,
                &tenant,
                Some(&user),
                &id,
                CashMovementInput {
                    tipo: "ingreso".into(),
                    amount: dec("500"),
                    reason: "carrera".into(),
                },
            )
            .await
        }
    };
    let cl = {
        let db = db.clone();
        let tenant = tenant.clone();
        let id = s.id.clone();
        async move {
            service::close_session(
                &db,
                &tenant,
                &id,
                CloseSessionInput {
                    closing_cash_counted: dec("1500"),
                    notes: None,
                },
            )
            .await
        }
    };
    let (mv_res, cl_res) = futures::future::join(mv, cl).await;

    // Close always wins or loses cleanly; never panics, never double-closes.
    let final_s = service::get_session(&db, &tenant, &s.id).await.unwrap();
    if cl_res.is_ok() {
        assert_eq!(final_s.status, "closed");
        let expected = final_s.closing_cash_expected.unwrap();
        // If the movement landed before the freeze it is in expected (1500);
        // otherwise it was rejected (closed) and expected stays at opening (1000).
        match mv_res {
            Ok(_) => assert_eq!(
                expected,
                dec("1500"),
                "a movement that succeeded must be counted in the close's expected"
            ),
            Err(e) => {
                assert_eq!(e.code(), "CONFLICT");
                assert_eq!(expected, dec("1000"));
            }
        }
    }
}

#[tokio::test]
async fn invalid_movement_type_or_amount_rejected() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "c1".into(),
            register: None,
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let err = service::add_movement(
        &db,
        &tenant,
        Some(&user),
        &s.id,
        CashMovementInput {
            tipo: "fuga".into(),
            amount: dec("100"),
            reason: "test".into(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
    let err = service::add_movement(
        &db,
        &tenant,
        Some(&user),
        &s.id,
        CashMovementInput {
            tipo: "ingreso".into(),
            amount: dec("0"),
            reason: "test".into(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
}

fn cash_sale(product_id: &str, name: &str, price: &str) -> PosSaleRequest {
    PosSaleRequest {
        items: vec![PosSaleItem {
            product: product_id.into(),
            product_name: name.into(),
            quantity: 1,
            unit_price: dec(price),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec(price)),
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

/// Venta con VUELTO: el agregado `cash_sales_running` suma lo que quedó en el
/// cajón, no lo que el cliente entregó.
///
/// Detector de la migración 0046. Es un detector de verdad porque las ventas
/// se pagan con billete grande: si alguien vuelve a sumar `cash_amount` crudo,
/// el número da 6000 en vez de 4500 y esto revienta. Cobrar exacto — como hacía
/// la versión anterior de este test — hace que las dos fórmulas coincidan y el
/// test no distingue nada.
#[tokio::test]
async fn cash_sales_running_suma_lo_que_quedo_no_lo_que_entrego() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "c1".into(),
            register: None,
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let p = catalog::create_product(&db, &tenant, new_product("Para 500", "1500", 50))
        .await
        .unwrap();
    // Tres ventas de 1500 pagadas con 2000: entregado 6000, en el cajón 4500.
    for _ in 0..3 {
        let mut req = cash_sale(&p.id, &p.name, "1500");
        req.cash_amount = Some(dec("2000"));
        sales::post_sale(&db, &tenant, Some(&user), Some("admin"), None, req)
            .await
            .unwrap();
    }
    let live = service::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(
        live.cash_sales,
        dec("4500"),
        "el vuelto sale del cajón: entraron 4500, no los 6000 entregados"
    );
    assert_eq!(live.cash_sales, scan_cash_sales(&db, &tenant, &live).await);
}

/// El agregado mantenido tiene que dar lo mismo que un escaneo independiente
/// DESPUÉS de una devolución real, que es donde una vista precomputada se
/// equivoca (`surrealdb-view-update-gotcha`).
///
/// Desde la 0049 el cajón es libro de caja puro: la venta se queda BRUTA en
/// `cash_sales` y lo devuelto sale por un `cash_movement(tipo='retiro')`. Así
/// que el test mide las dos mitades por separado, y encima el esperado, que es
/// el número contra el que alguien cuenta billetes.
///
/// Es detector en las dos direcciones:
/// * si el evento volviera a restar cuando `status='refunded'`, el agregado
///   caería por debajo del escaneo y además se restaría dos veces con el
///   retiro;
/// * si el retiro no se emitiera, el esperado quedaría en 4500 con 3000 en el
///   cajón.
#[tokio::test]
async fn cash_sales_bruto_y_devolucion_por_retiro_cuadran_con_el_escaneo() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "c1".into(),
            register: None,
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let p = catalog::create_product(&db, &tenant, new_product("Para 500", "1500", 50))
        .await
        .unwrap();
    let mut orders = Vec::new();
    for _ in 0..3 {
        let v = sales::post_sale(
            &db,
            &tenant,
            Some(&user),
            Some("admin"),
            None,
            cash_sale(&p.id, &p.name, "1500"),
        )
        .await
        .unwrap();
        orders.push(v.order.id);
    }
    assert_eq!(
        service::arqueo(&db, &tenant, &s.id).await.unwrap().cash_sales,
        dec("4500")
    );

    // Devolución REAL por el camino del dominio (no un `UPDATE status` crudo:
    // eso probaba el evento contra sí mismo y se saltaba el retiro).
    sales::create_refund(
        &db,
        &tenant,
        Some(&user),
        NewDevolucion {
            order: Some(orders[0].clone()),
            tipo: "venta".into(),
            motivo: "no le sirvió".into(),
            notas: None,
            items: vec![NewDevolucionItem {
                product: Some(p.id.clone()),
                product_name: p.name.clone(),
                quantity: 1,
                unit_price: dec("1500"),
                restock: true,
            }],
            metodo_reembolso: Some("efectivo".into()),
        },
    )
    .await
    .unwrap();

    let live = service::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(
        live.cash_sales,
        dec("4500"),
        "las tres ventas entraron; devolver no reescribe lo que ya entró"
    );
    assert_eq!(
        live.movements_out,
        dec("1500"),
        "la devolución salió por un retiro con su propia fecha"
    );
    assert_eq!(
        live.cash_sales,
        scan_cash_sales(&db, &tenant, &live).await,
        "el agregado mantenido tiene que dar lo mismo que el escaneo"
    );
    // El número que alguien va a comparar contra billetes.
    let close = service::close_session(
        &db,
        &tenant,
        &s.id,
        CloseSessionInput {
            closing_cash_counted: dec("3000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        close.session.closing_cash_expected,
        Some(dec("3000")),
        "vendí 4500 y devolví 1500: el cajón espera 3000"
    );
    assert_eq!(close.session.discrepancia, Some(Decimal::ZERO));
}

/// Escaneo independiente del efectivo de las ventas de la sesión, con la misma
/// calificación que el evento después de la 0049 (`cancelled` afuera,
/// `refunded` adentro) y la fórmula canónica del dominio.
async fn scan_cash_sales(db: &Db, tenant: &Thing, live: &CloseSummary) -> Decimal {
    #[derive(serde::Deserialize)]
    struct S {
        total: Decimal,
        cash_amount: Option<Decimal>,
        card_amount: Option<Decimal>,
    }
    let mut r = db
        .query(
            "SELECT total, cash_amount, card_amount FROM order \
             WHERE tenant=$t AND payment_method IN ['pos_cash','pos_mixed'] \
               AND status != 'cancelled' \
               AND created_at >= $a",
        )
        .bind(("t", tenant.clone()))
        .bind(("a", surrealdb::sql::Datetime::from(live.session.opened_at)))
        .await
        .unwrap();
    let scan: Vec<S> = r.take(0).unwrap();
    scan.into_iter()
        .map(|o| {
            domain::invariants::cash_into_drawer(
                o.total,
                o.cash_amount,
                o.card_amount.unwrap_or(Decimal::ZERO),
            )
        })
        .sum()
}

/// An order in tenant A must not bump tenant B's session running total.
#[tokio::test]
async fn cash_sales_running_is_tenant_isolated() {
    let (db, tenant_a, user_a) = setup().await;
    let tenant_b: Thing = db
        .query("CREATE tenant SET name='B', slug='b' RETURN id")
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    let user_b: Thing = db
        .query("CREATE user SET tenant=$t, email='b@t.l', password='x', roles=['admin'] RETURN id")
        .bind(("t", tenant_b.clone()))
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    // Both sessions open BEFORE the sale (so the window guard would let either
    // accrue if the tenant filter were missing).
    let sb = service::open_session(
        &db,
        &tenant_b,
        &user_b,
        OpenSessionInput {
            register_name: "cb".into(),
            register: None,
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let sa = service::open_session(
        &db,
        &tenant_a,
        &user_a,
        OpenSessionInput {
            register_name: "ca".into(),
            register: None,
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();

    let p = catalog::create_product(&db, &tenant_a, new_product("Para", "1500", 50))
        .await
        .unwrap();
    sales::post_sale(
        &db,
        &tenant_a,
        Some(&user_a),
        Some("admin"),
        None,
        cash_sale(&p.id, &p.name, "1500"),
    )
    .await
    .unwrap();

    assert_eq!(
        service::arqueo(&db, &tenant_a, &sa.id)
            .await
            .unwrap()
            .cash_sales,
        dec("1500")
    );
    assert_eq!(
        service::arqueo(&db, &tenant_b, &sb.id)
            .await
            .unwrap()
            .cash_sales,
        dec("0")
    );
}

#[tokio::test]
async fn cross_tenant_isolation_for_sessions() {
    let (db, tenant, user) = setup().await;
    let other_tenant: Thing = db
        .query("CREATE tenant SET name='O', slug='o' RETURN id")
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "c1".into(),
            register: None,
            branch: None,
            opening_cash: dec("1000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let err = service::get_session(&db, &other_tenant, &s.id)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "NOT_FOUND");
}

// --- transferencia (V4 pagos, migración 0043) -------------------------------

/// **El invariante del vector pagos**: una venta por transferencia es plata que
/// entró al negocio pero NO al cajón. El efectivo esperado del arqueo tiene que
/// ignorarla — si la sumara, el cajero cerraría con un faltante fantasma todos
/// los días. Se verifica contra el mismo `arqueo` que usa el cierre real.
#[tokio::test]
async fn venta_por_transferencia_no_entra_al_efectivo_esperado() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "caja-1".into(),
            register: None,
            branch: None,
            opening_cash: dec("10000"),
            notes: None,
        },
    )
    .await
    .unwrap();

    let p = catalog::create_product(&db, &tenant, new_product("Pan", "2000", 50))
        .await
        .unwrap();
    let venta = |metodo: &str, cash: Option<Decimal>| PosSaleRequest {
        items: vec![PosSaleItem {
            product: p.id.clone(),
            product_name: p.name.clone(),
            quantity: 1,
            unit_price: dec("2000"),
        }],
        payment_method: metodo.into(),
        cash_amount: cash,
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

    // Una en efectivo (sí entra al cajón) y una por transferencia (no).
    sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta("pos_cash", Some(dec("2000"))),
    )
    .await
    .unwrap();
    let transfer = sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta("pos_transferencia", None),
    )
    .await
    .expect("la venta por transferencia debe persistir (whitelist 0043)");

    // Guardián del whitelist: sin `DEFINE FIELD OVERWRITE` en 0043 la venta ni
    // siquiera se guardaría (la tabla es SCHEMAFULL y el ASSERT la rechaza).
    assert_eq!(
        transfer.order.payment_method, "pos_transferencia",
        "el tender tiene que persistir tal cual se cobró"
    );
    // Liquida exacto: no hay efectivo recibido, así que tampoco hay vuelto que
    // devolver ni plata que el cajón deba esperar.
    assert!(
        transfer.order.cash_amount.is_none(),
        "una transferencia no registra efectivo recibido"
    );

    let live = service::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(
        live.cash_sales,
        dec("2000"),
        "sólo la venta en efectivo cuenta como venta en efectivo"
    );
    assert_eq!(
        live.session.closing_cash_expected,
        Some(dec("12000")),
        "esperado = apertura 10000 + 2000 en efectivo; la transferencia NO infla el cajón"
    );

    // Y el cierre cuadra contando sólo el efectivo real: cero discrepancia.
    let close = service::close_session(
        &db,
        &tenant,
        &s.id,
        CloseSessionInput {
            closing_cash_counted: dec("12000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        close.session.discrepancia,
        Some(Decimal::ZERO),
        "el cajón cuadra: la transferencia nunca estuvo en el cajón"
    );
}

// --- el bug de plata: el vuelto (migración 0046) ------------------------------

/// Una venta con vuelto, tal como la manda el POS: `cash_amount` es lo que el
/// cliente **entregó** ("Pagó con" en Android), no lo que se cobró.
fn venta_con_vuelto(
    product_id: &str,
    name: &str,
    price: &str,
    metodo: &str,
    cash: Option<&str>,
    card: Option<&str>,
) -> PosSaleRequest {
    PosSaleRequest {
        items: vec![PosSaleItem {
            product: product_id.into(),
            product_name: name.into(),
            quantity: 1,
            unit_price: dec(price),
        }],
        payment_method: metodo.into(),
        cash_amount: cash.map(dec),
        card_amount: card.map(dec),
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

/// EL BUG DE PLATA. Venta de $5.000 pagada con un billete de $10.000 sobre una
/// apertura de $50.000: en el cajón quedan $55.000 (los otros $5.000 se fueron
/// como vuelto). Hasta la migración 0046 el evento de 0030 sumaba `cash_amount`
/// crudo y el arqueo pedía $60.000 — un faltante de $5.000 inventado, por venta
/// y todos los días.
///
/// Este test tiene que ir contra la base con las migraciones corridas: el bug
/// no vivía en una función pura sino en el `DEFINE EVENT` de SurrealDB, así que
/// un test del invariante solo no lo habría atrapado nunca.
#[tokio::test]
async fn arqueo_descuenta_el_vuelto_de_una_venta_en_efectivo() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "caja-1".into(),
            register: None,
            branch: None,
            opening_cash: dec("50000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let p = catalog::create_product(&db, &tenant, new_product("Palta kilo", "5000", 50))
        .await
        .unwrap();

    let venta = sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta_con_vuelto(&p.id, &p.name, "5000", "pos_cash", Some("10000"), None),
    )
    .await
    .unwrap();
    assert_eq!(
        venta.order.cash_amount,
        Some(dec("10000")),
        "la orden guarda lo ENTREGADO: es el dato con el que se calcula el vuelto"
    );

    let live = service::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(
        live.cash_sales,
        dec("5000"),
        "al cajón entraron 5000, no los 10000 del billete: 5000 volvieron como vuelto"
    );
    assert_eq!(
        live.session.closing_cash_expected,
        Some(dec("55000")),
        "esperado = apertura 50000 + 5000 de venta neta"
    );

    // Contar el cajón físico: hay 55.000 y el arqueo cuadra en cero. Con el bug
    // esta misma plata daba -5000 y el feriante salía a buscar un faltante que
    // nunca existió.
    let close = service::close_session(
        &db,
        &tenant,
        &s.id,
        CloseSessionInput {
            closing_cash_counted: dec("55000"),
            notes: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        close.session.discrepancia,
        Some(Decimal::ZERO),
        "el cajón cuadra: el vuelto salió del mismo cajón"
    );
}

/// Venta mixta: el vuelto sale del lado efectivo (a una tarjeta no se le cobra
/// de más), así que al cajón entra `total − tarjeta`. Total 8.000 con 3.000 de
/// tarjeta y un billete de 10.000 → entran 5.000, vuelven 5.000.
#[tokio::test]
async fn arqueo_de_una_venta_mixta_entra_total_menos_tarjeta() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "caja-1".into(),
            register: None,
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let p = catalog::create_product(&db, &tenant, new_product("Canasto", "8000", 50))
        .await
        .unwrap();

    sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta_con_vuelto(
            &p.id,
            &p.name,
            "8000",
            "pos_mixed",
            Some("10000"),
            Some("3000"),
        ),
    )
    .await
    .unwrap();

    let live = service::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(
        live.cash_sales,
        dec("5000"),
        "mixta: entran 8000 - 3000 de tarjeta = 5000; los otros 5000 del billete son vuelto"
    );
}

/// `cash_amount` ausente en una venta en efectivo significa "no se registró lo
/// entregado" — en Android "Pagó con" es opcional —, **no** "no entró plata".
/// El evento de 0030 lo contaba como 0 y dejaba el arqueo por DEBAJO: el mismo
/// bug al revés, y el que hace que al cajero le "sobre" plata.
#[tokio::test]
async fn arqueo_cuenta_la_venta_en_efectivo_sin_monto_entregado() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "caja-1".into(),
            register: None,
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let p = catalog::create_product(&db, &tenant, new_product("Cilantro", "700", 50))
        .await
        .unwrap();

    sales::post_sale(
        &db,
        &tenant,
        Some(&user),
        Some("admin"),
        None,
        venta_con_vuelto(&p.id, &p.name, "700", "pos_cash", None, None),
    )
    .await
    .unwrap();

    let live = service::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(
        live.cash_sales,
        dec("700"),
        "cobró 700 en efectivo: están en el cajón aunque nadie anotara con cuánto pagó"
    );
}

/// Devolver una venta con vuelto tiene que sacar el NETO, no lo entregado: de
/// una venta de 1500 pagada con 2000 sale 1500, no 2000. Si sale de más, el
/// arqueo del turno queda por debajo y aparece un faltante nuevo justo después
/// de una devolución.
///
/// Desde la 0049 la plata sale por un `cash_movement(tipo='retiro')` y no
/// reescribiendo `cash_sales`, así que el test mide las dos mitades. Es la rama
/// UPDATE del evento la que está bajo prueba: la devolución deja la orden en
/// `status='refunded'`, y el agregado NO se tiene que mover.
///
/// Detector en las tres direcciones:
/// * si el evento sumara lo entregado en vez del neto, `cash_sales` daría 4000;
/// * si la rama UPDATE volviera a calificar por `status != 'refunded'` (la 0046),
///   restaría 1500 de nuevo y `cash_sales` caería a 1500 — doble descuento
///   contra el retiro;
/// * si el retiro no se emitiera, el esperado quedaría en 3000 con 1500 en el
///   cajón.
#[tokio::test]
async fn devolver_una_venta_con_vuelto_saca_solo_el_neto() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "caja-1".into(),
            register: None,
            branch: None,
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let p = catalog::create_product(&db, &tenant, new_product("Tomate kilo", "1500", 50))
        .await
        .unwrap();

    // Dos ventas de 1500, cada una pagada con 2000 → 3000 netos en el cajón.
    let mut orders = Vec::new();
    for _ in 0..2 {
        let v = sales::post_sale(
            &db,
            &tenant,
            Some(&user),
            Some("admin"),
            None,
            venta_con_vuelto(&p.id, &p.name, "1500", "pos_cash", Some("2000"), None),
        )
        .await
        .unwrap();
        orders.push(v.order.id);
    }
    assert_eq!(
        service::arqueo(&db, &tenant, &s.id)
            .await
            .unwrap()
            .cash_sales,
        dec("3000"),
        "entraron 1500 por venta, no los 2000 del billete"
    );

    // Devolución REAL por el camino del dominio. Devuelve la venta entera, así
    // que la orden queda `status='refunded'`: es el caso exacto que la 0046
    // usaba para restar del agregado.
    sales::create_refund(
        &db,
        &tenant,
        Some(&user),
        NewDevolucion {
            order: Some(orders[0].clone()),
            tipo: "venta".into(),
            motivo: "no le sirvió".into(),
            notas: None,
            items: vec![NewDevolucionItem {
                product: Some(p.id.clone()),
                product_name: p.name.clone(),
                quantity: 1,
                unit_price: dec("1500"),
                restock: true,
            }],
            metodo_reembolso: Some("efectivo".into()),
        },
    )
    .await
    .unwrap();

    let live = service::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(
        live.cash_sales,
        dec("3000"),
        "devolver no reescribe lo que ya entró: el agregado se queda bruto"
    );
    assert_eq!(
        live.movements_out,
        dec("1500"),
        "la devolución saca los 1500 que entraron, no los 2000 del billete"
    );

    let close = service::close_session(
        &db,
        &tenant,
        &s.id,
        CloseSessionInput {
            closing_cash_counted: dec("1500"),
            notes: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        close.session.closing_cash_expected,
        Some(dec("1500")),
        "entraron 3000 netos y salieron 1500: quedan 1500 contables"
    );
    assert_eq!(close.session.discrepancia, Some(Decimal::ZERO));
}

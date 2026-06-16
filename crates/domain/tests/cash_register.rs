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
        discount_percent: None,
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
            opening_cash: dec("5000"),
            notes: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "CONFLICT");
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

#[tokio::test]
async fn invalid_movement_type_or_amount_rejected() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "c1".into(),
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
    }
}

/// The maintained `cash_sales_running` aggregate (migration 0030) must equal the
/// old live `math::sum(cash_amount)` scan — including after a sale is refunded,
/// the case a pre-computed view would get wrong (`surrealdb-view-update-gotcha`).
#[tokio::test]
async fn cash_sales_running_matches_scan_after_refund() {
    let (db, tenant, user) = setup().await;
    let s = service::open_session(
        &db,
        &tenant,
        &user,
        OpenSessionInput {
            register_name: "c1".into(),
            opening_cash: dec("0"),
            notes: None,
        },
    )
    .await
    .unwrap();
    let p = catalog::create_product(&db, &tenant, new_product("Para 500", "1500", 50))
        .await
        .unwrap();
    // Three cash sales (CREATE events) → 4500.
    for _ in 0..3 {
        sales::post_sale(
            &db,
            &tenant,
            Some(&user),
            Some("admin"),
            None,
            cash_sale(&p.id, &p.name, "1500"),
        )
        .await
        .unwrap();
    }
    let live = service::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(live.cash_sales, dec("4500"));

    // Refund one order — sales repo does `UPDATE order SET status='refunded'`,
    // which the event must subtract (UPDATE branch).
    let mut r = db
        .query("SELECT VALUE id FROM order WHERE tenant=$t LIMIT 1")
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let ids: Vec<Thing> = r.take(0).unwrap();
    db.query("UPDATE $o SET status='refunded'")
        .bind(("o", ids[0].clone()))
        .await
        .unwrap()
        .check()
        .unwrap();

    // Maintained drops to 3000 and equals an independent live scan byte-for-byte.
    let live2 = service::arqueo(&db, &tenant, &s.id).await.unwrap();
    assert_eq!(live2.cash_sales, dec("3000"));

    #[derive(serde::Deserialize)]
    struct S {
        sum: Option<Decimal>,
    }
    let mut r = db
        .query(
            "SELECT math::sum(cash_amount) AS sum FROM order \
             WHERE tenant=$t AND payment_method IN ['pos_cash','pos_mixed'] \
               AND status NOT IN ['refunded','cancelled'] \
               AND created_at >= $a GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .bind(("a", surrealdb::sql::Datetime::from(live2.session.opened_at)))
        .await
        .unwrap();
    let scan: Option<S> = r.take(0).unwrap();
    let scan_val = scan.and_then(|x| x.sum).unwrap_or(dec("0"));
    assert_eq!(
        live2.cash_sales, scan_val,
        "maintained running total must equal the live scan"
    );
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

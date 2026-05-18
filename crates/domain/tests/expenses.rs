//! Expenses + daily sales report tests (kv-mem).

use std::str::FromStr;

use domain::catalog::{model::*, service as catalog};
use domain::expenses::{model::*, service};
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

#[tokio::test]
async fn create_then_list_filters_by_category_and_payment_method() {
    let (db, tenant, user) = setup().await;
    service::create_expense(
        &db,
        &tenant,
        Some(&user),
        NewExpense {
            category: "rent".into(),
            description: "alquiler mayo".into(),
            amount: dec("500000"),
            payment_method: "bank".into(),
            cash_session: None,
            supplier: None,
            note: None,
            incurred_at: None,
        },
    )
    .await
    .unwrap();
    service::create_expense(
        &db,
        &tenant,
        Some(&user),
        NewExpense {
            category: "utilities".into(),
            description: "agua + luz".into(),
            amount: dec("85000"),
            payment_method: "cash".into(),
            cash_session: None,
            supplier: None,
            note: None,
            incurred_at: None,
        },
    )
    .await
    .unwrap();

    let all = service::list_expenses(&db, &tenant, ExpenseFilters::default())
        .await
        .unwrap();
    assert_eq!(all.len(), 2);

    let rent_only = service::list_expenses(
        &db,
        &tenant,
        ExpenseFilters {
            category: Some("rent".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(rent_only.len(), 1);
    assert_eq!(rent_only[0].amount, dec("500000"));

    let cash_only = service::list_expenses(
        &db,
        &tenant,
        ExpenseFilters {
            payment_method: Some("cash".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(cash_only.len(), 1);
}

#[tokio::test]
async fn invalid_amount_or_payment_method_rejected() {
    let (db, tenant, user) = setup().await;
    let err = service::create_expense(
        &db,
        &tenant,
        Some(&user),
        NewExpense {
            category: "x".into(),
            description: "x".into(),
            amount: Decimal::ZERO,
            payment_method: "cash".into(),
            cash_session: None,
            supplier: None,
            note: None,
            incurred_at: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");

    let err = service::create_expense(
        &db,
        &tenant,
        Some(&user),
        NewExpense {
            category: "x".into(),
            description: "x".into(),
            amount: dec("100"),
            payment_method: "bitcoin".into(),
            cash_session: None,
            supplier: None,
            note: None,
            incurred_at: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_INPUT");
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
async fn sales_daily_aggregates_orders_per_utc_date_excluding_refunded() {
    let (db, tenant, user) = setup().await;
    let p = catalog::create_product(&db, &tenant, new_product("P", "1000", 100))
        .await
        .unwrap();
    // Three pos_cash sales in a single UTC day (whatever "today" is in test
    // runtime). Sales reuse the existing post_sale; created_at = time::now().
    for _ in 0..3 {
        let req = smodel::PosSaleRequest {
            items: vec![smodel::PosSaleItem {
                product: p.id.clone(),
                product_name: p.name.clone(),
                quantity: 1,
                unit_price: dec("1000"),
            }],
            payment_method: "pos_cash".into(),
            cash_amount: Some(dec("1000")),
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

    let rows = service::sales_daily(&db, &tenant, SalesReportFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "all sales fall on one UTC date");
    assert_eq!(rows[0].orders, 3);
    assert_eq!(rows[0].revenue, dec("3000"));
    assert_eq!(rows[0].cash, dec("3000"));
    // date is YYYY-MM-DD shaped
    assert_eq!(rows[0].date.len(), 10);
    assert_eq!(&rows[0].date[4..5], "-");
}

#[tokio::test]
async fn tenant_isolation_for_expenses_and_reports() {
    let (db, tenant, user) = setup().await;
    let other: Thing = db
        .query("CREATE tenant SET name='O', slug='o' RETURN id")
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    service::create_expense(
        &db,
        &tenant,
        Some(&user),
        NewExpense {
            category: "x".into(),
            description: "x".into(),
            amount: dec("100"),
            payment_method: "cash".into(),
            cash_session: None,
            supplier: None,
            note: None,
            incurred_at: None,
        },
    )
    .await
    .unwrap();
    let other_list = service::list_expenses(&db, &other, ExpenseFilters::default())
        .await
        .unwrap();
    assert!(other_list.is_empty());
    let other_rep = service::sales_daily(&db, &other, SalesReportFilters::default())
        .await
        .unwrap();
    assert!(other_rep.is_empty());
}

async fn seed_batch(
    db: &Db,
    tenant: &Thing,
    product: &Thing,
    code: &str,
    expiry_expr: &str,
    stock: i64,
    active: bool,
) {
    db.query(format!(
        "CREATE product_batch SET tenant=$t, product=$p, batch_code=$c, \
         expiry_date={expiry_expr}, stock=$s, active=$a"
    ))
    .bind(("t", tenant.clone()))
    .bind(("p", product.clone()))
    .bind(("c", code.to_string()))
    .bind(("s", stock))
    .bind(("a", active))
    .await
    .unwrap()
    .check()
    .unwrap();
}

#[tokio::test]
async fn near_expiry_window_sort_expired_and_exclusions() {
    let (db, tenant, _user) = setup().await;
    let p = catalog::create_product(&db, &tenant, new_product("P", "1000", 100))
        .await
        .unwrap();
    let pid = Thing::from_str(&p.id).unwrap();

    seed_batch(&db, &tenant, &pid, "SOON", "time::now() + 10d", 5, true).await;
    seed_batch(&db, &tenant, &pid, "EDGE", "time::now() + 25d", 3, true).await;
    seed_batch(&db, &tenant, &pid, "FAR", "time::now() + 200d", 9, true).await;
    seed_batch(&db, &tenant, &pid, "EXPIRED", "time::now() - 5d", 2, true).await;
    seed_batch(&db, &tenant, &pid, "INACTIVE", "time::now() + 1d", 7, false).await;
    seed_batch(&db, &tenant, &pid, "ZERO", "time::now() + 1d", 0, true).await;

    // Default 30d window: EXPIRED + SOON + EDGE; FAR/INACTIVE/ZERO excluded.
    let rows = service::near_expiry(&db, &tenant, NearExpiryFilters::default())
        .await
        .unwrap();
    let codes: Vec<&str> = rows.iter().map(|r| r.batch_code.as_str()).collect();
    assert_eq!(
        codes,
        vec!["EXPIRED", "SOON", "EDGE"],
        "sorted by expiry asc"
    );

    assert!(rows[0].expired);
    assert!(rows[0].days_to_expiry < 0, "EXPIRED has negative days");
    assert_eq!(rows[0].stock, 2);
    assert_eq!(rows[0].product_name, "P");
    assert_eq!(rows[0].product_id, p.id);

    assert!(!rows[1].expired);
    assert!(
        (9..=10).contains(&rows[1].days_to_expiry),
        "SOON ~10 days, got {}",
        rows[1].days_to_expiry
    );

    // Wide window pulls FAR in too.
    let wide = service::near_expiry(&db, &tenant, NearExpiryFilters { days: Some(365) })
        .await
        .unwrap();
    assert_eq!(wide.len(), 4);
    assert_eq!(wide.last().unwrap().batch_code, "FAR");

    // days=0 → only batches expiring at/before now.
    let none_future = service::near_expiry(&db, &tenant, NearExpiryFilters { days: Some(0) })
        .await
        .unwrap();
    assert_eq!(none_future.len(), 1);
    assert_eq!(none_future[0].batch_code, "EXPIRED");
}

#[tokio::test]
async fn near_expiry_tenant_scoped() {
    let (db, tenant, _user) = setup().await;
    let p = catalog::create_product(&db, &tenant, new_product("P", "1000", 100))
        .await
        .unwrap();
    let pid = Thing::from_str(&p.id).unwrap();
    seed_batch(&db, &tenant, &pid, "SOON", "time::now() + 5d", 5, true).await;

    let other: Thing = db
        .query("CREATE tenant SET name='O', slug='o' RETURN id")
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    let seen = service::near_expiry(&db, &other, NearExpiryFilters::default())
        .await
        .unwrap();
    assert!(seen.is_empty(), "other tenant sees no batches");
}

#[tokio::test]
async fn margins_daily_revenue_cost_margin_and_unknown_cost() {
    let (db, tenant, user) = setup().await;
    // Known-cost product: price 1000, cost 100.
    let priced = catalog::create_product(&db, &tenant, new_product("P", "1000", 100))
        .await
        .unwrap();
    // No-cost product (cost_price None).
    let nocost = catalog::create_product(
        &db,
        &tenant,
        NewProduct {
            name: "NC".into(),
            slug: None,
            description: None,
            price: dec("500"),
            cost_price: None,
            stock: 100,
            category: None,
            image_url: None,
            external_id: None,
            laboratory: None,
            therapeutic_action: None,
            active_ingredient: None,
            prescription_type: None,
            presentation: None,
            discount_percent: None,
        },
    )
    .await
    .unwrap();

    let req = smodel::PosSaleRequest {
        items: vec![
            smodel::PosSaleItem {
                product: priced.id.clone(),
                product_name: priced.name.clone(),
                quantity: 2,
                unit_price: dec("1000"),
            },
            smodel::PosSaleItem {
                product: nocost.id.clone(),
                product_name: nocost.name.clone(),
                quantity: 3,
                unit_price: dec("500"),
            },
        ],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("3500")),
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

    let rows = service::margins_daily(&db, &tenant, SalesReportFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "single UTC day");
    let r = &rows[0];
    // revenue = 2*1000 + 3*500 = 3500
    assert_eq!(r.revenue, dec("3500"));
    // cost = 2 * 100 (only the priced product has cost) = 200
    assert_eq!(r.cost, dec("200"));
    assert_eq!(r.margin, dec("3300"));
    // 3300 / 3500 * 100 = 94.2857… -> 94.29
    assert_eq!(r.margin_pct, dec("94.29"));
    assert_eq!(r.items_without_cost, 1, "the no-cost line");
}

#[tokio::test]
async fn margins_daily_tenant_scoped_and_empty() {
    let (db, tenant, _user) = setup().await;
    let other: Thing = db
        .query("CREATE tenant SET name='O', slug='o' RETURN id")
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    let rows = service::margins_daily(&db, &other, SalesReportFilters::default())
        .await
        .unwrap();
    assert!(rows.is_empty());
    let _ = tenant;
}

#[tokio::test]
async fn top_products_ranking_abc_and_limit() {
    let (db, tenant, user) = setup().await;
    let a = catalog::create_product(&db, &tenant, new_product("A", "1000", 100))
        .await
        .unwrap();
    let b = catalog::create_product(&db, &tenant, new_product("B", "500", 100))
        .await
        .unwrap();
    let c = catalog::create_product(&db, &tenant, new_product("C", "100", 100))
        .await
        .unwrap();
    // Revenue: A 8*1000=8000 (80%), B 3*500=1500 (15%), C 5*100=500 (5%).
    let req = smodel::PosSaleRequest {
        items: vec![
            smodel::PosSaleItem {
                product: a.id.clone(),
                product_name: a.name.clone(),
                quantity: 8,
                unit_price: dec("1000"),
            },
            smodel::PosSaleItem {
                product: b.id.clone(),
                product_name: b.name.clone(),
                quantity: 3,
                unit_price: dec("500"),
            },
            smodel::PosSaleItem {
                product: c.id.clone(),
                product_name: c.name.clone(),
                quantity: 5,
                unit_price: dec("100"),
            },
        ],
        payment_method: "pos_cash".into(),
        cash_amount: Some(dec("10000")),
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

    let rows = service::top_products(&db, &tenant, TopProductsFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);

    assert_eq!(rows[0].rank, 1);
    assert_eq!(rows[0].product_name, "A");
    assert_eq!(rows[0].qty_sold, 8);
    assert_eq!(rows[0].revenue, dec("8000"));
    assert_eq!(rows[0].revenue_pct, dec("80.00"));
    assert_eq!(rows[0].abc_class, "A");
    assert_eq!(rows[0].product_id.as_deref(), Some(a.id.as_str()));

    assert_eq!(rows[1].product_name, "B");
    assert_eq!(rows[1].revenue_pct, dec("15.00"));
    assert_eq!(rows[1].abc_class, "B");

    assert_eq!(rows[2].product_name, "C");
    assert_eq!(rows[2].revenue_pct, dec("5.00"));
    assert_eq!(rows[2].abc_class, "C");

    // Limit truncates output but ABC was computed over the full ranking.
    let capped = service::top_products(
        &db,
        &tenant,
        TopProductsFilters {
            from: None,
            to: None,
            limit: Some(2),
        },
    )
    .await
    .unwrap();
    assert_eq!(capped.len(), 2);
    assert_eq!(capped[1].product_name, "B");
    assert_eq!(capped[1].abc_class, "B");
}

#[tokio::test]
async fn top_products_tenant_scoped_empty() {
    let (db, _tenant, _user) = setup().await;
    let other: Thing = db
        .query("CREATE tenant SET name='O', slug='o' RETURN id")
        .await
        .unwrap()
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .unwrap();
    let rows = service::top_products(&db, &other, TopProductsFilters::default())
        .await
        .unwrap();
    assert!(rows.is_empty());
}

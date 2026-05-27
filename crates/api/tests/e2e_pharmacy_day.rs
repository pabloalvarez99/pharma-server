//! E2E scenario 1 — a full pharmacy day.
//!
//! Opens a caja, seeds 20 SKUs each with 3 staggered-expiry batches, runs 30
//! mixed POS sales (some with a loyalty customer) + 3 partial returns, then
//! verifies the day's reporting + accounting invariants. READS go through the
//! real HTTP API (`near-expiry`, `margins-daily`, `cash-sessions/.../arqueo`);
//! WRITES are seeded via `domain::*::service` because the HTTP write routes are
//! role-gated and currently 500 (BUG-001).
//!
//! Invariants asserted (green):
//! * Σ `stock_movement.delta` per (tenant, product) == `product.stock`
//! * refunded orders are excluded from `margins-daily`
//! * the urgent (<30d) lot surfaces in `near-expiry`, sorted soonest-first
//! * `arqueo.expected == opening + Σ sale.cash − Σ refunded.order.cash`
//!   (refunded orders drop out of cash entirely once `status='refunded'`)
//!
//! ## BUG-007 (medium), discovered here
//! The `product.stock == Σ product_batch.stock` invariant is BROKEN after a
//! refund restock: `apply_refund` adds the returned qty to `product.stock` and
//! writes a `return` `stock_movement`, but never restores any
//! `product_batch.stock` — so for batch-tracked SKUs `Σ batch` ends up 1-per-
//! returned-unit short of `product.stock`. Asserted in its own `#[ignore]`d
//! test below; the movement-ledger invariant (which counts both sale and
//! return deltas) still holds and stays green.

mod e2e_common;

use std::collections::HashMap;
use std::sync::Arc;

use axum::http::StatusCode;
use domain::money::Decimal;
use e2e_common::*;
use surrealdb::sql::Thing;

struct Day {
    db: Arc<db::Db>,
    tenant: Thing,
    user: Thing,
    token_free: String,
    token_pro: String,
    session_id: String,
    products: Vec<String>,
    customer: String,
    _tdb: TestDb,
}

const OPENING: &str = "50000";

async fn build_day() -> Day {
    let tdb = spawn_db().await;
    let (tid, uid, roles) = seed_tenant_admin(&tdb.db, "farmacia-dia", "admin@dia.cl").await;
    let tenant = tid_thing(&tid);
    let user = surrealdb::sql::thing(&uid).unwrap();
    let db = tdb.db.clone();

    let token_free = token_for(&uid, &tid, roles.clone());
    let token_pro = token_for(&uid, &tid, roles);

    // Open caja with $50.000.
    let session_id = seed_cash_session(&db, &tenant, &user, "caja-1", OPENING).await;

    // 20 SKUs, varied price/cost. Two of them carry interacting active
    // ingredients so the day exercises the interaction surface too.
    let mut products = Vec::with_capacity(20);
    for i in 0..20 {
        let price = 1000 + i * 250; // 1000..5750
        let cost = 500 + i * 120;
        let ai = match i {
            0 => Some("Ibuprofeno 400mg"),
            1 => Some("Warfarina 5mg"),
            _ => None,
        };
        let pid = seed_product(
            &db,
            &tenant,
            &format!("SKU {i:02}"),
            &price.to_string(),
            &cost.to_string(),
            0, // stock comes entirely from batches
            ai,
        )
        .await;

        // 3 batches/SKU, staggered expiry, 50 units each.
        let urgent = (chrono::Utc::now() + chrono::Duration::days(15)).to_rfc3339();
        let mid = (chrono::Utc::now() + chrono::Duration::days(180)).to_rfc3339();
        let far = (chrono::Utc::now() + chrono::Duration::days(730)).to_rfc3339();
        seed_batch(
            &db,
            &tenant,
            &pid,
            &format!("L{i:02}-URG"),
            &urgent,
            50,
            &cost.to_string(),
        )
        .await;
        seed_batch(
            &db,
            &tenant,
            &pid,
            &format!("L{i:02}-MID"),
            &mid,
            50,
            &cost.to_string(),
        )
        .await;
        seed_batch(
            &db,
            &tenant,
            &pid,
            &format!("L{i:02}-FAR"),
            &far,
            50,
            &cost.to_string(),
        )
        .await;
        products.push(pid);
    }

    let customer = seed_customer(&db, &tenant, "Cliente Fiel").await;

    Day {
        db,
        tenant,
        user,
        token_free,
        token_pro,
        session_id,
        products,
        customer,
        _tdb: tdb,
    }
}

fn price_of(i: usize) -> i64 {
    1000 + (i as i64) * 250
}

/// 30 sales: each picks 1..=4 SKUs (deterministic pattern), alternating cash /
/// mixed payment, every 4th sale credited to the loyalty customer. Returns the
/// list of (order_id, cash_paid) and the total cash taken in.
async fn run_sales(day: &Day) -> (Vec<(String, i64)>, i64) {
    let mut orders = Vec::new();
    let mut total_cash = 0_i64;
    for s in 0..30usize {
        let n_lines = 1 + (s % 4); // 1..=4
        let mut lines: Vec<SaleLine> = Vec::with_capacity(n_lines);
        let names: Vec<String> = (0..n_lines)
            .map(|k| {
                let idx = (s + k) % day.products.len();
                format!("SKU {idx:02}")
            })
            .collect();
        let prices: Vec<String> = (0..n_lines)
            .map(|k| price_of((s + k) % day.products.len()).to_string())
            .collect();
        let mut total = 0_i64;
        for k in 0..n_lines {
            let idx = (s + k) % day.products.len();
            lines.push(SaleLine {
                product: &day.products[idx],
                name: &names[k],
                qty: 1,
                unit_price: &prices[k],
            });
            total += price_of(idx);
        }

        let with_customer = s % 4 == 0;
        let cust = if with_customer {
            Some(day.customer.as_str())
        } else {
            None
        };

        // Even sale = pure cash; odd sale = mixed (half cash, half card).
        let resp = if s % 2 == 0 {
            total_cash += total;
            seed_sale(
                &day.db,
                &day.tenant,
                Some(&day.user),
                "pos_cash",
                Some(&total.to_string()),
                None,
                cust,
                &lines,
            )
            .await
        } else {
            let cash = total / 2;
            let card = total - cash;
            total_cash += cash;
            seed_sale(
                &day.db,
                &day.tenant,
                Some(&day.user),
                "pos_mixed",
                Some(&cash.to_string()),
                Some(&card.to_string()),
                cust,
                &lines,
            )
            .await
        };
        let cash_paid = resp
            .order
            .cash_amount
            .map(|d| d.try_into().unwrap_or(0_i64))
            .unwrap_or(0);
        orders.push((resp.order.id.clone(), cash_paid));
    }
    (orders, total_cash)
}

/// Refund 3 distinct orders fully (single-line orders chosen so the refund is
/// clean). Returns the cash that those refunded orders had contributed.
async fn run_returns(day: &Day, orders: &[(String, i64)]) -> i64 {
    // Sales 0, 8, 16 are `s % 4 == 0` (have a customer) AND `s % 8 == 0`/etc;
    // pick single-line cash orders: s=0 (1 line), s=8 (1 line), s=16 (1 line).
    let mut refunded_cash = 0_i64;
    for &s in &[0usize, 8, 16] {
        let (order_id, cash_paid) = &orders[s];
        let idx = s % day.products.len();
        let name = format!("SKU {idx:02}");
        let price = price_of(idx).to_string();
        let body = domain::sales::model::NewDevolucion {
            order: Some(order_id.clone()),
            tipo: "venta".into(),
            motivo: "devolución parcial del día".into(),
            notas: None,
            items: vec![domain::sales::model::NewDevolucionItem {
                product: Some(day.products[idx].clone()),
                product_name: name,
                quantity: 1,
                unit_price: price.parse::<Decimal>().unwrap(),
                restock: true,
            }],
            metodo_reembolso: Some("efectivo".into()),
        };
        domain::sales::service::create_refund(&day.db, &day.tenant, Some(&day.user), body)
            .await
            .expect("refund");
        refunded_cash += *cash_paid;
    }
    refunded_cash
}

/// Best-effort POS latency probe (p50/p99) over the in-process sale path.
/// Driven through `domain::sales::service::post_sale` (the HTTP route is
/// BUG-001-gated). Prints the distribution; asserts only a generous ceiling so
/// it never flakes on CI — the numbers are the deliverable, not a hard SLA.
#[tokio::test]
async fn pos_sale_latency_probe() {
    let day = build_day().await;
    let warmup = 5usize;
    let n = 50usize;
    let mut samples: Vec<u128> = Vec::with_capacity(n);
    for s in 0..(warmup + n) {
        let idx = s % day.products.len();
        let price = price_of(idx).to_string();
        let name = format!("SKU {idx:02}");
        let lines = [SaleLine {
            product: &day.products[idx],
            name: &name,
            qty: 1,
            unit_price: &price,
        }];
        let start = std::time::Instant::now();
        let _ = seed_sale(
            &day.db,
            &day.tenant,
            Some(&day.user),
            "pos_cash",
            Some(&price),
            None,
            None,
            &lines,
        )
        .await;
        let micros = start.elapsed().as_micros();
        if s >= warmup {
            samples.push(micros); // discard cold-start/JIT warmup
        }
    }
    samples.sort_unstable();
    let p = |q: f64| samples[((samples.len() as f64 - 1.0) * q).round() as usize];
    let p50 = p(0.50) as f64 / 1000.0;
    let p99 = p(0.99) as f64 / 1000.0;
    let max = *samples.last().unwrap() as f64 / 1000.0;
    println!(
        "POS sale latency (in-process domain path, {warmup} warmup discarded, {n} samples): \
         p50={p50:.2}ms p99={p99:.2}ms max={max:.2}ms"
    );
    // Very generous ceiling — purely a smoke guard against pathological
    // regressions, NOT the <50ms p99 product budget (that targets the release
    // build on real hardware; this is a debug build with a tempfile DB).
    assert!(
        p99 < 1000.0,
        "POS p99 latency unexpectedly high: {p99:.2}ms"
    );
}

/// Σ stock_movement.delta grouped per product == product.stock for every SKU.
async fn assert_movements_match_stock(day: &Day) {
    #[derive(serde::Deserialize)]
    struct P {
        id: Thing,
        stock: i64,
    }
    let prods: Vec<P> = day
        .db
        .query("SELECT id, stock FROM product WHERE tenant = $t")
        .bind(("t", day.tenant.clone()))
        .await
        .unwrap()
        .take(0)
        .unwrap();
    for p in prods {
        #[derive(serde::Deserialize)]
        struct S {
            s: Option<i64>,
        }
        let mut q = day
            .db
            .query("SELECT math::sum(delta) AS s FROM stock_movement WHERE product = $p GROUP ALL")
            .bind(("p", p.id.clone()))
            .await
            .unwrap();
        let row: Option<S> = q.take(0).unwrap();
        let sum = row.and_then(|r| r.s).unwrap_or(0);
        assert_eq!(
            sum, p.stock,
            "Σ stock_movement.delta ({sum}) must equal product.stock ({}) for {}",
            p.stock, p.id
        );
    }
}

/// Σ batch stock per SKU == product.stock for every SKU.
async fn assert_batches_match_stock(day: &Day) {
    #[derive(serde::Deserialize)]
    struct P {
        id: Thing,
        stock: i64,
    }
    let prods: Vec<P> = day
        .db
        .query("SELECT id, stock FROM product WHERE tenant = $t")
        .bind(("t", day.tenant.clone()))
        .await
        .unwrap()
        .take(0)
        .unwrap();
    for p in prods {
        #[derive(serde::Deserialize)]
        struct S {
            s: Option<i64>,
        }
        let mut q = day
            .db
            .query(
                "SELECT math::sum(stock) AS s FROM product_batch \
                 WHERE product = $p GROUP ALL",
            )
            .bind(("p", p.id.clone()))
            .await
            .unwrap();
        let row: Option<S> = q.take(0).unwrap();
        let sum = row.and_then(|r| r.s).unwrap_or(0);
        assert_eq!(
            sum, p.stock,
            "Σ product_batch.stock ({sum}) must equal product.stock ({}) for {}",
            p.stock, p.id
        );
    }
}

#[tokio::test]
async fn full_pharmacy_day_invariants_hold() {
    let day = build_day().await;
    let app_free = api::build_router(state_free(day.db.clone()));
    let app_pro = api::build_router(state_pro(day.db.clone(), &["reports.margins_daily"]));

    let (orders, _gross_cash) = run_sales(&day).await;
    let refunded_cash = run_returns(&day, &orders).await;

    // --- stock integrity ---------------------------------------------------
    // Movement ledger (sale + return deltas) reconciles with product.stock.
    assert_movements_match_stock(&day).await;
    // NOTE: Σ batch == product.stock is BUG-007 (broken by refund restock) —
    // exercised in its own ignored test, not here.

    // --- near-expiry (HTTP, READ — not gated) ------------------------------
    let (st, body) = req_json(
        &app_free,
        "GET",
        "/api/v1/reports/near-expiry?days=30",
        Some(&day.token_free),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "near-expiry must be 200: {body}");
    let rows = body.as_array().expect("near-expiry array");
    assert!(!rows.is_empty(), "the 15-day urgent lots must surface");
    // Only the URG lots are within 30 days; MID/FAR are not.
    assert!(
        rows.iter()
            .all(|r| r["batch_code"].as_str().unwrap().ends_with("-URG")),
        "near-expiry must contain only the urgent (<30d) lots"
    );
    // Sorted soonest-first (ascending days_to_expiry).
    let days: Vec<i64> = rows
        .iter()
        .map(|r| r["days_to_expiry"].as_i64().unwrap())
        .collect();
    let mut sorted = days.clone();
    sorted.sort_unstable();
    assert_eq!(
        days, sorted,
        "near-expiry rows sorted ascending by days_to_expiry"
    );

    // --- margins-daily (HTTP, READ — Pro-gated) ----------------------------
    let (st, body) = req_json(
        &app_pro,
        "GET",
        "/api/v1/reports/margins-daily",
        Some(&day.token_pro),
        None,
        &[],
    )
    .await;
    assert_eq!(
        st,
        StatusCode::OK,
        "margins-daily (Pro) must be 200: {body}"
    );
    let mrows = body.as_array().expect("margins array");
    assert_eq!(
        mrows.len(),
        1,
        "all sales are 'today' → exactly one date bucket"
    );
    let m = &mrows[0];
    let revenue: Decimal = m["revenue"].as_str().unwrap().parse().unwrap();
    let cost: Decimal = m["cost"].as_str().unwrap().parse().unwrap();
    let margin: Decimal = m["margin"].as_str().unwrap().parse().unwrap();
    assert_eq!(margin, revenue - cost, "margin == revenue - cost");
    assert!(revenue > Decimal::ZERO, "non-trivial revenue");
    assert!(
        cost > Decimal::ZERO && cost < revenue,
        "cost positive and below revenue"
    );

    // Refunded orders MUST be excluded from margins. Cross-check: the report
    // revenue equals Σ order_item.subtotal over NON-refunded orders only.
    let expected_rev = non_refunded_revenue(&day).await;
    assert_eq!(
        revenue, expected_rev,
        "margins revenue must exclude refunded orders (got {revenue}, expected {expected_rev})"
    );

    // --- margins-daily is 402 on Free (gate sanity) ------------------------
    let (st_free, body_free) = req_json(
        &app_free,
        "GET",
        "/api/v1/reports/margins-daily",
        Some(&day.token_free),
        None,
        &[],
    )
    .await;
    assert_eq!(
        st_free,
        StatusCode::PAYMENT_REQUIRED,
        "Free tier must get 402 for margins-daily: {body_free}"
    );
    assert_eq!(body_free["error"]["code"], "FEATURE_REQUIRES_UPGRADE");

    // --- caja close arithmetic via arqueo (HTTP, READ — not gated) ---------
    let (st, arq) = req_json(
        &app_free,
        "GET",
        &format!("/api/v1/cash-sessions/{}/arqueo", day.session_id),
        Some(&day.token_free),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "arqueo must be 200: {arq}");
    let expected: Decimal = arq["session"]["closing_cash_expected"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let cash_sales: Decimal = arq["cash_sales"].as_str().unwrap().parse().unwrap();

    // expected == opening + cash_sales (no manual movements this day).
    let opening: Decimal = OPENING.parse().unwrap();
    assert_eq!(
        expected,
        opening + cash_sales,
        "expected cash == opening + cash_sales (no movements)"
    );

    // And cash_sales == Σ cash of NON-refunded orders == gross cash − refunded
    // orders' cash (the refund flips the whole order to 'refunded', dropping
    // its cash out of the rollup).
    let live_cash = non_refunded_cash(&day).await;
    assert_eq!(
        cash_sales, live_cash,
        "cash_sales must equal Σ cash of non-refunded orders"
    );
    assert!(
        refunded_cash > 0,
        "the 3 refunded orders had paid cash that must have dropped out"
    );
}

/// Σ order_item.subtotal over orders whose status != refunded/cancelled.
async fn non_refunded_revenue(day: &Day) -> Decimal {
    #[derive(serde::Deserialize)]
    struct O {
        id: Thing,
    }
    let live: Vec<O> = day
        .db
        .query("SELECT id FROM order WHERE tenant=$t AND status NOT IN ['refunded','cancelled']")
        .bind(("t", day.tenant.clone()))
        .await
        .unwrap()
        .take(0)
        .unwrap();
    let ids: Vec<Thing> = live.into_iter().map(|o| o.id).collect();
    #[derive(serde::Deserialize)]
    struct It {
        order: Thing,
        subtotal: Decimal,
    }
    let items: Vec<It> = day
        .db
        .query("SELECT order, subtotal FROM order_item WHERE tenant=$t AND order IN $ids")
        .bind(("t", day.tenant.clone()))
        .bind(("ids", ids.clone()))
        .await
        .unwrap()
        .take(0)
        .unwrap();
    let live_set: HashMap<String, ()> = ids.iter().map(|i| (i.to_string(), ())).collect();
    items
        .into_iter()
        .filter(|i| live_set.contains_key(&i.order.to_string()))
        .map(|i| i.subtotal)
        .sum()
}

/// BUG-007 (medium): after the day's 3 refunds (restock=true on batch-tracked
/// SKUs), `product.stock` is incremented but no `product_batch.stock` is
/// restored, so `Σ batch < product.stock` for the returned SKUs. Correct
/// behavior asserted here; ignored until the refund path returns units to a
/// lot (or otherwise keeps the batch sum reconciled).
#[tokio::test]
async fn batch_sum_matches_product_stock_after_returns() {
    let day = build_day().await;
    let (orders, _cash) = run_sales(&day).await;
    let _ = run_returns(&day, &orders).await;
    assert_batches_match_stock(&day).await;
}

/// Σ order.cash_amount over non-refunded pos_cash/pos_mixed orders.
async fn non_refunded_cash(day: &Day) -> Decimal {
    #[derive(serde::Deserialize)]
    struct S {
        s: Option<Decimal>,
    }
    let mut q = day
        .db
        .query(
            "SELECT math::sum(cash_amount) AS s FROM order \
             WHERE tenant=$t AND payment_method IN ['pos_cash','pos_mixed'] \
               AND status NOT IN ['refunded','cancelled'] GROUP ALL",
        )
        .bind(("t", day.tenant.clone()))
        .await
        .unwrap();
    let row: Option<S> = q.take(0).unwrap();
    row.and_then(|r| r.s).unwrap_or(Decimal::ZERO)
}

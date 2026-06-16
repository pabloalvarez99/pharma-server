//! Criterion harness for the **POS DB hot path** — the domain-service queries
//! a cashier hammers all day, measured at a realistic catalog scale (50k SKUs).
//!
//! Sibling to `crates/api/benches/pos_sale.rs` but a layer down: that one
//! drives the full axum HTTP stack against a *single* product to catch
//! routing/middleware regressions; this one drives the **domain service
//! functions directly** against a **50 000-product** catalog to catch the cost
//! that only shows up at scale — index lookups, table-scan aggregates, the
//! atomic sale tx.
//!
//! Engine is in-memory SurrealDB (`kv-mem`), same rationale as the api bench:
//! we measure CPU + query-planner + code-path cost, not disk fsync. Numbers are
//! an **upper-bound floor** for the on-disk `kv-surrealkv` build — if an op
//! doesn't fit the budget without a disk in the loop, it never will once
//! SurrealKv writes back. Validate the absolute number on the deploy VM; use
//! this bench to catch *regressions* between commits.
//!
//! Ops measured (the five that dominate a POS day), each one line:
//! - `lookup_by_barcode` — `product_barcode` unique-index scan (scan-gun path).
//! - `lookup_by_sku` — `product.external_id` index lookup (`find_id_by_external_id`).
//! - `stock_stats_agg` — `catalog::service::stats`: count()/math::sum over all 50k.
//! - `cierre_caja_agg` — `cash_register::service::compute_summary`: aggregate close.
//! - `post_sale_insert` — `sales::service::post_sale`: full atomic sale (the only
//!   write; registered LAST so its row growth never pollutes the read benches).
//!
//! Budget (CLAUDE.md § "Performance budget"): POS endpoints <50ms p99 on the
//! minimum target box (i3 + SSD + 8GB). Criterion reports mean/median but NOT
//! p99, so on top of the Criterion groups this bench runs a **manual percentile
//! pass** (p50/p95/p99 per op) and prints a table + an explicit budget verdict
//! to stderr before the Criterion measurements start.
//!
//! Tunables (env, all optional):
//!   PHARMA_BENCH_PRODUCTS  catalog size           (default 50000)
//!   PHARMA_BENCH_SALES     seeded cash sales for the cierre aggregate (def 800)
//!   PHARMA_BENCH_SAMPLES   iterations per manual percentile op        (def 2000)
//!
//! Smoke the harness fast (tiny dataset, no measurement):
//!   PHARMA_BENCH_PRODUCTS=500 cargo bench -p domain --bench pos_hotpath -- --test
//!
//! Real run (writes target/criterion/ + prints the percentile table):
//!   cargo bench -p domain --bench pos_hotpath
//!
//! Regression gate (opt-in, default OFF so a known-OVER op can be measured
//! before/after a fix without the harness panicking):
//!   PHARMA_BENCH_GATE=1                 fail if ANY op breaks the <50ms p99 budget
//!   PHARMA_BENCH_GATE=cierre_caja_agg   fail only if that op does (comma-list for >1)
//! e.g. guard BUG-perf-005 post-fix:
//!   PHARMA_BENCH_GATE=cierre_caja_agg cargo bench -p domain --bench pos_hotpath

use std::time::{Duration, Instant};

use chrono::Utc;
use criterion::{criterion_group, criterion_main, Criterion};
use rust_decimal::Decimal;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;
use tokio::runtime::Runtime;

use domain::cash_register::{model::*, service as cash};
use domain::catalog::{repo as catalog_repo, service as catalog};
use domain::sales::{model::*, service as sales};

type Db = Surreal<surrealdb::engine::local::Db>;

/// POS endpoints must stay under this on the minimum target hardware.
const BUDGET_P99: Duration = Duration::from_millis(50);
const UNIT_PRICE: i64 = 1500;
const COST_PRICE: i64 = 500;
/// Per-product stock cushion — Criterion + the manual pass run tens of
/// thousands of single-unit sales; we never want INSUFFICIENT_STOCK to hide
/// the real cost.
const SEED_STOCK: i64 = 100_000_000;

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Everything the benches need, built once.
struct Harness {
    db: Db,
    tenant: Thing,
    /// A product Thing that exists + has stock (the sale target).
    target_product: String,
    /// The barcode mapped to `target_product`.
    target_barcode: String,
    /// An external_id (SKU) that exists in the catalog.
    target_sku: String,
    /// Open cash session id holding the seeded cash sales.
    session_id: String,
}

/// Bulk-seed the catalog with `n` products + a 1:1 `product_barcode` row each.
///
/// Seeding 50k rows through `catalog::service::create_product` would run a
/// slug-uniqueness probe per row (~tens of seconds). This is *setup*, not a
/// measured path, so we bulk-`INSERT` in chunks instead. Record-id tokens
/// (`tenant:…`, `product:…`) and the deterministic name/slug/sku/barcode
/// strings are fully controlled by this function (no external input), so
/// interpolating them into the statement is safe; the money operands are
/// `.bind()`-ed and type-checked by the schema.
async fn seed_catalog(db: &Db, tenant: &Thing, n: usize) -> Vec<Thing> {
    const CHUNK: usize = 1000;
    let tok = tenant.to_string(); // "tenant:xxxx"

    let mut i = 0usize;
    while i < n {
        let end = (i + CHUNK).min(n);
        let mut rows = String::with_capacity(CHUNK * 96);
        rows.push('[');
        for j in i..end {
            if j > i {
                rows.push(',');
            }
            // name/slug/external_id are deterministic ascii — safe to inline.
            rows.push_str(&format!(
                "{{tenant:{tok},name:'P{j:07}',slug:'p{j:07}',\
                 price:$price,cost_price:$cost,stock:{SEED_STOCK},\
                 external_id:'SKU{j:07}'}}"
            ));
        }
        rows.push(']');
        // `rust_decimal`'s `serde-with-str` feature serializes `Decimal` as a
        // string, which the schema's `decimal` field rejects. Bind the surreal
        // numeric value directly (same as the domain repos' `dec_val` helper).
        let price: surrealdb::sql::Value =
            surrealdb::sql::Number::from(Decimal::from(UNIT_PRICE)).into();
        let cost: surrealdb::sql::Value =
            surrealdb::sql::Number::from(Decimal::from(COST_PRICE)).into();
        db.query(format!("INSERT INTO product {rows}"))
            .bind(("price", price))
            .bind(("cost", cost))
            .await
            .expect("bulk insert product")
            .check()
            .expect("product insert ok");
        i = end;
    }

    // Pull the generated ids; order is irrelevant — we map barcodes by index.
    let mut r = db
        .query("SELECT VALUE id FROM product WHERE tenant=$t")
        .bind(("t", tenant.clone()))
        .await
        .expect("select product ids");
    let ids: Vec<Thing> = r.take(0).expect("product ids");
    assert_eq!(ids.len(), n, "seeded product count mismatch");

    // 1:1 barcode table (so the unique-index lookup hits a realistically sized
    // tree, not a 1-row table).
    let mut k = 0usize;
    while k < n {
        let end = (k + CHUNK).min(n);
        let mut rows = String::with_capacity(CHUNK * 64);
        rows.push('[');
        for (off, prod) in ids[k..end].iter().enumerate() {
            if off > 0 {
                rows.push(',');
            }
            let j = k + off;
            rows.push_str(&format!(
                "{{tenant:{tok},product:{prod},barcode:'BC{j:07}'}}"
            ));
        }
        rows.push(']');
        db.query(format!("INSERT INTO product_barcode {rows}"))
            .await
            .expect("bulk insert barcode")
            .check()
            .expect("barcode insert ok");
        k = end;
    }

    ids
}

async fn build_harness() -> Harness {
    let n_products = env_usize("PHARMA_BENCH_PRODUCTS", 50_000);
    let n_sales = env_usize("PHARMA_BENCH_SALES", 800);

    let db: Db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("bench").use_db("bench").await.unwrap();
    db::run_embedded(&db).await.expect("migrations");

    // Tenant + admin user (roles unused by the domain layer, set for realism).
    let mut r = db
        .query("CREATE tenant SET name='Bench', slug='bench' RETURN id")
        .await
        .unwrap();
    let tenant: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().expect("tenant");
    let mut r = db
        .query(
            "CREATE user SET tenant=$t, email='bench@local', password='x', \
             roles=['admin'] RETURN id",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let admin: Thing = r.take::<Option<Thing>>((0, "id")).unwrap().expect("admin");

    let ids = seed_catalog(&db, &tenant, n_products).await;
    let mid = n_products / 2;
    let target_product = ids[mid].to_string();
    let target_barcode = format!("BC{mid:07}");
    let target_sku = format!("SKU{mid:07}");

    // Open a cash session and seed `n_sales` real cash sales into it so the
    // cierre aggregate scans a realistic day's order volume. These run BEFORE
    // any measurement, so their created_at falls inside the session window the
    // cierre bench reads; the post_sale bench (registered last) writes only
    // AFTER the cierre bench has finished measuring.
    let session = cash::open_session(
        &db,
        &tenant,
        &admin,
        OpenSessionInput {
            register_name: "caja-bench".into(),
            opening_cash: Decimal::from(50_000),
            notes: None,
        },
    )
    .await
    .expect("open session");

    for _ in 0..n_sales {
        sales::post_sale(
            &db,
            &tenant,
            None,
            None,
            None,
            single_item_sale(&target_product),
        )
        .await
        .expect("seed cash sale");
    }
    // A few movements so sum_movements isn't trivially empty.
    for _ in 0..5 {
        cash::add_movement(
            &db,
            &tenant,
            Some(&admin),
            &session.id,
            CashMovementInput {
                tipo: "ingreso".into(),
                amount: Decimal::from(1000),
                reason: "fondo".into(),
            },
        )
        .await
        .expect("movement");
    }

    Harness {
        db,
        tenant,
        target_product,
        target_barcode,
        target_sku,
        session_id: session.id,
    }
}

fn single_item_sale(product_id: &str) -> PosSaleRequest {
    PosSaleRequest {
        items: vec![PosSaleItem {
            product: product_id.to_string(),
            product_name: "Paracetamol 500".into(),
            quantity: 1,
            unit_price: Decimal::from(UNIT_PRICE),
        }],
        payment_method: "pos_cash".into(),
        cash_amount: Some(Decimal::from(UNIT_PRICE)),
        card_amount: None,
        discount: None,
        customer: None,
        customer_name: None,
        customer_phone: None,
        notes: None,
        external_ref: None,
        prescriptions: Vec::new(),
    }
}

// --- the five hot-path ops, as closures the harness reuses for both the
//     manual percentile pass and the Criterion measurement ---------------------

async fn op_lookup_barcode(h: &Harness) {
    let mut r = h
        .db
        .query("SELECT VALUE product FROM product_barcode WHERE tenant=$t AND barcode=$b LIMIT 1")
        .bind(("t", h.tenant.clone()))
        .bind(("b", h.target_barcode.clone()))
        .await
        .expect("barcode query");
    let hit: Option<Thing> = r.take(0).expect("take barcode");
    debug_assert!(hit.is_some(), "barcode must resolve");
}

async fn op_lookup_sku(h: &Harness) {
    let hit = catalog_repo::find_id_by_external_id(&h.db, &h.tenant, &h.target_sku)
        .await
        .expect("sku lookup");
    debug_assert!(hit.is_some(), "sku must resolve");
}

async fn op_stock_stats(h: &Harness) {
    let s = catalog::stats(&h.db, &h.tenant).await.expect("stats");
    debug_assert!(s.total > 0);
}

async fn op_cierre_caja(h: &Harness) {
    let _ = cash::compute_summary(&h.db, &h.tenant, &h.session_id, Utc::now())
        .await
        .expect("compute_summary");
}

async fn op_post_sale(h: &Harness) {
    sales::post_sale(
        &h.db,
        &h.tenant,
        None,
        None,
        None,
        single_item_sale(&h.target_product),
    )
    .await
    .expect("post_sale");
}

/// Manual percentile pass: run `op` `samples` times, sort the latencies, print
/// p50/p95/p99 + a budget verdict. Criterion gives mean/median for regression
/// tracking but not p99, which is the figure CLAUDE.md's budget is stated in.
fn percentiles(name: &str, samples: usize, mut run: impl FnMut() -> Duration) -> bool {
    let mut lat: Vec<Duration> = Vec::with_capacity(samples);
    for _ in 0..samples {
        lat.push(run());
    }
    lat.sort_unstable();
    let pick = |p: f64| lat[((samples as f64 * p) as usize).min(samples - 1)];
    let p50 = pick(0.50);
    let p95 = pick(0.95);
    let p99 = pick(0.99);
    let ok = p99 <= BUDGET_P99;
    eprintln!(
        "  {name:<20} p50={:>8.3}ms  p95={:>8.3}ms  p99={:>8.3}ms  budget(<50ms p99)={}",
        p50.as_secs_f64() * 1e3,
        p95.as_secs_f64() * 1e3,
        p99.as_secs_f64() * 1e3,
        if ok { "OK" } else { "*** OVER ***" }
    );
    ok
}

fn bench_pos_hotpath(c: &mut Criterion) {
    let rt = Runtime::new().expect("tokio runtime");
    let h = rt.block_on(build_harness());
    let samples = env_usize("PHARMA_BENCH_SAMPLES", 2000);

    let n = rt.block_on(async { catalog::stats(&h.db, &h.tenant).await.expect("stats").total });
    eprintln!("\n=== POS DB hot-path percentiles (catalog = {n} products, kv-mem) ===");

    // Manual percentile pass — reads first, the single write op LAST so its row
    // growth can't inflate the cierre/stats scans above it. Capture each op's
    // pass/fail by name so the opt-in regression gate below can name the breacher.
    let results: Vec<(&str, bool)> = vec![
        (
            "lookup_by_barcode",
            percentiles("lookup_by_barcode", samples, || {
                let s = Instant::now();
                rt.block_on(op_lookup_barcode(&h));
                s.elapsed()
            }),
        ),
        (
            "lookup_by_sku",
            percentiles("lookup_by_sku", samples, || {
                let s = Instant::now();
                rt.block_on(op_lookup_sku(&h));
                s.elapsed()
            }),
        ),
        (
            "stock_stats_agg",
            percentiles("stock_stats_agg", samples, || {
                let s = Instant::now();
                rt.block_on(op_stock_stats(&h));
                s.elapsed()
            }),
        ),
        (
            "cierre_caja_agg",
            percentiles("cierre_caja_agg", samples, || {
                let s = Instant::now();
                rt.block_on(op_cierre_caja(&h));
                s.elapsed()
            }),
        ),
        (
            "post_sale_insert",
            percentiles("post_sale_insert", samples, || {
                let s = Instant::now();
                rt.block_on(op_post_sale(&h));
                s.elapsed()
            }),
        ),
    ];
    let all_ok = results.iter().all(|(_, ok)| *ok);
    eprintln!(
        "=== budget verdict: {} ===\n",
        if all_ok {
            "PASS — every op p99 < 50ms"
        } else {
            "FAIL — see *** OVER *** above (file in BUG LOG)"
        }
    );

    // Opt-in regression gate. By default the bench only PRINTS the verdict and
    // exits 0, so a known-OVER op (e.g. BUG-perf-005 cierre_caja_agg pre-fix) can
    // still be MEASURED before/after a fix without the harness panicking. Set
    // PHARMA_BENCH_GATE to turn the budget into a HARD failure — the post-fix /
    // CI regression guard:
    //   PHARMA_BENCH_GATE=1                 gate every op
    //   PHARMA_BENCH_GATE=all               same as 1
    //   PHARMA_BENCH_GATE=cierre_caja_agg   gate only that op (comma-list for >1)
    // The op names match the table above (and the Criterion group below).
    if let Ok(gate) = std::env::var("PHARMA_BENCH_GATE") {
        let gate = gate.trim();
        let gate_all = gate == "1" || gate.eq_ignore_ascii_case("all");
        let gated: Vec<&str> = results
            .iter()
            .filter(|(name, ok)| !*ok && (gate_all || gate.split(',').any(|g| g.trim() == *name)))
            .map(|(name, _)| *name)
            .collect();
        assert!(
            gated.is_empty(),
            "PHARMA_BENCH_GATE regression guard: op(s) over the <50ms p99 budget: {gated:?} \
             (see the *** OVER *** rows above; file in teamwork_op.txt BUG LOG)"
        );
    }

    // Criterion measurement (for `--save-baseline` regression diffs). Same
    // order: reads, then the write.
    let mut group = c.benchmark_group("pos_hotpath");
    group.measurement_time(Duration::from_secs(8));

    group.bench_function("lookup_by_barcode", |b| {
        b.to_async(&rt).iter(|| op_lookup_barcode(&h));
    });
    group.bench_function("lookup_by_sku", |b| {
        b.to_async(&rt).iter(|| op_lookup_sku(&h));
    });
    group.bench_function("stock_stats_agg", |b| {
        b.to_async(&rt).iter(|| op_stock_stats(&h));
    });
    group.bench_function("cierre_caja_agg", |b| {
        b.to_async(&rt).iter(|| op_cierre_caja(&h));
    });
    group.bench_function("post_sale_insert", |b| {
        b.to_async(&rt).iter(|| op_post_sale(&h));
    });

    group.finish();
}

criterion_group!(benches, bench_pos_hotpath);
criterion_main!(benches);

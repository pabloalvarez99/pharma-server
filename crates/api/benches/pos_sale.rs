//! Criterion harness for the POS hot path — `POST /api/v1/pos/sale`.
//!
//! Background: per `CLAUDE.md` § "Performance budget" the POS endpoints must
//! stay <50ms p99 on the minimum target hardware (i3 + SSD + 8GB). This bench
//! exercises the full HTTP stack the same way our integration tests do —
//! `tower::ServiceExt::oneshot` against `api::build_router` — so we measure
//! axum routing + middleware (auth, audit, role) + handler + the domain layer
//! against the **in-memory** SurrealDB engine (`kv-mem`).
//!
//! Why kv-mem and not the real `kv-surrealkv`? We want to measure CPU + code
//! path overhead, not disk fsync. `kv-surrealkv` is faster than kv-mem in
//! steady state once warm, but disk variance pollutes the numbers. The numbers
//! here are an **upper-bound floor**: if we don't fit under budget without any
//! disk in the loop, we never will once SurrealKv writes back to disk. Use a
//! real-disk smoke (the deploy VM) to validate the absolute number; use this
//! bench to catch *regressions* between commits.
//!
//! Telemetry: no `tracing_subscriber::init()` here — Criterion forks a fresh
//! process per `cargo bench` invocation, so without an installed subscriber
//! the `tracing` macros short-circuit to ~0ns.
//!
//! Smoke-test the harness without taking measurements:
//! `cargo bench -p api --bench pos_sale -- --test`
//!
//! Real run (writes to `target/criterion/`):
//! `cargo bench -p api --bench pos_sale`

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use criterion::{criterion_group, criterion_main, Criterion};
use http_body_util::BodyExt;
use rust_decimal::Decimal;
use std::str::FromStr;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;
use tokio::runtime::Runtime;
use tower::ServiceExt;

use domain::catalog::{model::NewProduct, service as catalog};
use pharma_core::config::JwtConfig;

const PRODUCT_NAME: &str = "Paracetamol 500";
const UNIT_PRICE: &str = "1500";
/// Generous stock cushion — Criterion may run thousands of iterations per
/// sample; we want zero "insufficient stock" 4xx hiding the real cost.
const SEED_STOCK: i64 = 10_000_000;

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "bench-secret-do-not-ship".into(),
        issuer: "pharma-bench".into(),
        ttl_seconds: 3600,
    }
}

struct Harness {
    /// Pre-built axum service — cloned per iteration (`Router` is cheap to
    /// clone; it's `Arc`-internally).
    router: axum::Router,
    /// Bearer header value `Bearer <jwt>` — built once.
    bearer: String,
    /// Serialized `PosSaleRequest` JSON — built once, cloned per iteration.
    body_bytes: Vec<u8>,
    /// Path to hit. Constant; kept here for read-path bench reuse below.
    pos_uri: &'static str,
    inventory_uri: &'static str,
    products_uri: &'static str,
}

async fn build_harness() -> Harness {
    // 1. In-memory SurrealDB + run all embedded migrations. `db::run_embedded`
    //    uses the `include_dir!` blob baked into the `db` crate at compile
    //    time, so the bench works no matter where it's run from.
    let db: Surreal<surrealdb::engine::local::Db> = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("bench").use_db("bench").await.unwrap();
    db::run_embedded(&db).await.expect("migrations");

    // 2. Tenant + admin user. Roles include "admin" so the role layer on
    //    /pos/sale (admin|owner|cashier) admits us.
    let mut r = db
        .query("CREATE tenant SET name='Bench', slug='bench' RETURN id")
        .await
        .unwrap();
    let tenant: Thing = r
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .expect("tenant id");
    let mut r = db
        .query(
            "CREATE user SET tenant=$t, email='bench@local', password='x', \
             roles=['admin'] RETURN id",
        )
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let user: Thing = r
        .take::<Option<Thing>>((0, "id"))
        .unwrap()
        .expect("user id");

    // 3. Product with huge stock so the inner sale never trips INSUFFICIENT_STOCK.
    let product = catalog::create_product(
        &db,
        &tenant,
        NewProduct {
            name: PRODUCT_NAME.into(),
            slug: None,
            description: None,
            price: Decimal::from_str(UNIT_PRICE).unwrap(),
            cost_price: Some(Decimal::from_str("500").unwrap()),
            stock: SEED_STOCK,
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
        },
    )
    .await
    .expect("seed product");

    // 4. JWT — bypass /login to keep the bench narrow (argon2 verify isn't
    //    what we're measuring).
    let jwt = jwt_cfg();
    let token = auth::issue(
        &jwt,
        &user.to_string(),
        &tenant.to_string(),
        vec!["admin".to_string()],
    )
    .expect("issue jwt");
    let bearer = format!("Bearer {token}");

    // 5. AppState mirrors the integration-test layout exactly. No node
    //    identity, no license path, no metrics token, no data dir → backup
    //    endpoints inert, license defaults to Free.
    let state = api::AppState {
        started_at: chrono::Utc::now(),
        jwt,
        db: Some(Arc::new(db)),
        metrics_token: None,
        node_identity: None,
        data_dir: None,
        license: Arc::new(arc_swap::ArcSwap::from_pointee(
            license::License::free_default(uuid::Uuid::nil()),
        )),
        license_path: None,
        rate_limit: None,
        docs_enabled: true,
        public_catalog: pharma_core::config::PublicCatalogConfig::default(),
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
        stock_webhook: std::sync::Arc::new(pharma_core::config::StockWebhookConfig::default()),
    };
    let router = api::build_router(state);

    // 6. Body: single-item happy path. `unit_price` is `Decimal` w/ str serde,
    //    so the JSON value MUST be a string.
    let body = serde_json::json!({
        "items": [{
            "product": product.id,
            "product_name": product.name,
            "quantity": 1,
            "unit_price": UNIT_PRICE,
        }],
        "payment_method": "pos_cash",
        "cash_amount": "1500",
    });
    let body_bytes = serde_json::to_vec(&body).unwrap();

    // Sanity: prove the harness actually returns 2xx before Criterion starts
    // recording. Catches model drift early.
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/pos/sale")
                .header("content-type", "application/json")
                .header("authorization", &bearer)
                .body(Body::from(body_bytes.clone()))
                .unwrap(),
        )
        .await
        .expect("oneshot");
    let status = resp.status();
    if status != StatusCode::CREATED {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        panic!(
            "pos/sale smoke failed: {status} {}",
            String::from_utf8_lossy(&bytes)
        );
    }

    // Also smoke-test the read paths the optional benches hit.
    for uri in ["/api/v1/inventory", "/api/v1/products?limit=100"] {
        let resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(uri)
                    .header("authorization", &bearer)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("oneshot read");
        assert_eq!(resp.status(), StatusCode::OK, "{uri} not 200");
    }

    Harness {
        router,
        bearer,
        body_bytes,
        pos_uri: "/api/v1/pos/sale",
        inventory_uri: "/api/v1/inventory",
        products_uri: "/api/v1/products?limit=100",
    }
}

fn pos_sale_bench(c: &mut Criterion) {
    let rt = Runtime::new().expect("tokio runtime");
    let h = rt.block_on(build_harness());

    let mut group = c.benchmark_group("pos_sale");
    // ~50 samples is the Criterion default; bump iteration target slightly so
    // the t-test has signal without dragging dev loops out.
    group.measurement_time(Duration::from_secs(8));

    group.bench_function("single_item_cash_happy_path", |b| {
        b.to_async(&rt).iter(|| {
            // Each iteration clones the cheap pre-built parts. The router is
            // an Arc-tree internally; cloning a `Vec<u8>` body is one alloc.
            let router = h.router.clone();
            let body = h.body_bytes.clone();
            let bearer = h.bearer.clone();
            let uri = h.pos_uri;
            async move {
                let resp = router
                    .oneshot(
                        Request::builder()
                            .method("POST")
                            .uri(uri)
                            .header("content-type", "application/json")
                            .header("authorization", bearer)
                            .body(Body::from(body))
                            .unwrap(),
                    )
                    .await
                    .expect("oneshot");
                // Drain body so the response future fully completes — without
                // this we'd skip the JSON serialization of the response, which
                // is part of the latency budget.
                let status = resp.status();
                let _ = resp.into_body().collect().await.unwrap();
                debug_assert_eq!(status, StatusCode::CREATED);
            }
        });
    });

    group.finish();
}

fn reads_bench(c: &mut Criterion) {
    let rt = Runtime::new().expect("tokio runtime");
    let h = rt.block_on(build_harness());

    let mut group = c.benchmark_group("reads");
    group.measurement_time(Duration::from_secs(6));

    let bench_read =
        |group: &mut criterion::BenchmarkGroup<'_, _>, name: &'static str, uri: &'static str| {
            let router = h.router.clone();
            let bearer = h.bearer.clone();
            group.bench_function(name, |b| {
                b.to_async(&rt).iter(|| {
                    let router = router.clone();
                    let bearer = bearer.clone();
                    async move {
                        let resp = router
                            .oneshot(
                                Request::builder()
                                    .method("GET")
                                    .uri(uri)
                                    .header("authorization", bearer)
                                    .body(Body::empty())
                                    .unwrap(),
                            )
                            .await
                            .expect("oneshot");
                        let status = resp.status();
                        let _ = resp.into_body().collect().await.unwrap();
                        debug_assert_eq!(status, StatusCode::OK);
                    }
                });
            });
        };

    bench_read(&mut group, "GET_inventory", h.inventory_uri);
    bench_read(&mut group, "GET_products_limit_100", h.products_uri);

    group.finish();
}

criterion_group!(benches, pos_sale_bench, reads_bench);
criterion_main!(benches);

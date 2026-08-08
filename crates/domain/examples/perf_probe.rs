//! Ad-hoc probe for query plans and latency against a real SurrealKV directory.
//! Takes the queries on the command line so a hypothesis can be tested without
//! rebuilding. `$t` is bound to the tenant of `--tenant`.
//!
//!   cargo run --release -p domain --example perf_probe -- <db-path> <slug> "<sql>" ["<sql>" …]
//!
//! Prints, per query: the plan of every run (to catch a planner that picks
//! different plans for the same SQL) and the full timing series. Run with the
//! server stopped (SurrealKV is single-writer), against a COPY if the query writes.

use std::collections::BTreeSet;
use std::time::Instant;

use surrealdb::engine::local::SurrealKv;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

const RUNS: usize = 20;

fn plan_of(explain: &str) -> String {
    if explain.contains("Iterate Table") {
        return "Iterate Table".to_string();
    }
    match explain.find("index: '") {
        Some(i) => {
            let rest = &explain[i + 8..];
            let name = &rest[..rest.find('\'').unwrap_or(0)];
            let kind = if explain.contains("from:") {
                "range"
            } else {
                "eq"
            };
            format!("{name} ({kind})")
        }
        None => "no index".to_string(),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: perf_probe <db-path> <slug> <sql>…");
    let slug = args.next().expect("tenant slug");
    let queries: Vec<String> = args.collect();
    anyhow::ensure!(!queries.is_empty(), "pass at least one query");

    let db = Surreal::new::<SurrealKv>(path.as_str()).await?;
    db.use_ns("pharma").use_db("main").await?;

    let mut r = db
        .query("SELECT id FROM tenant WHERE slug = $s LIMIT 1")
        .bind(("s", slug))
        .await?;
    let tenant: Thing = r.take::<Option<Thing>>((0, "id"))?.expect("tenant");

    for q in &queries {
        // Same SQL, RUNS times: collect every plan seen and every timing.
        let mut plans: BTreeSet<String> = BTreeSet::new();
        let mut times = Vec::with_capacity(RUNS);
        let mut rows_seen = 0usize;
        for _ in 0..RUNS {
            let explain: surrealdb::Value = db
                .query(format!("{q} EXPLAIN"))
                .bind(("t", tenant.clone()))
                .await?
                .take(0)?;
            plans.insert(plan_of(&explain.to_string()));

            let t0 = Instant::now();
            let mut res = db.query(q.as_str()).bind(("t", tenant.clone())).await?;
            let rows: surrealdb::Value = res.take(0)?;
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
            rows_seen = match rows.into_inner() {
                surrealdb::sql::Value::Array(a) => a.len(),
                surrealdb::sql::Value::None => 0,
                _ => 1,
            };
        }
        let mut sorted = times.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("\n{q}");
        println!("  plans seen: {:?}", plans);
        println!("  rows: {rows_seen}");
        println!(
            "  min={:.1}  median={:.1}  max={:.1}",
            sorted[0],
            sorted[RUNS / 2],
            sorted[RUNS - 1]
        );
        println!(
            "  series: {}",
            times
                .iter()
                .map(|t| format!("{t:.0}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    Ok(())
}

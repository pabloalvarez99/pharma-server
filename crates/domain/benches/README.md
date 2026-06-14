# POS DB hot-path bench (`pos_hotpath`)

Criterion harness for the database operations a cashier hammers all day,
measured at **realistic catalog scale (50 000 SKUs)** against an in-memory
SurrealDB (`kv-mem`).

Sibling to `crates/api/benches/pos_sale.rs`, one layer down: that bench drives
the full axum HTTP stack against a *single* product to catch routing/middleware
regressions; this one drives the **domain service functions directly** against a
50k-product catalog to catch the cost that only appears at scale — index
lookups, table-scan aggregates, the atomic sale transaction.

## Ops measured

| bench id           | domain call                                  | what it stresses                         |
|--------------------|----------------------------------------------|------------------------------------------|
| `lookup_by_barcode`| `product_barcode` unique-index scan          | scan-gun path (`SELECT VALUE product …`) |
| `lookup_by_sku`    | `catalog::repo::find_id_by_external_id`       | `product.external_id` index lookup        |
| `stock_stats_agg`  | `catalog::service::stats`                      | `count()`/`math::sum` over all 50k rows  |
| `cierre_caja_agg`  | `cash_register::service::compute_summary`      | aggregate cash close (sales+movements)   |
| `post_sale_insert` | `sales::service::post_sale`                    | full atomic sale (stock check + write)   |

`post_sale_insert` is the only write op and is registered **last** so its row
growth never pollutes the read benches above it. `cierre_caja_agg` aggregates
over a seeded day of cash sales whose timestamps fall inside the session window
*before* any benchmarked write happens.

> Note: "stock por bodega" from the lane brief maps to `stock_stats_agg` — the
> schema has **no warehouse/bodega split** (stock is a single `int` per product
> plus FEFO `product_batch` lots), so the realistic "stock at scale" read is the
> catalog aggregate.

## Why `kv-mem`

We measure CPU + query-planner + code-path cost, not disk fsync. The numbers are
an **upper-bound floor** for the shipped on-disk `kv-surrealkv` build: if an op
doesn't fit the budget without a disk in the loop, it never will once SurrealKv
writes back. Validate the *absolute* number on the deploy VM; use this bench to
catch *regressions* between commits.

## Performance budget

CLAUDE.md § "Performance budget": POS endpoints **< 50 ms p99** on the minimum
target box (i3 + SSD + 8 GB). Criterion reports mean/median but **not p99**, so
the bench runs a **manual percentile pass** (p50/p95/p99 per op) and prints a
table + an explicit budget verdict to stderr before the Criterion groups run:

```
=== POS DB hot-path percentiles (catalog = 50000 products, kv-mem) ===
  lookup_by_barcode    p50=   0.0xxms  p95=   0.0xxms  p99=   0.0xxms  budget(<50ms p99)=OK
  ...
=== budget verdict: PASS — every op p99 < 50ms ===
```

An op over budget is filed in `teamwork_op.txt` BUG LOG (the lane measures; it
does not silently "fix" by weakening a budget).

## Run

```bash
# Real run: writes target/criterion/ + prints the percentile table.
cargo bench -p domain --bench pos_hotpath

# Fast harness smoke (tiny dataset, no measurement) — for CI/dev sanity.
PHARMA_BENCH_PRODUCTS=500 cargo bench -p domain --bench pos_hotpath -- --test

# Regression diff between two commits:
git checkout main   && cargo bench -p domain --bench pos_hotpath -- --save-baseline before
git checkout HEAD@{1} && cargo bench -p domain --bench pos_hotpath -- --baseline before
```

GitHub Actions billing is locked on this repo (`.github/workflows/bench.yml` is
`workflow_dispatch`-only), so this bench is a **local gate**, not a CI gate.

## Tunables (env, optional)

| var                     | meaning                                  | default |
|-------------------------|------------------------------------------|---------|
| `PHARMA_BENCH_PRODUCTS` | catalog size                             | 50000   |
| `PHARMA_BENCH_SALES`    | seeded cash sales for the cierre agg     | 800     |
| `PHARMA_BENCH_SAMPLES`  | iterations per manual percentile op      | 2000    |

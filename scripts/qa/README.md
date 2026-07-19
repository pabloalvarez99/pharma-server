# POS runtime QA — live-backend cashier-day harness

Drives a **real cashier day** against a live `pharma-api` server (not vitest unit
journeys). Catches runtime bugs the pure-logic tests cannot: SurrealDB schema
asserts, 500s, wrong HTTP status codes, payload-shape mismatches between the Tauri
client and the backend.

## What it does

`pos-runtime-qa.sh [pharmacy|minimarket]` runs the full counter loop end-to-end:

1. `pharma migrate` → fresh schema in a throwaway tempdir DB
2. `pharma tenant-create` + `pharma user-create` (owner) + `pharma seed-demo`
3. boots `pharma-api` on `127.0.0.1:8099`, waits for `/health/live`
4. `POST /api/v1/login` → JWT
5. `GET /api/v1/products` → pick a seeded SKU
6. `POST /api/v1/cash-sessions` → **open caja**
7. `POST /api/v1/pos/sale` (cash, with change) → **venta**
8. `GET /api/v1/orders/{id}/receipt` → **boleta/ticket**
9. `POST /api/v1/dte/boletas` → **boleta SII** (no CAF → must 4xx clean, not 5xx)
10. `POST /api/v1/pos/returns` → **devolución** parcial (restock)
11. probe: OLD client payload `tipo=total` (documents BUG-paul-001)
12. `GET /api/v1/cash-sessions/{id}/arqueo` → **arqueo**
13. `POST /api/v1/cash-sessions/{id}/close` → **cierre**

Prints `PASS:`/`FAIL:` per step; first hard failure exits non-zero.

## Prereqs

```bash
# from the worktree root
cargo build -p api -p cli --bins      # produces target/debug/{pharma,pharma-api}.exe
```

Also needs `curl` + `jq` on PATH (Git Bash ships both on this box).

## Run

```bash
bash scripts/qa/pos-runtime-qa.sh pharmacy
bash scripts/qa/pos-runtime-qa.sh minimarket
```

## `pos-payments-fidelidad-qa.sh` — deepened cashier-loop edges (ola 5)

Companion harness that covers the payment/discount/loyalty edges
`pos-runtime-qa.sh` does **not**, against the same live stack:

- **Multi-tender** (`pos_mixed`, efectivo + tarjeta): exact split persists
  `cash_amount`/`card_amount`; underpay (`cash+card < total`) → 4xx; overpay
  probes `receipt.change`.
- **Descuento global**: `total == subtotal - discount`; over-discount clamps
  total to `>= 0` (never negative).
- **Descuento por línea**: cashier-adjusted `unit_price` flows into the subtotal.
- **Cliente + fidelidad**: points awarded `== floor(total/regla)`,
  `customer.loyalty_points` bumped exactly, loyalty ledger row written.
- **Devolución parcial + restock + RE-VENTA** del ítem devuelto: asserts
  `stock` after each step (`-3 → -2 → -3`) — catches FEFO/stock desync.

```bash
bash scripts/qa/pos-payments-fidelidad-qa.sh pharmacy
bash scripts/qa/pos-payments-fidelidad-qa.sh minimarket
```

Runs **all** scenarios (no exit-on-first-fail) so multiple bugs surface in one
run; exits non-zero if any hard assertion failed. Sale POSTs accept `200`/`201`.
Both verticals must end `FAILS=0`.

Each run uses its own `mktemp` DB dir and kills the server on exit — repeatable,
no shared `./data/surreal` lock with the dev service.

## Notes

- Port override: `PORT=9000 bash scripts/qa/pos-runtime-qa.sh`.
- Both verticals must pass. Minimarket must sell + return **without** any receta
  / controlado step (multi-rubro invariant).
- CI is billing-walled → this is a **local** gate, same as the client e2e harness.

# E2E harness — `npm run e2e`

Repeatable, local, end-to-end check that a real Chilean operator can run the free
core through a **real build** of `pharma-api` against a **real SurrealDB**, in
**both verticals** (farmacia + minimarket). One command, no webview, no cloud.

## Shape (and why)

**API-level E2E**, not UI-driving. It boots the actual server and hits the same
`/api/v1/*` endpoints the Tauri views call (sourced from
`client/src-tauri/src/lib.rs`), asserting operator-visible outcomes (stock moved,
sale created, caja closed). This is the lightest shape that runs on a plain
Windows box — no `tauri-driver`/WebDriver, no display. If we later want true UI
E2E it can sit alongside; the golden-path assertions here are the contract.

CI is billing-walled, so this is primarily a **LOCAL gate**. The same run is
reproducible in CI via `.github/workflows/e2e.yml`, which is **`workflow_dispatch`
only** (manual, opt-in) so it never auto-burns minutes on push/PR.

## Gate (one command)

The canonical gate is **local** and a single command — client build + unit tests
+ live e2e, exit-nonzero on any failure:

```bash
cd client
npm run gate          # = npm run build && npm run test && npm run e2e
```

`.github/workflows/e2e.yml` runs this exact script, but is `workflow_dispatch`
only (billing-wall, CLAUDE.md regla #9) — so **local `npm run gate` is the source
of truth**, CI is opt-in reproduction.

## Run (e2e only)

```bash
cd client
npm run e2e
```

First run builds `pharma-api` + `pharma` in release (slow, once). Later runs reuse
the binaries. Exit code is non-zero on any failed assertion.

### Env knobs

| var           | default                | meaning                                  |
|---------------|------------------------|------------------------------------------|
| `E2E_REBUILD` | unset                  | `1` forces `cargo build --release`       |
| `E2E_PORT`    | `18080`                | port pharma-api binds (127.0.0.1)        |
| `RUST_LOG`    | `warn`                 | server log level (printed on boot fail)  |

## What it does

1. Build (cached) `pharma-api` + `pharma`.
2. Create a throwaway SurrealKv DB under the OS temp dir.
3. **Server down** (SurrealKv is a single-writer file lock): `pharma migrate`,
   then create two tenants + an `admin,owner` user each.
4. Boot `pharma-api` on the temp DB; wait for `/health/ready`.
5. For **each vertical** (`pharmacy`, `minimarket`) seed demo data via
   `POST /admin/seed-demo` (same service as the in-app "datos demo" button) and
   run the golden path:
   `login -> open caja -> sale -> receipt -> boleta -> devolución -> cierre ->
   reporte`.
6. Tear down server + temp DB.

## Golden-path assertions

- **login** returns a JWT.
- **catalog** non-empty after seed; minimarket carries **no** `active_ingredient`
  (no clinical pack leaks into a non-pharmacy rubro).
- **POS sale** returns `201` with no prescription required (multi-rubro: a
  minimarket sale must not demand a receta), stock decremented by exactly 1.
- **receipt** has ≥1 line.
- **boleta (DTE SII)** is universal — it must be handled **cleanly** on Free with
  no CAF/cert: a coded `4xx` upsell, never a `5xx`/crash. The actual gate
  status/code is logged.
- **devolución** with `restock=true` restores stock to the pre-sale level.
- **arqueo** preview returns; **cierre** closes the session.
- **reporte** — the day's sale surfaces in the core (Free) `sales-daily` report,
  closing the operator's daily loop.

## Goods-receipt assertions (`goodsReceiptFlow`)

Mirrors `compras.ts`: supplier → draft PO → receive. The receipt sub-flow is
**forward-compatible** with BUG-bob-002 (a draft PO can't be received — `POST
/receive` only accepts `sent/approved/partially_received` and no route issues a
`draft→sent` transition). The flow first **probes** for a transition route (`POST
/{id}/send`, then `/{id}/approve`); both 404 today, so it stays an **xfail**. The
day a transition lands, these real assertions run automatically (no edit needed):

- **partial receipt** → PO `partially_received`, stock `+= partial qty`.
- **WAC** after each receipt = `(stock0·cost0 + Σqty·unitCost)/(stock0+Σqty)`
  (a never-costed SKU seeds the line average) — within a cent.
- **full receipt** → PO `received`, stock `+= full ordered qty`.
- **over-receipt** on a completed PO refused (4xx, never 5xx).

## Minimarket multi-rubro contract (`noRecetaBoletaFlow`, minimarket-only)

recetas/controlados are PHARMACY-ONLY; boleta/DTE is UNIVERSAL:

- catalog carries **no** `active_ingredient` (no clinical/controlled marker).
- a plain sale closes **with no prescription step** (`201`).
- a **boleta still emits cleanly** (coded `4xx` upsell on Free, never `5xx`).

## Files

| file                 | role                                                    |
|----------------------|---------------------------------------------------------|
| `run.mjs`            | orchestrator (`npm run e2e` entrypoint)                 |
| `flows.mjs`          | golden path + goods-receipt + compliance + minimarket   |
| `lib/harness.mjs`    | build/CLI/server lifecycle + HTTP client + assertions   |

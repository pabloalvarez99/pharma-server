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
only** (manual, opt-in) so it never auto-burns minutes on push/PR. When CI billing
is unblocked, flip that workflow's `on:` to `push`/`pull_request` — the job already
runs the full gate (`npm run gate`).

## Run

The **canonical gate** is one command — client build + unit tests + live e2e:

```bash
cd client
npm run gate     # = npm run build && npm run test && npm run e2e
```

Or just the live-stack e2e on its own:

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
   run, in order:
   - **golden path** — `login -> open caja -> sale -> receipt -> boleta ->
     devolución -> cierre -> reporte`.
   - **goods-receipt** — supplier -> draft PO -> receive (BUG-bob-002: probes
     for a draft→sent/approve transition; xfail until one ships, see below).
   - **compliance** — core reports 200, margins Pro-gate, factura/libro clean.
   - **no-receta (minimarket only)** — confirms a non-pharmacy rubro is never
     forced through receta/controlados machinery, while boleta still emits.
6. Tear down server + temp DB.

## Known-bug xfail (self-healing)

`goodsReceiptFlow` characterizes **BUG-bob-002**: the app creates a `draft` PO,
but the wired `POST /receive` only accepts `sent`/`approved`/`partially_received`
and **no route issues a draft→sent transition** — so goods receipt is unreachable
through the app and stock never moves. The flow probes `/send`, `/approve`,
`/submit`; while none exists it asserts a clean `409` and logs the xfail. The
moment a transition route lands it advances the PO and the real receive+stock
assertions run for keeps — turning a stale xfail red.

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

## Files

| file                 | role                                                          |
|----------------------|---------------------------------------------------------------|
| `run.mjs`            | orchestrator (`npm run e2e` entrypoint)                       |
| `flows.mjs`          | per-vertical flows: golden / goods-receipt / compliance / no-receta |
| `lib/harness.mjs`    | build/CLI/server lifecycle + HTTP client + assertions         |

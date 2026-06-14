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

CI is billing-walled, so this is a **LOCAL gate**, not a CI job.

## Run

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
   `login -> open caja -> sale -> receipt -> boleta -> devolución -> cierre`.
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

## Files

| file                 | role                                                    |
|----------------------|---------------------------------------------------------|
| `run.mjs`            | orchestrator (`npm run e2e` entrypoint)                 |
| `flows.mjs`          | the per-vertical golden path                            |
| `lib/harness.mjs`    | build/CLI/server lifecycle + HTTP client + assertions   |

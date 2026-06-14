# E2E harness — `npm run e2e`

Repeatable, local, **API-level** end-to-end suite for pharma-server. One command
spins a server on a throwaway DB, seeds two demo verticals, and drives the exact
HTTP endpoints the desktop views call — asserting the cashier's visible result.

## Why API-level (not webview UI)

The form was chosen as **the lightest thing that runs on this Windows**:

| | API-level (chosen) | Webview UI E2E |
|---|---|---|
| Extra runtime | none — Node ≥18 `fetch` + the built `pharma`/`pharma-api` exes | WebView2 runtime + `tauri-driver` + matching `msedgedriver` + a built Tauri app |
| Determinism | high (no render race, no element waits) | brittle on headless/CI-less Windows |
| What it proves | the same server contract the views consume (`client/src/api.ts` → Tauri → these endpoints) | pixels |

`client/src/api.ts` is a thin typed wrapper over Tauri `invoke`, and the Rust
side (`client/src-tauri/src/lib.rs`) just forwards to these HTTP endpoints. So
hitting the endpoints **is** the click-path, minus the chrome. The DTE full
emission cycle (CAF + cert + TED) is already covered by the Rust integration
suite (`crates/api/tests/dte_endpoints.rs`); this harness does **not** duplicate
that plumbing — see "Boleta / 402" below.

## Run

```bash
cd client
npm run e2e
```

Prereqs (the runner checks and fails loudly if missing):
- `pharma.exe` and `pharma-api.exe` built: from the repo root run
  `cargo build -p api -p cli`.
- Node ≥ 18 (global `fetch`). Verified on Node 26.

The runner is dependency-free (`e2e/*.mjs`, plain Node ESM) on purpose: it is a
**local gate**, not a CI job (CI is billing-walled). Nothing here is type-checked
by `tsc --noEmit`, so it never affects the client build gate.

## What it does

1. Fresh temp DB dir under `e2e/.tmp/<run>/surreal` (deleted on exit).
2. `pharma migrate` → schema.
3. Two tenants (`farmacia-demo`, `minimarket-demo`), one `admin,cashier` user each.
4. `pharma seed-demo --tenant … --vertical pharmacy|minimarket`.
5. Boots `pharma-api` on `127.0.0.1:<port>`, waits for `GET /health/ready`.
6. Runs the golden paths below against **both** verticals, asserting visible state.
7. Kills the server, removes the temp dir, exits non-zero on any failure.

## Golden paths (both verticals)

- **Counter day**: login → open cash session → POS sale (seeded product) →
  emit boleta (reachability + config boundary, see below) → refund the sale →
  close cash session (arqueo/discrepancia visible).
- **Restock**: create product → add lot (`product_batch`, expiry) → create
  supplier + purchase order → assert the receive guard rejects the fresh `draft`
  PO (see gap below) → raise stock with a `POST /stock-movements` (+delta) →
  assert product stock rose.
- **Free 402**: `GET /api/v1/reports/margins-daily` → `402
  FEATURE_REQUIRES_UPGRADE` (the canonical Free-tier gate, Fase 10d POC).

### Minimarket-specific

- Every seeded product is `prescription_type == "direct"` with no
  `active_ingredient` — i.e. the core does **not** demand a receta/controlado
  for a non-pharmacy rubro. The same POS sale that needs a prescription record
  in a pharmacy needs none here, and it still completes.
- Boleta path behaves identically to pharmacy (rubro-agnostic).

## Findings (gaps surfaced by this harness)

- **No PO draft→sent/approved transition over HTTP.** `POST /purchase-orders`
  always creates `draft`; `POST /purchase-orders/{id}/receive` requires
  `sent`/`approved`/`partially_received`. There is **no API endpoint** to issue
  a PO to the supplier (even `crates/api/tests/po_receiving.rs` DB-pokes the
  status to test receive). The harness asserts the receive guard correctly
  rejects a `draft` (409) and raises stock via `POST /stock-movements` instead.
  → Recommend a `POST /purchase-orders/{id}/send` (or `/approve`).

### Boleta / 402 — what's actually asserted

Local boleta **emission** (`POST /api/v1/dte/boletas`) is **not** license-gated
("Free tier OK" in `crates/api/src/v1/dte.rs`); the **SII send**
(`POST /api/v1/dte/{id}/send`) is the Pro+ gate. Emission additionally needs an
emisor config + digital cert + an active CAF, none of which the demo seed loads.

So this harness asserts the **boundary**, not a signed 201: against a paid order
it confirms `POST /dte/boletas` is reached with valid auth + cashier role and is
rejected only at emisor/CAF configuration (`400`/`409`) — proving no
clinical/license block sits in front of it for either vertical. The real Free
`402` is asserted separately and unambiguously via `reports/margins-daily`. The
full signed-boleta cycle lives in `crates/api/tests/dte_endpoints.rs`.

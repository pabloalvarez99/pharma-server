# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.26] — 2026-05-27

Scope: post-integration P0/P1 hardening + Fase 11a/11b license rails landed on
`integration/0.1.25`. Deploy still parked (cert Authenticode + smoke VM).

### Added

- **License rails**
  - Prod pubkey `lk-prod-2026-01` embedded in `crates/license/src/keys.rs` —
    binary now verifies real licenses minted by `pharma-license-server`.
    Cross-repo canonical-JSON contract test green. (Fase 11a, `#51`)
  - CLI `pharma license activate <ID> [--server URL] [--reload-url]
    [--reload-token]` — fetch license from server, Ed25519 verify offline,
    persist, optional hot-reload. (Fase 11b, `#52`)
- **MSI UX (Fase 9.0)**
  - Post-install dashboard auto-launch (WiX `LaunchDashboardWait` CustomAction
    + `launch-wait.ps1` health poll, interactive-only) + Start Menu shortcut.
    Sandbox smoke harness. (`#53`)
- **Zero-cost launch path** — self-sign cert ([ADR-0008]) + Hyper-V smoke
  (`installer/sign`, `installer/smoke`) + MP/Stripe pilot ([ADR-0009]) +
  `cargo audit` baseline (`audit.toml` ignore-list, RUSTSEC-2021-0046 false
  positive documented).

### Fixed

- **P0 SQL injection** — catalog `bulk_update_price`/`etiquetas` raw-string
  interpolation → typed `PriceOp`/`TagField` enums + bound params. Tenant-scope
  helper for expenses. (`#56`)
- **P0 idempotency (BUG-002)** — same `Idempotency-Key` + different body now
  returns 409 instead of replaying stale sale. SHA256 body fingerprint, new
  migration `0020_idempotency_body_hash.surql`. (`#67`)
- **P0 over-refund (BUG-005) + batch restock (BUG-007)** — cumulative prior-
  refund guard; restock restores consumed FEFO lots. (`#63`)
- **P0 license tenant-binding (BUG-006)** — license reload rejects foreign-
  tenant signed licenses (403) after Ed25519 verify. (`#62`)
- **P1 agent panic elimination** — 18 `unwrap`/`expect` in `crates/agent` hot
  path → `Result<T, AgentError>`; hostile peer input no longer panics
  `POST /agent/inbox`. (`#61`)
- **BUG-001 role gate** — `role::layer` Stack arg order (fix landed via step 02);
  regression sentinels flipped (admin POST → 200, no-token → 401).

## [0.1.25] — 2026-05-24

Scope: closes the 28-step `integration-merge-plan-v2` window. Brings second-wave
features (public web ingest, exec dashboard, stock webhook, web-sync interop,
POS bench harness) on top of the first-wave consolidation (rate-limit, swagger
8→9, DTE 9.1 cascade, audit-log v2, customers loyalty/history, order-receipt,
purchasing-receiving, public-catalog, near-expiry alerts, client Tauri scaffold,
22k-catalog migration tooling).

### Added

- **HTTP API**
  - `POST /api/v1/public/orders/web` — HMAC-SHA256 signed, per-IP rate-limited,
    opt-in web-order ingest. Idempotent by `external_order_id`, channel=`web`,
    no stock decrement (reservation until pickup). 64 KiB body cap → 413. New
    migration `0019_order_channel.surql`. (`feat/api-public-orders-web`,
    ADR-0012 pattern 2)
  - `GET /api/v1/public/catalog?tenant=<slug>&q=&limit=&offset=` — read-only,
    no-auth, per-IP rate-limited. Returns `external_id/name/price/in_stock/
    active_ingredient` only; no internal id, no cost, no stock count. CORS
    pinned to `public_catalog.cors_origins`. Opt-in, default 404.
    (`feat/api-public-catalog`)
  - `GET /api/v1/reports/dashboard` — single-call exec KPI aggregate:
    ventas_hoy, ventas_mes, inventario, top_productos[5], por_vencer,
    margen_hoy (null on Free tier — no 402). Concurrent `tokio::try_join!` fan-
    out; role-gated admin/owner/quimico; `Cache-Control: private, max-age=30`.
    (`feat/api-exec-dashboard`)
  - Per-IP + per-tenant rate-limit middleware (governor token-bucket); 429 +
    `Retry-After`; `/health` exempt; configurable via `AppConfig.rate_limit`.
    (`feat/api-rate-limit-v2`)
  - Audit-log query API v2 (`pub mod audit;` in `v1/mod.rs`) with
    before/after deltas. (`feat/api-audit-log-query-v2`)
  - Customers loyalty + purchase history endpoints
    (`/api/v1/customers/{search,{id},{id}/history}`).
    (`feat/customers-loyalty-history`)
  - Order receipt endpoint on `v1/sales.rs` with role gate.
    (`feat/order-receipt`)
  - Purchasing / receiving endpoints under `v1/purchasing.rs`; migration
    renumbered to `0018_purchase_order_receiving.surql`.
    (`feat/purchasing-receiving`)
  - Bulk customers + historic orders importers (`feat/api-import-customers`,
    `feat/api-import-historic-orders`).
  - Swagger UI served when `docs.enabled=true`; utoipa-swagger-ui bumped
    8 → 9. (`feat/api-swagger-ui`)

- **DTE / Boleta electrónica SII (Fase 9.1 cascade)**
  - New `crates/dte/` workspace crate with XML boleta 39, TED, CAF parsing.
    Migration `0017_dte.surql`. (`feat/dte-9-1-abc-xml-ted-caf`)
  - Cancel + resend flows, libros X/Z. (`feat/dte-9-1-fgh-cancel-libro-xz`)
  - PFX certificate encryption (AES-GCM + Argon2id, zeroize on drop).
    (`feat/dte-9-1-i-cert-encrypt`)
  - Tier gating (Free/Pro/Business) for DTE send.
    (`feat/dte-9-1-j-tier-gating`)
  - SII upload + polling via `reqwest` rustls-tls.
    (`feat/dte-9-1-d-e-sii-upload-v2`)
  - `pharma dte | caf | cert` CLI subcommands with real wiring.
    (`feat/dte-9-1-k-cli`, `feat/dte-cli-wire-cert`)

- **Stock webhook (ADR-0013)**
  - Async fire-and-forget ERP→web stock-change dispatcher on
    `stock_movement`. HMAC-SHA256 signed POST to
    `AppConfig.stock_webhook.target_url`; bounded retry `[1s, 5s, 30s]` then
    drop+WARN. Non-blocking (`tokio::spawn`) so POS p99 <50ms is preserved.
    Opt-in, default false. (`feat/api-stock-webhook`)
  - Prometheus counter `pharma_stock_webhook_dropped_total{tenant,reason}`
    where `reason ∈ {contract_error_4xx, retry_exhausted}` — exported via
    `metrics` 0.23 through the existing axum-prometheus 0.7 recorder. No
    `external_id` label (cardinality bounded).

- **Jobs**
  - Near-expiry scan job + scheduler entry.
    (`feat/jobs-near-expiry-alert-v2`)

- **Client (Tauri 2 desktop)**
  - New `client/` workspace with Tauri 2 + React/TS scaffold.
    (`feat/client-tauri-scaffold-v2`)
  - Login polish + tier-badge AppShell rebrand.
    (`feat/client-login-polish`)
  - Dashboard view (post-login landing): ventas hoy, inventario, stock
    crítico, por vencer, top-5. Independent loads via `Promise.allSettled`.
  - Caja view: apertura / cierre / arqueo (expected vs counted with
    discrepancy classification). Money fields cross the wire as strings.
  - Clientes view: debounced search by name/RUT/phone, detail panel
    (puntos, lifetime spend, visitas), purchase history. Maps server 404 to
    `CUSTOMERS_MODULE_MISSING` sentinel for graceful degradation.
  - New Tauri commands: `customer_search`, `customer_detail`,
    `customer_history`, `cash_sessions`, `open_cash_session`,
    `cash_arqueo`, `close_cash_session`. (`feat/client-dashboard-caja-clientes`)

- **Web / ERP interop (ADR-0012)**
  - `docs/adr/0012-web-onprem-http-interop.md` + operator guide
    `docs/strategy/web-interop.md`.
  - `scripts/web-sync/pull-catalog.mjs` zero-dep Node tool that runs in
    the web env, fetches `/api/v1/public/catalog`, emits
    `catalog_upsert.sql` for Cloud SQL. README + env-var contract.
    `scripts/web-sync/test.mjs` node:test suite (7 cases).
    (`feat/web-sync-interop`)

- **Performance**
  - Criterion harness `cargo bench -p api --bench pos_sale` covering
    `single_item_cash_happy_path`, `GET inventory`, `GET products?limit=100`.
    `docs/ops/performance.md`. CI `bench-smoke` job (`cargo bench -- --test`)
    so PosSaleRequest/AppState drift breaks CI rather than rotting the
    benchmark silently. (`bench/pos-hot-path`)

- **Migrations**
  - `0017_dte.surql`
  - `0018_purchase_order_receiving.surql` (renumbered from `0017_*` to
    resolve stem collision against DTE)
  - `0019_order_channel.surql`

- **Catalog tooling**
  - `feat/migration-full-catalog` — extract/import scripts + tests for the
    22k-SKU live catalog (used for Coquimbo go-live).

- **MSI installer**
  - `installer/wix/main.wxs` `ServiceComponents` finalized: registers
    `pharma-service.exe` as `PharmaServer` (`ownProcess`, auto-start, with
    `ServiceControl` install/uninstall) + TCP 8080 firewall rule +
    `MajorUpgrade`. `Permanent=yes` on the data-dir component so customer
    SurrealDB survives uninstall/upgrade (no-data-loss invariant).
    `docs/ops/msi-build.md` with `cargo wix --package service` command,
    candle dry-run, clean-VM smoke procedure, and signtool Authenticode
    step. (`feat/msi-installer-complete`)

- **Configuration**
  - `AppConfig` gains: `rate_limit`, `docs`, `public_catalog`,
    `public_orders`, `stock_webhook` (all `Default`-able). Matching sections
    added to `config/default.toml`.

- **Workspace dependencies**
  - `governor`, `nonzero_ext`, `hmac`, `subtle`, `sha2`, `hex`, `metrics`,
    `metrics-util`, `reqwest`, `quick-xml`, `aes-gcm`, `argon2`, `zeroize`,
    `rpassword`. `utoipa-swagger-ui` bumped 8 → 9.

### Changed

- `crates/api/src/lib.rs::run()` now refuses to boot when `PHARMA__JWT__SECRET`
  is the placeholder `change-me-in-production` (escape hatch:
  `PHARMA_ALLOW_INSECURE_JWT=1` for local dev). 3 unit tests.
  (Shipped commit `20b7612`; the duplicate hunk in `bench/pos-hot-path` is
  dropped per merge plan §4.8.)
- `import_products` is now idempotent — upserts by `(tenant, external_id)`
  via new `repo::find_id_by_external_id` rather than inserting duplicates;
  accepts `price` OR `sale_price` CSV header; `ImportSummary` reports
  `updated` count. Makes the 22k-catalog migration safely re-runnable.
- Exec dashboard `top_productos` now emits the full domain `TopProductRow`
  (rank, revenue_pct, abc_class, product_id) via serde — the Tauri ABC
  table now renders without schema mismatch.
- Client login: server URL field is now resolved by priority (persisted
  last-used > build-time `VITE_SERVER_URL` > loopback fallback). The
  "Conexión avanzada" panel auto-opens on first launch when no value is
  configured, so multi-machine LAN deploys no longer silently log into
  loopback. Hint copy updated for LAN IP:port.
- `AppConfig` field union: a single `RateLimitConfig` instance survives
  (re-add from `feat/api-public-catalog` is discarded per merge plan §4.4).

### Fixed

- **Middleware order:** `AllowedRoles` extension was installed in the wrong
  layer order, so admin-mutating endpoints (POST /products, agent-orders
  accept/reject/fulfill, etc.) returned `500 Missing request extension
  AllowedRoles` instead of `401/403/2xx`. Stack corrected; 4 regression
  tests in `role_extension.rs`. (`fix/role-allowed-roles-extension` /
  shipped commit `51fdcea`)
- **Orphan domain module:** `crates/domain/src/sales/web_order.rs` was added
  in commit `bb7c7d8` but `pub mod web_order` was never declared in
  `sales/mod.rs` — the file was orphan, cargo never compiled it, and the
  HTTP handler did not exist. Subsequent commit (`654ed90`) declares the
  module, surfaces it to `clippy -D warnings`, and adds the missing
  `crates/api/src/v1/public_orders.rs` handler. *(See lesson:
  "orphan modules / truncated output can fake a passing gate".)*
- **web-sync `pull-catalog.mjs`:**
  - `sqlString()` guarded against `" "` (space) instead of `"\0"` (NUL),
    so every real product name failed `process.exit(3)` on the first item
    — Pattern A was 100% non-functional. Now guards NUL.
  - Tombstone `NOT IN` list was being re-parsed out of already-rendered SQL
    via regex and silently degraded to `NOT IN (NULL)` for SKUs containing
    commas. Now derived from typed `items[].sku`.
  - Entry-point guard read `process.argv[1].replace(...)` unconditionally
    and threw `TypeError` when imported via stdin/REPL. Replaced with
    `pathToFileURL(argv[1])` + undefined guard.
  - 7 new `node:test` cases cover all three regressions.

### Security

- **Public web ingest authenticity:** `POST /api/v1/public/orders/web`
  verifies HMAC-SHA256 over the raw request body with constant-time
  `hmac::Mac::verify_slice`; any body mutation breaks the MAC. Opt-in
  (`enabled=false` OR empty `hmac_secret` → uniform 404, feature hidden).
  Bounded 64 KiB body (413 over limit). Per-IP rate-limit.
- **Replay protection:** idempotent by `external_order_id` (a verbatim
  replay re-resolves to the same order, no second write).
  `X-Pharma-Timestamp` freshness window ≤ 5 minutes as defence-in-depth.
- **Stock webhook authenticity:** outbound POST is HMAC-SHA256 signed with
  `stock_webhook.hmac_secret`; opt-in default false.
- **JWT secret guard:** server refuses to boot with the placeholder
  `change-me-in-production` JWT secret (silent forgeable-token bypass
  closed). Escape hatch: `PHARMA_ALLOW_INSECURE_JWT=1` for local dev only.
- **Public catalog tenant non-disclosure:** unknown tenant slug → 404
  (no tenant-existence oracle). Internal ids, cost, and stock counts are
  never exposed.
- **DTE PFX certificates** are stored AES-GCM encrypted with an
  Argon2id-derived KEK; passphrases zeroized on drop.

### Deferred

- **Stock-webhook coalescing / debounce** — the second ADR-0013 gap. The
  web side already dedups by `idempotency_key` and `new_stock` is an
  absolute idempotent value, so over-notification is benign. Tracked as
  separate follow-up; not worth process-wide async state in this release.
- **License-server real backend (`pharma-license-server`)** — repo skeleton
  and Webpay/Stripe integration remain Fase 11 work. v0.1.25 ships only the
  client-side license verify crate behaviour merged in earlier releases; no
  online activation yet.
- **Authenticode-signed MSI** — `signtool sign` step documented but pending
  code-signing certificate (Fase 9 blocker).
- **`feat/client-login-polish` collapse** — kept linear after step 22 per
  merge plan §3 row 23; trivial textual overlap accepted.
- **Stale branches dropped** (will NOT be merged in 0.1.25): `feat/dte-fase-9-1`,
  `feat/dte-9-1-d-e-sii-upload`, `feat/dte-cli`, `feat/dte-9-1-d-e-i-sii-cert`,
  `feat/dte-9-1-lm-tests-docs`, `feat/client-shell-tier-badge`,
  `feat/client-functional-views`, `feat/api-audit-log-query`,
  `feat/audit-log-before-after` — all superseded.

### Known issues

- **CI billing-blocked** — GitHub Actions is suspended for this account.
  All 0.1.25 merges land **without remote CI validation**; the local gate
  (`cargo fmt --all -- --check && cargo clippy --workspace --all-targets
  -- -D warnings && cargo test --workspace`) is the only safety net until
  billing is resolved.
- **No CI-built MSI for v0.1.25** — last shipped MSI installer remains
  [v0.1.23](https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.23)
  (12.30 MB, unsigned). A 0.1.25 MSI must be built locally with
  `cargo wix --package service` until CI is back.
- **Authenticode certificate** still pending — current MSI raises Windows
  SmartScreen warning on first run.
- **Target-branch ambiguity** — the integration target for the 28-step
  merge plan (`feature/erp-parity` vs `release/tufarmacia-golive` vs a new
  `integration/0.1.25` cut) requires operator confirmation per merge plan
  §0 before the sequence can run.

[0.1.25]: https://github.com/pabloalvarez99/pharma-server/compare/v0.1.24...v0.1.25

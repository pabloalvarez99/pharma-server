# Integration Merge Plan v2 — 0.1.25 session (2026-05-26)

Augments `docs/ops/integration-merge-plan.md` (v1, on `chore/release-engineering`)
with the 5-agent integration analysis + the **7 second-wave PRs** opened in the
2026-05-26 session that v1 does not cover.

## What's new vs v1

1. **Second wave (7 branches)** slotted into the sequence:
   `bench/pos-hot-path`, `feat/api-public-orders-web`, `feat/api-exec-dashboard`,
   `feat/api-stock-webhook`, `feat/client-dashboard-caja-clientes` (re-take),
   `feat/web-sync-interop` (re-take of latest HEAD).
2. **Migration map is now definitive**: `0017_dte` / `0018_purchase_order_receiving`
   (post-rename) / `0019_order_channel` (second-wave). The renumber of
   `feat/purchasing-receiving` is a **hard prerequisite** — git does not detect
   the stem collision.
3. **JWT-boot-guard collision** between `bench/pos-hot-path` and shipped commit
   `20b7612` on `release/tufarmacia-golive`. Take the shipped version.
4. **Target-branch ambiguity** (v1 targets `feature/erp-parity`, release work
   landed on `release/tufarmacia-golive`). Gated up front — operator must confirm
   before any merge.
5. **CI billing-blocked**: merges will land **without remote CI gating**. Local
   gate is the only safety net until billing is resolved.
6. New `AppState` / `core/config.rs` rules for stock-webhook, public-orders,
   exec-dashboard.

---

## 0. Target-branch confirmation gate (STOP — operator)

v1 was written assuming integration target = `feature/erp-parity`. Between v1
and v2, hot-fixes for the Coquimbo go-live were merged directly onto
`release/tufarmacia-golive` (including `20b7612` JWT boot-guard,
`fix/role-allowed-roles-extension`, `fix/catalog-import-upsert`,
`feat/msi-installer-complete`, `chore/production-hardening`).

**Operator must answer before any merge runs:**

- [ ] Integration target = `feature/erp-parity` (v1 assumption — re-base on
      `feature/erp-parity` HEAD; replay hot-fixes from `release/...` if needed).
- [ ] Integration target = `release/tufarmacia-golive` (already carries the
      hot-fixes; step 00 `fix/role-allowed-roles-extension` and the JWT
      boot-guard are **already merged** — skip them).
- [ ] New branch `integration/0.1.25` cut from one of the above.

The rest of this plan is target-agnostic except where flagged "[golive: skip]".

---

## 1. Definitive migration-number map

`feature/erp-parity` ends at `0016_purchase_payment.surql`. After integration:

| Number | Source branch | File |
|--------|---------------|------|
| 0017 | `feat/dte-9-1-abc-xml-ted-caf` (DTE cascade) | `0017_dte.surql` |
| 0018 | `feat/purchasing-receiving` **after rename** | `0018_purchase_order_receiving.surql` |
| 0019 | `feat/api-public-orders-web` (second wave) | `0019_order_channel.surql` |

The **rename of `0017_purchase_order_receiving.surql -> 0018_...`** is a hard
prerequisite of step 13 below. `_migrations` tracks by filename stem; two
`0017_*` files cannot coexist and tooling will not detect it.

---

## 2. Drop-list (skip; do not merge)

| Branch | Reason |
|--------|--------|
| `feat/dte-fase-9-1` | superseded by `feat/dte-9-1-abc-xml-ted-caf` + cascade |
| `feat/dte-9-1-d-e-sii-upload` | superseded by `feat/dte-9-1-d-e-sii-upload-v2` |
| `feat/dte-cli` | superseded by `feat/dte-9-1-k-cli` |
| `feat/dte-9-1-d-e-i-sii-cert` | rolled into `feat/dte-9-1-i-cert-encrypt` |
| `feat/dte-9-1-lm-tests-docs` | incomplete roll-up; coverage lands via cascade |
| `feat/client-shell-tier-badge` | superseded by `feat/client-tauri-scaffold-v2` |
| `feat/client-functional-views` | superseded by `feat/client-dashboard-caja-clientes` |
| `feat/client-login-polish` | collapse into `feat/client-dashboard-caja-clientes` OR keep linear (see step 19) |
| `feat/api-audit-log-query` | superseded by `feat/api-audit-log-query-v2` |
| `feat/audit-log-before-after` | alias of `feat/api-audit-log-query-v2` |

---

## 3. Merge order (29 steps)

Numbers are merge sequence, not migration numbers. **First wave** carried from
v1; **second wave** rows marked *(SW)*. Run the local gate
(`cargo fmt --all -- --check && cargo clippy --workspace --all-targets -D warnings && cargo test --workspace`)
after every step.

| # | Branch | What it adds | Conflict notes | Resolution |
|---|--------|--------------|----------------|------------|
| 00 | `fix/role-allowed-roles-extension` | `AllowedRoles` middleware order fix | `middleware/role.rs` | trivial; [golive: skip — already merged] |
| 01 | `feat/dte-9-1-abc-xml-ted-caf` | XML boleta 39 + TED + CAF; **adds `0017_dte.surql`** | new crate `crates/dte/`, root `Cargo.toml` | take as-is |
| 02 | `feat/dte-9-1-fgh-cancel-libro-xz` | cancel/resend + libro + X/Z | inherits `0017_dte` | linear off 01 |
| 03 | `feat/dte-9-1-i-cert-encrypt` | PFX AES-GCM + Argon2id | inherits `0017_dte` | linear off 02 |
| 04 | `feat/dte-9-1-j-tier-gating` | Free/Pro/Business gating | `crates/api` (Tier→SendTier) | linear |
| 05 | `feat/dte-9-1-d-e-sii-upload-v2` | SII upload + polling | reqwest rustls-tls dep | union deps |
| 06 | `feat/dte-9-1-k-cli` | `pharma dte/caf/cert` (stubs) | `crates/cli` | linear |
| 07 | `feat/dte-cli-wire-cert` | replaces bail! stubs | depends on 02/03/06 | merge after all DTE |
| **PRE-13** | (renumber) | `git mv 0017_purchase_order_receiving.surql 0018_...` on `feat/purchasing-receiving` before step 13 | filename stem collision | see §6.1 |
| 08 | `feat/api-audit-log-query-v2` | `pub mod audit;` in `v1/mod.rs` | router block | union pub mod + .merge() |
| 09 | `feat/api-rate-limit-v2` | `RateLimitConfig`, `AppState.rate_limit`, governor | `core/config.rs`, `lib.rs`, root Cargo | base for §6.2 |
| 10 | `feat/api-swagger-ui` | `DocsConfig`, `docs_enabled`, utoipa-swagger-ui 8→9 | `core/config.rs`, `lib.rs`, root Cargo | keep `9` |
| 11 | `feat/customers-loyalty-history` | routes inside `v1/customers.rs` | no router collision | linear |
| 12 | `feat/order-receipt` | `v1/sales.rs` + role gate | `middleware/role.rs` | union allowed roles |
| 13 | `feat/purchasing-receiving` *(post-rename)* | `v1/purchasing.rs` + `0018_...surql` | `middleware/role.rs`, migrations | union roles; verify file is `0018_*` |
| 14 | `feat/api-public-catalog` | `PublicCatalogConfig` + `v1/public_catalog.rs` | **re-adds identical `RateLimitConfig`** | keep rate-limit from step 09; take only `PublicCatalogConfig` (see §6.3) |
| 15 | `feat/api-import-customers` | import-customers handler | already carries `fix/role-...` | trivial |
| 16 | `feat/api-import-historic-orders` | `domain/src/sales/historic.rs` + handler | no router collision | linear |
| 17 | **(SW)** `feat/api-public-orders-web` | `v1/public_orders.rs` HMAC + wires orphan `domain/sales/web_order.rs`; **adds `0019_order_channel.surql`**; new deps `hmac`/`subtle`/`sha2`/`hex`; reuses governor/nonzero_ext | `AppState`, `core/config.rs`, root Cargo, `v1/mod.rs` | depends on 09 (governor) and 13 (mig 0019 free); union AppState + config + Cargo |
| 18 | **(SW)** `feat/api-exec-dashboard` | `v1/dashboard.rs` + role guard + try_join | `v1/mod.rs`, `middleware/role.rs` | union pub mod + roles |
| 19 | **(SW)** `feat/api-stock-webhook` | `v1/sales.rs` hook + `crates/api/src/stock_webhook.rs` + `StockWebhookConfig`; deps `metrics`/`metrics-util`; metric `pharma_stock_webhook_dropped_total` | `AppState`, `core/config.rs`, root Cargo, `v1/sales.rs` (with step 12) | union AppState + config + Cargo; replay sales hook on top of step 12 |
| 20 | `feat/jobs-near-expiry-alert-v2` | `run_near_expiry_scan` + schedule | `crates/jobs` | linear |
| 21 | `feat/client-tauri-scaffold-v2` | new `client/` Tauri 2 | no server coupling | linear |
| 22 | **(SW)** `feat/client-dashboard-caja-clientes` | LAN-UX login fix + caja/clientes views | **carries `crates/dte/` + `0017_dte.surql` from its base** | **MUST merge AFTER step 03 (DTE i-cert) so dte files are no-op merge**; take only `client/` diff |
| 23 | `feat/client-login-polish` | rebrand AppShell | conflicts with step 22 | **collapse into 22 OR rebase on 22**; if kept linear, expect trivial textual overlap |
| 24 | **(SW)** `bench/pos-hot-path` | CI bench-smoke + JWT boot-guard | `.github/workflows/ci.yml` + **`crates/api/src/lib.rs::run()` collides with shipped `20b7612`** | **DROP the JWT boot-guard hunk** (already shipped); take only the `ci.yml` bench-smoke. [golive: collision is certain; cherry-pick ci.yml only] |
| 25 | `feat/migration-full-catalog` | extract/import scripts + tests | scripts + tests only | linear |
| 26 | **(SW)** `feat/web-sync-interop` | ADR-0012 + `web-interop.md` + `pull-catalog.mjs` (latest HEAD `e06908e`) | same branch as v1 step 21 — **re-take latest HEAD** | supersedes v1's older snapshot |

**Total: 26 steps + 1 pre-step rename = 27 ordered actions.**

---

## 4. Per-conflict resolutions

### 4.1 Migration 0017 double collision (CRITICAL — carried from v1)

`feat/dte-9-1-abc-xml-ted-caf` and `feat/purchasing-receiving` both claim
`0017`. DTE merges first (step 01) → keeps `0017_dte.surql`. Before step 13:

```powershell
git checkout feat/purchasing-receiving
git mv migrations/0017_purchase_order_receiving.surql migrations/0018_purchase_order_receiving.surql
rg "0017_purchase_order_receiving"   # update any reference; expect none in src
git commit -m "chore(migrations): renumber 0017 -> 0018 (0017 taken by DTE)"
git push
```

Step 17 (`feat/api-public-orders-web`) then takes `0019_order_channel.surql`
unconflicted.

### 4.2 `crates/core/src/config.rs` — `AppConfig` field union

Final struct must contain exactly one of each:

- `rate_limit: RateLimitConfig` (step 09; re-added identical by 14 — keep one)
- `docs: DocsConfig` (step 10)
- `public_catalog: PublicCatalogConfig` (step 14)
- `public_orders: PublicOrdersConfig` *(SW step 17)*
- `stock_webhook: StockWebhookConfig` *(SW step 19)*

All are `Default`-able; `config/default.toml` gets `[rate_limit]`, `[docs]`,
`[public_catalog]`, `[public_orders]`, `[stock_webhook]` sections.

### 4.3 `crates/api/src/lib.rs` — `AppState` constructors

Every constructor (prod + every test harness) must set every field:

- `rate_limit: Option<Arc<rate_limit::RateLimitState>>`
- `public_catalog: PublicCatalogConfig`
- `docs_enabled: bool`
- `public_orders: PublicOrdersConfig` + HMAC verifier *(SW)*
- `stock_webhook: Option<Arc<stock_webhook::StockWebhookState>>` *(SW)*

Compile-only check: `cargo test -p api --no-run` after each AppState-touching
merge (steps 09, 10, 14, 17, 19).

### 4.4 `feat/api-public-catalog` rate-limit re-add

Identical to v1 §2: when step 14 lands, **discard its `RateLimitConfig` +
`rate_limit` hunks** (already from step 09), **keep only** the
`PublicCatalogConfig` + `public_catalog` additions.

### 4.5 `crates/api/src/v1/mod.rs` router

Union of `pub mod` + `.merge()` lines from steps 08 (`audit`), 14
(`public_catalog`), 17 (`public_orders`), 18 (`dashboard`). Order is not
functional.

### 4.6 `crates/api/src/middleware/role.rs`

Steps 00, 12, 13, 18 each add `AllowedRoles` entries. Union all role lists; do
not drop any route guard.

### 4.7 Root `Cargo.toml` workspace dependency union

Union additions; for `utoipa-swagger-ui` keep `9` (not `8`). New deps that must
appear once at the workspace level:
`governor`, `nonzero_ext` (09, reused by 17), `reqwest` + `quick-xml` +
`aes-gcm` + `argon2` + `zeroize` + `rpassword` (DTE), `hmac` + `subtle` +
`sha2` + `hex` (17), `metrics` + `metrics-util` (19). After resolving:
`cargo update -w && cargo tree -d` — fail if any duplicated major.

### 4.8 `bench/pos-hot-path` ↔ shipped `20b7612` JWT boot-guard

Both modify `crates/api/src/lib.rs::run()` adding `check_jwt_secret()`. The
shipped version (`PHARMA_ALLOW_INSECURE_JWT=1` escape hatch + 3 unit tests) is
authoritative. **Drop the boot-guard hunk from `bench/pos-hot-path`** at merge
(step 24); take only `.github/workflows/ci.yml` bench-smoke job.

### 4.9 `feat/client-dashboard-caja-clientes` carries DTE base

The branch was forked from a DTE-stack ancestor; merging it re-introduces
`crates/dte/` + `0017_dte.surql`. Ordering (step 22 after step 03) makes those
files a textual no-op merge. If the operator wants paranoid safety: rebase
`feat/client-dashboard-caja-clientes` onto post-DTE HEAD before step 22 so the
diff narrows to `client/` only.

---

## 5. Post-integration checklist

Carried from v1, expanded for second wave:

- [ ] Target-branch confirmation recorded (§0).
- [ ] All 26 steps merged in order; local gate green after each.
- [ ] `ls migrations/` shows exactly one `0017_*` (`0017_dte.surql`), one
      `0018_*` (`0018_purchase_order_receiving.surql`), one `0019_*`
      (`0019_order_channel.surql`); no orphans.
- [ ] `AppConfig` has one each of `rate_limit`, `docs`, `public_catalog`,
      `public_orders`, `stock_webhook`.
- [ ] `AppState` constructors all compile: `cargo test -p api --no-run`.
- [ ] `v1/mod.rs` registers `audit`, `public_catalog`, `public_orders`,
      `dashboard` exactly once each.
- [ ] Root `Cargo.toml`: `utoipa-swagger-ui = 9`; new deps present once
      (`governor`, `nonzero_ext`, `hmac`, `subtle`, `sha2`, `hex`, `metrics`,
      `metrics-util`, `reqwest`, `quick-xml`, `aes-gcm`, `argon2`, `zeroize`,
      `rpassword`); `cargo tree -d` clean; `workspace.package.version = 0.1.25`.
- [ ] `config/default.toml` has `[rate_limit]`, `[docs]`, `[public_catalog]`,
      `[public_orders]`, `[stock_webhook]`.
- [ ] JWT boot-guard present exactly once (from `20b7612`, not from
      `bench/pos-hot-path`).
- [ ] CI workflow has `bench-smoke` job (from step 24).
- [ ] Metric `pharma_stock_webhook_dropped_total` exposed on `/metrics`.
- [ ] Full gate green on integrated branch:
      `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
- [ ] `CHANGELOG.md [0.1.25]` reflects what actually landed (both waves).
- [ ] `client/` Tauri app builds (`cd client && npm i && npm run tauri build` —
      smoke only, not blocking).

---

## 6. Known blockers

1. **GitHub Actions billing-blocked** — merges land **without remote CI**. The
   local gate is the only signal until billing is resolved. Do not skip
   `cargo fmt`/`clippy`/`test` between steps.
2. **Target-branch ambiguity** — §0 must be resolved before any merge.
3. **MSI release v0.1.25** — CI-built MSI cannot ship until billing is fixed.
   Last shipped MSI = v0.1.23. Manual local build is the only path.
4. **Stale-branch garbage** — the drop-list (§2) must be honored; merging any
   superseded branch will re-introduce conflicts that this plan does not
   describe.

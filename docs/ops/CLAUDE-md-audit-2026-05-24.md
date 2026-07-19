# CLAUDE.md audit — 2026-05-24

Read-only audit of root `CLAUDE.md` against current repo truth (v0.1.24, bitácora
ESTADO ACTUAL, second-wave PRs pushed this session, integration plan v2). No
file was modified. Operator decides which edits to apply.

Sources cross-checked:

- `CLAUDE.md` (root, 216 lines)
- `bitacora.md` § ESTADO ACTUAL (top 120 lines)
- `docs/ops/integration-merge-plan-v2.md`
- `docs/ops/stale-branches.md`
- `docs/adr/0001-freemium-pivot.md`
- `Cargo.toml` workspace
- `git log` of the 6 session branches (`origin/feat/api-public-orders-web`,
  `origin/feat/api-stock-webhook`, `origin/feat/api-exec-dashboard`,
  `origin/feat/client-dashboard-caja-clientes`, `origin/feat/web-sync-interop`,
  `bench/pos-hot-path`)

Note: `docs/ops/coquimbo-golive-playbook.md` was referenced in the brief but is
**not present** in the working tree. Treat that finding as a gap if the doc was
expected to exist — could not cross-check against it.

---

## Section-by-section assessment

### Producto / Visión comercial
**Accurate.** Pillars (offline-first, multi-tenant, performance budget, vendor-
agnostic, errors in Spanish) all hold. The 9-module list still matches roadmap
intent; no second-wave PR removes anything. Performance budget "POS <50ms p99"
is now backed by `bench/pos-hot-path` Criterion harness (`c7a5417`) — worth a
one-line forward reference but not required.

### Modelo de negocio (freemium)
**Accurate.** Tier matrix lines (Free/Pro/Business/Enterprise) match ADR-0001
and `freemium-master-plan.md`. Invariants 1-7 unchanged. Microtx catalog
unchanged. License architecture bullet ("Pubkey embebida + 100% local") still
matches `crates/license` MVP. No drift.

### Roadmap (fases)
**Stale.** Fase 10 in `CLAUDE.md` is listed as the active item with sub-steps
10a-10d open. Bitácora ESTADO ACTUAL marks Fase 10 MVP **CIERRA** (PR #47 +
hot-reload). The doc should reflect Fase 10 = done (10e pending E2E with real
license) and Fase 9 = next blocker. Additionally, second-wave PRs introduce
items not in the phase list: HMAC public-orders ingest (#70), stock webhook +
ADR-0013 (#75), exec dashboard (#74), Tauri client views (#72), and
`bench/pos-hot-path` CI smoke (#69). These are sub-features of existing phases
(not new phases) but the roadmap section gives the impression nothing has
landed since the pivot — misleading for any agent reading top-down.

### Scope de este repo
**Accurate.** Boundary with `build-and-deploy-webdev-asap` still holds. ADR-
0013 stock-webhook is explicitly the ERP→web push direction, consistent with
"no cross-imports, no shared deploy".

### Stack
**Accurate today, will drift after 0.1.25.** Versions in `Cargo.toml`
workspace match the doc (axum 0.8, tower 0.5, tower-http 0.6, hyper 1.5,
utoipa 5, utoipa-axum 0.1, **utoipa-swagger-ui 8**, surrealdb 2.1,
jsonwebtoken 9, argon2 0.5, uuid 1.11, tokio 1.41, tracing 0.1,
tracing-opentelemetry 0.28, opentelemetry 0.27, opentelemetry-otlp 0.27,
axum-prometheus 0.7, tokio-cron-scheduler 0.13, async-nats 0.38,
windows-service 0.7, clap 4, config 0.14, chrono 0.4, thiserror 2, anyhow 1).
Workspace already has `arc-swap`, `base64`, `csv`, `tar`, `flate2`,
`ed25519-dalek`, `rand`, `bs58`, `sha2`, `hex`, `rpassword` which the doc does
**not** list — minor gap. Post-integration v2 plan upgrades `utoipa-swagger-ui`
to **9** and adds `governor`, `nonzero_ext`, `hmac`, `subtle`, `metrics`,
`metrics-util`, `reqwest`, `quick-xml`, `aes-gcm`, `argon2` (workspace dup),
`zeroize`. The crate table also misses `crates/domain`, `crates/agent`, and
`crates/license` which already exist in `[workspace] members`.

### Reglas siempre activas (1-8)
**Accurate but incomplete.** No rule is contradicted by session behavior.
However the session leaned heavily on `gh pr create --draft` + local gate
(billing-blocked CI), and operates under a *target-branch confirmation gate*
(`feature/erp-parity` vs `release/tufarmacia-golive`) per integration-merge-
plan-v2 §0. Neither is captured. Rule 6 says "ServiceComponents vacío hoy →
bloqueante M3" — verify: `installer/wix/main.wxs` was claimed to have
`ServiceInstall` + `ServiceControl` in the doc's own preamble line 117. These
two statements contradict each other inside `CLAUDE.md` itself.

### Modo de trabajo por defecto
**Not present as a named section.** `## Workflow` exists (lines 209-215) and
covers Plan mode + Explore subagents + verification. The session ran 5
parallel investigator agents, generated v2 of the merge plan, opened 6 PRs as
draft, and never amended a commit. The four-bullet `## Workflow` block does
not reflect any of that — neither prescriptive nor descriptive.

### Vault Obsidian
**Accurate paths.** `C:/Users/Administrator/Documents/obsidian-mind/` exists;
mapping table still useful. New strategy row (`docs/strategy/` + `docs/adr/`)
correctly points at this repo. `obs` binary on PATH (`notesmd-cli v0.3.5`).

### CLI-first
**Mostly accurate.** `cargo-wix-wix 0.3.9` confirmed installed (matches doc
line 117). `obs` version v0.3.5 confirmed. The `TODO: confirmar instalado`
note for cargo-wix on line 191 is stale — it *is* installed.

---

## Top 5 concrete edits suggested

1. **`CLAUDE.md` header (line 4)** — change `Fases 1-7 + 10(a-d) + 11(steps
   1-4) mergeadas ... Fase 10 license layer MVP CIERRA (PR #47)` to make
   Fase 10 unambiguously closed and surface second-wave PR set:
   `Fases 1-7 + 10 (incl. hot-reload) mergeadas; segunda ola PRs draft
   pendientes de merge (#69 bench-smoke, #70 public-orders HMAC, #72 Tauri
   client views, #73 web-sync-interop, #74 exec dashboard, #75 stock webhook
   ADR-0013); siguiente bloqueante = Fase 9 cert Authenticode`. **Why**:
   matches `bitacora.md` ESTADO ACTUAL and gives any new agent a correct
   landing context.

2. **`## Roadmap` Fase 10 block (lines 81-85)** — strike sub-steps 10a-10d
   (done) and replace with `Fase 10 — License MVP local ✅ (PR #47 +
   hot-reload). Pendiente: 10e E2E con license firmada real, CRL refresh,
   key prod (ambos en Fase 11).` **Why**: removes the strongest stale
   statement in the doc.

3. **`## Stack` crates table (lines 122-131)** — add three missing rows:
   `domain`, `agent`, `license`, each with a one-line rol. Also append the
   actually-present deps `arc-swap`, `base64`, `csv`, `tar`/`flate2`,
   `ed25519-dalek` family to the version list. **Why**: the doc claims to be
   "leído de `Cargo.toml` (workspace)" but is several months out of date
   relative to the actual `[workspace] members` and `[workspace.dependencies]`.

4. **`## Reglas siempre activas` rule 6 (MSI line 155)** — resolve the
   internal contradiction. Either (a) update to `installer/wix/main.wxs`
   carries `ServiceInstall` + `ServiceControl` + firewall TCP 8080
   (bloqueante M3 resuelto; resta firma Authenticode); or (b) drop the
   `ServiceComponents está vacío hoy` clause if the preamble at line 117 is
   the truth. **Why**: same file says both things. An autonomous agent will
   pick one at random.

5. **Add a `## Integration / merge target` rule** (new rule 9 or appended to
   rule 1) — capture the target-branch ambiguity codified in
   `docs/ops/integration-merge-plan-v2.md` §0: "Antes de cualquier merge en
   batch, confirmar target (`feature/erp-parity` vs `release/tufarmacia-
   golive`). Sin confirmación → STOP." **Why**: without this rule a future
   session will merge the second-wave PRs onto the wrong branch and lose the
   shipped JWT boot-guard collision context.

## Top 3 sections still accurate, keep verbatim

1. **Modelo de negocio (freemium, lockeado)** — invariants 1-7, tier matrix,
   microtx catalog, license architecture sub-bullets. Nothing in this session
   contradicts ADR-0001/-0002/-0005 or the strategy docs.
2. **Scope de este repo (IMPORTANTE)** — boundary against `build-and-deploy-
   webdev-asap` is still load-bearing and second-wave HMAC web-orders /
   stock-webhook are *exactly* the right shape (server-side HMAC, client repo
   stays separate).
3. **Vault Obsidian — leer bajo demanda** table — paths verified, mapping
   still correct, and the SessionStart hook reference is still valid.

## Could not verify (TBDs)

- **`docs/ops/coquimbo-golive-playbook.md`** referenced in the brief — file
  does not exist in the tree at audit time. Either the brief was speculative
  or the playbook was authored to a different path. Worth grepping
  `docs/coquimbo/` and the vault before concluding it is missing.
- **PR numbers #69-#75** — assumed from session context; GitHub was not
  queried (read-only constraint; would require `gh pr list`). Edit #1 should
  be re-checked against actual PR numbers before applying.
- **`utoipa-swagger-ui` final pin** — workspace shows `8`, integration plan
  v2 mandates `9`. If 0.1.25 has not landed yet the doc is still correct;
  after integration it becomes wrong. Re-check after first second-wave
  merge.

## Verdict

**Needs minor updates.** The doc is structurally sound and product/strategy
content (which is the load-bearing part for autonomous agents) is fully
accurate. The drift is concentrated in three places: (a) the header status
line and Roadmap Fase 10 block (stale post-PR #47 hot-reload), (b) the Stack
section's missing crates / deps, (c) an internal contradiction about MSI
ServiceComponents. Fixing the five edits above brings the doc back to truth;
no rewrite or restructure is warranted.

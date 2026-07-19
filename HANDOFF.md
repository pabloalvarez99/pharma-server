# HANDOFF — Continuar trabajo en RutBusiness (pharma-server)

> **Para Grok Build CLI (o cualquier agente/dev)**: este documento es el punto
> de entrada. Léelo completo antes de tocar código. Actualizado: 2026-07-19
> (A promote: **#325** → erp-parity · **#326** MERGED → main · **#324** closed).

### Estado rápido (2026-07-19 — A promote DONE)

| Item | Estado |
|---|---|
| **P0 stack** | **[PR #319](https://github.com/pabloalvarez99/pharma-server/pull/319) MERGED** → eature/erp-parity |
| **Variants UI pro** | **[PR #321](https://github.com/pabloalvarez99/pharma-server/pull/321)** + **[PR #325](https://github.com/pabloalvarez99/pharma-server/pull/325) MERGED** → eature/erp-parity @ **2fd309a** (polish); tip erp **0d1007** (+ #323 ariant_count GET) |
| **Promote main** | **[PR #326](https://github.com/pabloalvarez99/pharma-server/pull/326) MERGED** (squash) → main @ **84a6665** · CI PASS pre-merge |
| **Legacy promote** | **[PR #324](https://github.com/pabloalvarez99/pharma-server/pull/324) CLOSED** — CONFLICTING / superseded by #325/#326 |
| **A acción** | **PROMOTE DONE**. No force-push. Residual crates/assist/** never stage. |
| Smoke / client | 731 tests client verdes (claim C night); smoke P0 baseline 18 PASS |
| Residual assist (A) | Unstaged local — **nunca stagear** |
| **Policy** | No force-push · no keys · merge solo CI PASS |

### Variants UI pro — merged status

| Ref | SHA / URL |
|---|---|
| eature/erp-parity tip | 0d1007 	est(api): variant_count assert on parent product GET (#323) |
| #325 merge on erp-parity | 2fd309a eat(client): variants UI polish (Agotado, a11y, matrix thin) (#325) |
| #321 on erp-parity | 541ff6b eat(client): professional multi-SKU variants UI (#321) |
| main tip | 84a6665 eat(client): variants UI polish to main (resolve #324 conflict) (#326) |
| PR #325 | https://github.com/pabloalvarez99/pharma-server/pull/325 · MERGED → erp-parity |
| PR #326 | https://github.com/pabloalvarez99/pharma-server/pull/326 · MERGED → main |
| PR #324 | https://github.com/pabloalvarez99/pharma-server/pull/324 · CLOSED (superseded) |

**git log (erp-parity tip, real):**
`
a0d1007 test(api): variant_count assert on parent product GET (#323)
2fd309a feat(client): variants UI polish (Agotado, a11y, matrix thin) (#325)
541ff6b feat(client): professional multi-SKU variants UI (#321)
5da62ce feat: P0 ERP multi-rubro (attrs, rubro-pack, updater, ESC/POS) (#319)
`

**git log (main tip, real):**
`
84a6665 feat(client): variants UI polish to main (resolve #324 conflict) (#326)
bc84fd2 feat(client): professional multi-SKU variants UI (#321) (#322)
b4b7a9c Merge pull request #320 from pabloalvarez99/feature/erp-parity
5da62ce feat: P0 ERP multi-rubro (attrs, rubro-pack, updater, ESC/POS) (#319)
`

**Nota tips:** main y eature/erp-parity **no** comparten el mismo tip post-#326.
erp-parity lleva #323 (API ariant_count assert) además del stack client; main tiene el
client polish vía squash #326 sobre base #322. Re-sync futuro = merge/rebase erp→main o
cherry-pick #323 si se quiere paridad total en default branch.

**En scope (shipped #321 + #325 polish):**
1. Detalle producto: tabla variantes + modal **barcode-first** (createProductVariant)
2. Form nuevo producto: toggle **tiene variantes** → stock padre 0 + toast ES
3. POS: scan barcode → variante; guard padre con hijos; shells stock-0 clickeables
4. Modelo puro ariants-ui.ts + vitest denso; list badge ariant_count / ariants_stock
5. Polish: Agotado pills, a11y Esc/Enter, matrix chips thin, skeleton, multi-rubro honesty

**Fuera de scope / residual BLOCKED:**
- Edit/delete full API variantes — BLOCKED_API (stub uildEditVariantInput)
- Editor matriz bulk full talla×color — out of scope (solo chips thin)

### Demo 5 pasos (post-merge — tenant demo)

| # | Paso | Resultado esperado |
|---|---|---|
| 1 | Login dmin@demo.cl / demo1234, rubro **con stock físico** (tienda/minimarket) | Shell OK; variants UI solo si physicalStock |
| 2 | Inventario → **+ Nuevo producto** → *tiene variantes* → crear padre | Stock padre = 0; toast padre multi-SKU |
| 3 | Detalle padre → **+ Agregar variante** (barcode + talla/color) ×2; chips matrix opc. | Tabla con filas; skeleton mientras GET; Agotado si stock 0 |
| 4 | POS: escanear barcode hijo; click padre → error ES | Hijo al carrito; padre: «tiene variantes. Escanea…» |
| 5 | Cobrar línea de variante | Smoke cobro OK (no regresión caja) |

**Auto post-merge**
- [x] CI #326 uild + test (windows) PASS pre-merge
- [x] #326 squash-merged → main @ 84a6665
- [x] #324 closed superseded
- [ ] Manual demo 5 pasos en binario release (humano / on-call)

### Night ops log (A)

| UTC/local | Evento |
|---|---|
| 2026-07-19 ~15:05 | **A PROMOTE DONE**: #324 CLOSED superseded; #326 MERGED squash → main 84a6665; erp-parity tip 0d1007; HANDOFF docs |
| 2026-07-19 (C) | #325 MERGED → erp-parity @ 2fd309a; variants UI pro backlog agotado; 731 client tests |
| 2026-07-19 02:30 | agent cycle: erp=5da62ce main=b4b7a9c variants=#321 promote=#- merges=[none] |
| 2026-07-19 02:29 | cycle 5 erp=5da62ce main=b4b7a9c variantsPR=1 merges=[promote #320 → main] |
| 2026-07-19 02:00 | agent cycle: erp=5da62ce main=b4b7a9c variants=#321 promote=#- merges=[none] |
| 2026-07-19 01:58 | cycle 4 erp=5da62ce main=a6e6aa5 variantsPR=1 merges=[promote #320 → main] |
| 2026-07-19 01:30 | agent cycle: erp=5da62ce main=a6e6aa5 variantsPR=#321 promote=#320 merges=[none] |
| 2026-07-19 01:28 | cycle 3 erp=5da62ce main=a6e6aa5 variantsPR=1 merges=[none] |
| 2026-07-19 01:00 | agent cycle: erp=5da62ce main=a6e6aa5 #321/321 + #320/320 merges=[none] |
| 2026-07-19 00:58 | cycle 2 erp=5da62ce main=a6e6aa5 variantsPR=1 merges=[none] |
| 2026-07-19 ~00:35 | PR **#321** OPEN + CI IN_PROGRESS; tip ca4b853; QA checklist from PR only; WAIT merge; promote #320 still pending |
| 2026-07-19 00:28 | cycle 1 erp=5da62ce main=a6e6aa5 variantsPR=0 merges=[none] |
| 2026-07-19 ~00:26 | Cycle 1 pre-#321: no PR; residual dirty; HANDOFF |
---

## 1. Qué es este proyecto

**RutBusiness** (antes pharma-server): ERP/POS **offline-first** para
microempresas chilenas. Pivot en curso: de ERP de farmacia → **ERP global
multi-rubro** (core agnóstico + pack declarativo por rubro + fiscal SII
universal).

- **Server**: Rust, axum + SurrealDB embebida (`crates/`: `api`, `domain`,
  `dte`, `agent`, `pharma-core`, ...). Binario `pharma-api`, puerto 8080.
- **Client**: Tauri 2 (Rust + TS vanilla, Vite), workspace **independiente**
  en `client/src-tauri` (no forma parte del workspace raíz).
- **Fiscal**: DTE/SII completo (33/39/52/56/61, libro ventas, CAF).
- **Monetización**: tiers Free/Pro/Business/Enterprise, licencia Ed25519
  offline, gating HTTP 402.

## 2. Entorno de trabajo (Windows 11, máquina del dueño)

- **Worktree activo**: `D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\assist-b2`
  — post-merge #319: preferir base `origin/feature/erp-parity` @ `5da62ce`.
  Residual local assist **fuera del merge / no commitear en p0 dirty**.
- **DB del worktree**: `data/surreal` dentro del worktree. El server ancla la
  DB relativa a `C:\ProgramData\PharmaServer\data` (install dir) si no hay
  override → **siempre lanzar con `PHARMA__DB__PATH` absoluto** o usar
  `start-server.cmd` (raíz del worktree).
- **Demo**: tenant `demo`, sucursal `demo`, login `admin@demo.cl` /
  `demo1234`. Seed pharmacy (12 productos, lotes, proveedores, OC, ventas).
- **Accesos directos escritorio**: `RutBusinessServer.lnk` →
  `start-server.cmd` (wrapper con env). `RutBusiness.lnk` → cliente.
- **Servicios nssm**: `rutagent-demo` DETENIDO (ocupaba puerto 8080 y
  respawneaba un pharma-api viejo — si el server "se cierra solo", revisar
  esto primero). `rutagent-tunnel`, `tufarmacia-*` quedan, no bloquean.
- ⚠️ **sccache está roto** en esta máquina: `cargo check`/`cargo test` fallan
  en deps (`once_cell_polyfill`, `windows-sys`) con sccache. **Siempre
  prefijar `RUSTC_WRAPPER=""`** a comandos cargo.
- `cargo tauri` CLI no instalado globalmente; usar
  `client/node_modules/.bin/tauri`.

### Comandos esenciales

```bash
cd "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\assist-b2"
RUSTC_WRAPPER="" cargo check --workspace      # server
RUSTC_WRAPPER="" cargo test -p domain         # tests dominio
RUSTC_WRAPPER="" cargo test -p api            # tests api (integración)
cd client && npx tsc --noEmit                 # typecheck frontend
cd client && npm test                         # tests frontend (vitest/happy-dom)
cd client/src-tauri && cargo test             # tests Rust cliente (escpos, print)
./start-server.cmd                            # lanzar server con DB correcta
```

## 3. Estado P0 — qué está hecho y qué falta

Plan original (4 items): updater, attrs JSON, rubro-pack, hardware POS.

### ✅ P0.1 Tauri updater firmado — COMPLETO (código + runbook CDN)

- `client/src-tauri/Cargo.toml`: `tauri-plugin-updater = "2"`.
- `client/src-tauri/src/lib.rs`: plugin registrado.
- `client/src-tauri/tauri.conf.json`: `bundle.createUpdaterArtifacts: true` +
  `plugins.updater` (pubkey real, endpoint
  `https://cdn.pharma-server.cl/updates/rutbusiness/{{target}}-{{arch}}/{{current_version}}`,
  installMode `passive`).
- `client/src-tauri/capabilities/default.json`: `updater:default`.
- `client/src/updater.ts` + llamada en `main.ts` (check silencioso al boot,
  nunca bloquea login, fallos ignorados).
- **Keypair**: `client/keys/rutbusiness-updater.key` (privada, SIN password,
  gitignored) + `.pub`. Para firmar builds:
  `TAURI_SIGNING_PRIVATE_KEY=... TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" npm run tauri build`.
- ✅ **Runbook CDN** (Sesión C / C1): `docs/ops/cdn-updater.md` + script de
  staging local `scripts/publish-updater-artifacts.ps1` (copia a
  `dist-updater/`, genera `latest.json`, **no sube** al CDN real, no toca la
  privada).
- **FALTA (deploy humano)**: copiar `dist-updater/` a
  `cdn.pharma-server.cl` y publicar JSON por `{{current_version}}` vieja.
  Sin eso `check()` falla silencioso (by design).

### ✅ P0.2 product.attrs JSON — COMPLETO

- `migrations/0033_product_attrs.surql`: `DEFINE FIELD attrs` + back-fill de
  campos clínicos (laboratory, active_ingredient, etc.) dentro de `attrs`.
- `crates/domain/src/catalog/model.rs`: `Product.attrs: Option<Value>`
  (`#[serde(default, skip_serializing_if)]`).
- `crates/domain/src/catalog/repo.rs` + `crates/domain/src/inventory/repo.rs`:
  struct row con `attrs` (comentario explica rows viejas → `None`).
- Idea: campos fijos solo lo universal; cada rubro declara sus atributos en
  su pack → no más inflación de columnas (talla, duración servicio, etc.).
- Workspace compila: `cargo check --workspace` OK (con `RUSTC_WRAPPER=""`).

### ✅ P0.3 rubro-pack server-side — COMPLETO (código)

- `crates/domain/src/rubro.rs`: `RubroPack` (features, vocab, attrs, seed,
  coming_soon) + pack por cada rubro + fallback `otro` (NUNCA default
  farmacia). Tests incluidos.
- `crates/api/src/v1/rubro.rs`: `GET /api/v1/rubro-pack` (cualquier rol
  autenticado; lee `business.vertical` del tenant).
- Client: `client/src-tauri/src/commands/rubro.rs` (`rubro_pack` command,
  devuelve JSON crudo), `client/src/api/rubro.ts` (tipos + wrapper),
  `client/src/vertical.ts` (`loadRubroPack()` con fallback offline a
  constantes locales — LAN caída NUNCA rompe gating).
- ✅ `loadRubroPack` en shell post-login (`hydrateBranding`); `clearPackCache`
  en logout.
- ✅ Nav gatea con `visibleModulesForFeatures(featuresFromPack(...))`.
- ✅ POS / devoluciones / dashboard consumen `loadFeatures` / pack cache.
- ✅ `activeFeatures()` / `loadFeatures()` helpers; constantes locales quedan
  como offline fallback + previews de onboarding.

### ✅ P0.4 Hardware POS — COMPLETO (código + cajón mínimo)

- ✅ `client/src-tauri/src/escpos.rs`: builder ESC/POS puro (init/align/bold/
  double-size/feed/cut, CP437 para acentos/ñ, 58mm=32col, 80mm=48col). Tests.
- ✅ `client/src-tauri/src/commands/print.rs`: comando `print_ticket`
  (printer name + width58 + ReceiptInput) → spool RAW vía winspool
  (`PRINTER_HANDLE` / windows 0.61, solo `cfg(windows)`). Layout tests OK.
- ✅ `client/src/api/print.ts`: `printReceiptPreferThermal` + keys
  `rb.thermalPrinter` / `rb.thermalWidth58`.
- ✅ `views/pos.ts`: botón Imprimir → térmica si hay printer configurada,
  si no (o falla) `window.print()`.
- ✅ `views/configuracion.ts` Preferencias: nombre impresora + ancho 58/80 mm
  (localStorage por máquina).
- ✅ Barcode: keyboard-wedge ya funciona (sin código extra).
- ✅ **Cajón mínimo** (Sesión C / C2):
  - `escpos::drawer_kick_bytes()` = `ESC p 0 25 250` (+ tests).
  - `print_ticket(..., open_drawer?)` append kick al mismo job RAW.
  - comando `open_cash_drawer(printer)` para pulso suelto.
  - localStorage `rb.openDrawer` default **off**; checkbox en Preferencias
    térmica; `printReceiptPreferThermal` pasa el flag si está on.
- **FALTA (deploy / campo)**:
  1. Probar en impresora real Windows (TM-T20 / clones 58mm) + cajón RJ-11.

## 4. Verificación (Sesión C — CDN runbook + cajón)

```bash
cd "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\assist-b2"
cd client/src-tauri && RUSTC_WRAPPER="" cargo test   # ✅ 9/9 (print + escpos + drawer)
cd client && npx tsc --noEmit                        # ✅ PASS
# opcional: npm test ; RUSTC_WRAPPER="" cargo test -p domain / -p api
```

### Residuales Sesión C (2026-07-18) — HECHO sin rehacer P0

| Item | Estado | Artefactos |
|---|---|---|
| C1 Runbook CDN | ✅ | `docs/ops/cdn-updater.md`, `scripts/publish-updater-artifacts.ps1` |
| C2 Cajón ESC p | ✅ | `escpos::drawer_kick_bytes`, `open_cash_drawer`, flag `open_drawer`, `rb.openDrawer` |

**No rehacer** (ya estaba): updater plugin/keys, `print_ticket` base, layout
POS, Preferencias impresora (solo se añadió checkbox cajón), `loadRubroPack`.

**Git / PR (A — 2026-07-19)**:
- **[PR #319](https://github.com/pabloalvarez99/pharma-server/pull/319) MERGED** (`feat/p0-erp-global` → `feature/erp-parity`).
  - Merge commit: **`5da62ce`**. Pre-merge tip: **`ca655d9`**.
  - CI verde al merge; fixes clave: clippy rubro `36a029a`, firstrun `98d2d7e`, assist `attrs` `c05cbcc`.
- Smoke **18 PASS**; rubro-pack **200** con release rebuild.
- **En base (B/C):** variants multi-SKU (mig 0034+), catalog-POS pack UX, product.attrs persist.
- **Épica A agent:** residual §4.1 local unstaged; plan §4.2 listo. Siguiente: **"go agent épica"** → branch nueva desde `origin/feature/erp-parity` @ `5da62ce`.
- No stagear `.git.orphaned-pointer` ni `client/keys/*`. No force-push.

### 4.1 Residual dirty — inventario + riesgo (A, 2026-07-18)

Worktree único `assist-b2` tiene **tres dueños** en el working tree. A solo opera carril assist/docs; no stagea B/C.

#### Carril A — agent/assist (**fuera de PR / no commitear aún**)

| Path | Δ | Notas |
|---|---|---|
| `crates/assist/Cargo.toml` | M | quita `reqwest` |
| `crates/assist/src/lib.rs` | M | drop mod `llm` |
| `crates/assist/src/llm.rs` | **D** | elimina proveedor Anthropic/BYO-key (~344 LOC) |
| `crates/assist/src/provider.rs` | M | `select_provider` siempre Deterministic; warn si llm opt-in |
| `crates/assist/src/intent.rs` | M | +`CajasAbiertas`/`SaldoCaja`/`VentasPorSucursal`; drop varios intents farmacia-depth |
| `crates/assist/src/deterministic.rs` | M | handlers caja + redirect honesto ventas-por-sucursal |
| `crates/assist/src/actions.rs` | M | Wave 3.1 breadth: `ajustar_stock`/`abrir_caja`/`aplicar_descuento`/`registrar_pago_proveedor`; drop otras acciones |
| `crates/assist/tests/actions.rs` | M | +tests integration Wave 3.1 |
| `crates/assist/tests/deterministic.rs` | M | +tests reads caja |
| `crates/api/src/v1/assist.rs` | M | sin `load_assist_config`; no lee settings LLM |
| `docs/adr/0016-agent-assist-architecture.md` | M | +§ Wave 3.1 |
| **Stat A** | | **~10 files, +1476 / −2205** (net simplifica + reordena) |

**Riesgo si se commitea en `feat/p0-erp-global` (#319):** **ALTO**
- Contamina PR P0 multi-rubro con épica agent (scope review/CI explode).
- `llm.rs` deleted + `reqwest` out = decisión producto/ADR; no “drive-by” en P0.
- Diff actions/intent reescribe superficie del agente; rompe expectativas de branches viejas (`origin/feat/assist-*`).

**vs branch `feat/agent-assist`:** **no existe** en remote. Cercanas:
- `origin/feat/assist-mvp-agent`, `feat/assist-actions`, `feat/assist-actions-breadth`, `feat/assist-depth-intents` (históricas; base lejana a `feat/p0-erp-global`).
- Plan: al **go agent épica**, cortar `feat/agent-assist` (o `feat/assist-wave-3-1`) **desde tip verde post-#319** (o `feature/erp-parity` mergeado), **cherry-pick/apply solo paths A**; no rebase del monstruo P0+B+C dirty.

#### Carril B — variants / domain (en `feature/erp-parity` via #319)

**Merged:** mig 0034+, domain multi-SKU, API variants, firstrun/clippy fixes. A no edita estos paths.

#### Carril C — catalog / POS client (en `feature/erp-parity` via #319)

**Merged:** pack-driven form attrs, POS multi-rubro polish, inventory gating. Residual local C no stagear por A.

#### Ops / ruido (no stagear a ciegas)

| Path | Acción |
|---|---|
| `.git.orphaned-pointer` | **Nunca** stagear |
| `client/keys/*` | gitignored; **nunca** stagear |
| `CLAUDE.md`, `bitacora.md`, `Cargo.lock`, `docs/ops/cdn-updater.md`, `scripts/qa/tauri-smoke.md` | residual local; no mezclar con agent salvo decisión explícita |

### 4.2 Plan 4 commits — épica agent (propuesta A; **sin codear aún**)

Base: branch nueva desde tip CI-verde de #319 / post-merge. Solo paths §4.1 carril A (+ tests assist). Verificar por commit: `RUSTC_WRAPPER="" cargo test -p assist` y `cargo clippy -p assist -- -D warnings`.

| # | Commit (mensaje propuesto) | Paths | Qué entrega |
|---|---|---|---|
| **1** | `refactor(assist): drop in-tree LLM provider; always-deterministic seam` | `llm.rs` delete, `Cargo.toml` (−reqwest), `lib.rs`, `provider.rs`, `api/v1/assist.rs` | Offline-first por defecto; seam ADR-0016 intacta (`AssistConfig` + warn); sin network en build |
| **2** | `feat(assist): Wave 3.1 write actions (stock, caja, descuento, pago proveedor)` | `actions.rs` + `tests/actions.rs` | Whitelist writes reusando domain existente; parse conservador es-CL; tests propose/confirm/reject |
| **3** | `feat(assist): read intents cajas_abiertas / saldo_caja + honest redirect sucursal` | `intent.rs`, `deterministic.rs`, `tests/deterministic.rs` | Reads seguros; `VentasPorSucursal` no inventa montos |
| **4** | `docs(adr): Wave 3.1 assist breadth + gap ventas-por-sucursal` | `docs/adr/0016-agent-assist-architecture.md` | Documenta acciones, candados, gaps datos (branch/vendedor) |

**Fuera de estos 4 (follow-up):** reintroducir LLM opt-in (ADR-0017) en commit aparte; domain service “ventas por sucursal” (B); client ask-bar UX (C/otra lane).

**Gate:** #319 **MERGED** + CI verde. Orquestador: **"go agent épica"**.

## 5. Convenciones del codebase (respetar)

- **Español** en strings de usuario; inglés en identificadores/`code`.
- **Money = STRING** en el wire (nunca float JSON).
- JWT solo en memoria (`SessionState`), nunca disco.
- Comentarios de módulo explican el PORQUÉ (ver ejemplos en `rubro.rs`,
  `escpos.rs`). Mantener densidad/estilo.
- Tests junto al código (`#[cfg(test)] mod tests`).
- Migraciones SurrealDB numeradas: próxima libre = **0034**.
- Rust client usa `windows` crate 0.61, `reqwest` rustls (NO OpenSSL).
- Clippy estricto en server (warnings en CI): correr
  `cargo clippy --workspace` antes de commit.

## 6. Roadmap después de P0 (contexto estratégico)

- **En vuelo ahora**:
  1. **B/C** — variants + catalog-POS **merged** en `feature/erp-parity` (#319).
  2. **A** — agent épica **lista** (plan §4.2); residual unstaged → **"go agent épica"**.
- **P1** (1-2 meses, UN rubro a la vez, validar con cliente real):
  1. Tienda/Retail (variantes+SKU, etiquetas) — rubro más común de Chile.
  2. Café/Restaurant (comandas + BOM).
  3. Belleza/servicios (agenda + reservas).
- **P2**: Webpay Oneclick (suscripciones), license-server prod (Vercel),
  Webpay POS en punto de venta.
- **P3**: multi-bodega, sync online opt-in, storefront genérico, onboarding
  <10 min.
- **P4**: LATAM (abstraer fiscal-engine por país) — no antes de P3.
- Disciplina (docs/strategy/rubro-catalog.md §6.2): 1 rubro validado antes
  del siguiente. Farmacia es el moat — no desatender Tier-S farmacia.
- Docs de estrategia: `docs/strategy/` (scaling-architecture, payments-cl,
  latam-master-plan, license-server-spec), `docs/adr/`.

## 7. Riesgos conocidos

- sccache roto → `RUSTC_WRAPPER=""` siempre.
- Server "se cierra solo" → servicio nssm viejo ocupando 8080.
- DB equivocada → falta `PHARMA__DB__PATH` absoluto.
- Updater sin CDN publicado → `check()` falla silencioso (esperado).
  Staging local: `pwsh scripts/publish-updater-artifacts.ps1` (ver
  `docs/ops/cdn-updater.md`); upload es paso humano.
- Claves: `client/keys/rutbusiness-updater.key` es la ÚNICA copia de la
  privada de firma — respaldar fuera del repo. Nunca en CDN ni en git.
- Cajón: sin cable RJ-11 / sin checkbox, no hay pulso (default off).

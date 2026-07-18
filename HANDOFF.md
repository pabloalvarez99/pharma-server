# HANDOFF — Continuar trabajo en RutBusiness (pharma-server)

> **Para Grok Build CLI (o cualquier agente/dev)**: este documento es el punto
> de entrada. Léelo completo antes de tocar código. Escrito: 2026-07-18, tras
> ejecutar P0 del plan "ERP global para microempresas en Chile".

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
  — TODO el trabajo P0 está aquí, SIN COMMITEAR. El repo principal está en
  `D:\Respaldo Proyectos\GitHub\pharma-server` (no mezclar).
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

**Git worktree roto**: `assist-b2` apunta a
`D:/Respaldo Proyectos/GitHub/pharma-server/.git/worktrees/pharma-wt-assist-b2`
pero el repo padre `pharma-server` **no está en disco**. No hay commit hasta
restaurar el repo o re-adjuntar el worktree. Código P0 vive en el filesystem
del worktree, sin commitear.

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

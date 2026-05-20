# pharma-server — Project Context

Servidor Rust on-prem para ERP de farmacia. Single binary instalable vía MSI, axum HTTP API + SurrealDB embedded (kv-surrealkv) + Windows service. Producto **vendible separado** de Tu Farmacia.
**Estado**: v0.1.24 · branch `feature/erp-parity` · Fases 1-7 + 10(a-d) + 11(steps 1-4) mergeadas · **MSI release** v0.1.23 (https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.23, 12.30 MB; no MSI nuevo para 0.1.24 por CI billing) · ecosistema agentes COMERCIA end-to-end · **PIVOTE freemium MSI (2026-05-20)** → ver `docs/strategy/freemium-master-plan.md` · **Fase 10 license layer MVP CIERRA (PR #47)**: `crates/license` Ed25519 offline + 402 + CLI + 1 endpoint gated POC.

**Visión extendida (2026-05-16, actualizada 2026-05-20)** → ver [`docs/strategy/ecosystem-roadmap.md`](./docs/strategy/ecosystem-roadmap.md). Pharma-server no es solo ERP vendible; es **nodo de un ecosistema federado de agentes ERP** (farmacias, proveedores, droguerías) donde humanos reales operan cada nodo y transan vía protocolo común (Ed25519-signed JSON envelopes sobre HTTP/NATS). El modelo comercial es **freemium MSI Windows estilo LoL** (core gratis + tiers + microtx) — ver [`docs/strategy/freemium-master-plan.md`](./docs/strategy/freemium-master-plan.md) y [ADR-0001](./docs/adr/0001-freemium-pivot.md). Fase 13 = capa de confianza/marketplace B2B → ver [`docs/strategy/b2b-marketplace.md`](./docs/strategy/b2b-marketplace.md).

## Producto / Visión comercial

**Meta**: ERP **profesional para farmacias**, vendible como licencia on-prem (MSI + soporte). Comprador: farmacias independientes y cadenas chicas que quieren todo local, sin SaaS, sin cloud, sin lock-in.

Pilares de venta (no negociables):
- **Instalación 1 click** (MSI firmado, sin dependencias externas, sin Docker, sin Postgres aparte).
- **Offline-first**: opera sin internet. LAN-only. Datos siempre en la farmacia.
- **Multi-tenant** (una instalación, N sucursales/locales o N clientes en SaaS-en-VPS opcional).
- **Cumplimiento local CL**: boleta electrónica SII, libro de controlados ISP/DEIS, recetas magistrales, vencimientos, lotes, trazabilidad.
- **Auditoría completa**: cada cambio de stock/precio/venta queda en log inmutable.
- **Performance**: POS responde <100ms incluso con 50k SKUs. SurrealKv embedded = sin red en hot path.
- **Vendor-agnostic**: exporta CSV/JSON sin formatos propietarios. El cliente es dueño de sus datos.

Módulos objetivo (roadmap producto, no scaffold):
1. **Inventario**: SKU, lote, vencimiento, stock por bodega, alertas mínimos.
2. **POS / Ventas**: tickets, medios de pago, devoluciones, descuentos, convenios isapre.
3. **Compras**: OC, recepción, costo promedio ponderado.
4. **Recetas**: receta retenida, receta cheque, controlados (Ley 20.000).
5. **Caja**: apertura/cierre, arqueo, diferencias.
6. **Reportes**: ventas, márgenes, rotación, ABC, vencimientos próximos.
7. **Integraciones (opt-in)**: SII (DTE), ISP, transbank/getnet, balanza, lector códigos.
8. **Backup**: snapshot SurrealKv programado + restore guiado.
9. **Usuarios/roles**: cajero, químico, admin, dueño. Permisos por módulo.

Modelo de negocio: ver § "Modelo de negocio (freemium, lockeado)" abajo.

Reglas de diseño derivadas:
- **No agregar dependencia cloud** sin opción de operar offline.
- **No romper compat de DB** sin migración automática (cliente NO debe perder datos al actualizar).
- **UI desacoplada**: el server expone API HTTP/JSON estable y versionada (`/api/v1/...`). Frontend (POS, admin) es cliente separado.
- **Errores en español** en respuestas user-facing (códigos en inglés OK para devs).
- **Performance budget**: endpoints POS <50ms p99 en hardware mínimo (i3 + SSD + 8GB).

## Modelo de negocio (freemium, lockeado)

Decidido 2026-05-20. **Pivote** de licencia única → **MSI Windows freemium estilo LoL**: core gratis + tiers pagos + microtransacciones one-time. Detalle completo en [`docs/strategy/freemium-master-plan.md`](./docs/strategy/freemium-master-plan.md). Decisión raíz: [ADR-0001](./docs/adr/0001-freemium-pivot.md).

**Tiers** (resumen — ver master plan §3 para matrix completa):
- **Free** — POS + inventario + caja + gastos + recetas + backup local + sales-daily + 1 caja + 1 sucursal.
- **Pro** — 3 cajas, reportes margins/top-products, integraciones via microtx.
- **Business** — 10 cajas, 5 sucursales, sync online, SII/ISP auto, federación quote+PO.
- **Enterprise** — ilimitado, white-label, multi-cluster, SLA 4h.

**Microtx one-time** (catálogo cerrado v1): Branding pack, SII unlock, Telegram bot, Premium reports pack, Extra cashier seat, Premium support credits.

**Invariantes NO negociables** (codificados en [ADR-0005](./docs/adr/0005-core-gratis-no-locked-in.md)):
1. Core ERP siempre gratis offline. Capacidades sólo se *agregan* al Free, nunca se quitan.
2. License OFFLINE-FIRST — server NO requiere internet para operar features ya activadas.
3. Telemetría OPT-IN siempre, default OFF, sin PII (Ley 19.628).
4. Sin lock-in de datos — Free incluye export CSV/JSON completo de todo.
5. Sin dark patterns — máx 1 upgrade prompt/sesión, cero en POS hot path.
6. Sin kill-switch remoto — core gratis sigue operativo aunque license expire/revoque.
7. Compromiso de continuidad — si la empresa cierra, last release queda funcional indefinida.

**Arquitectura técnica del licenciamiento** ([`docs/strategy/license-architecture.md`](./docs/strategy/license-architecture.md), [ADR-0002](./docs/adr/0002-license-ed25519-offline.md)):
- License = JSON Ed25519-firmado (reusa `crates/agent/identity.rs` + `envelope.rs`).
- Pubkey del licenser embebida en binario. Validación 100% local.
- Feature gate API: `entitled(feature) -> bool` + `require(feature) -> Result` (retorna 402 `FEATURE_REQUIRES_UPGRADE`).
- CRL firmado distribuido por CDN ([ADR-0006](./docs/adr/0006-revocation-strategy-signed-crl.md)). Refresh opcional.
- Key rotation multi-key con `key_id` ([ADR-0007](./docs/adr/0007-key-rotation-licenser.md)).
- License-server vive en **repo separado** `pharma-license-server` ([ADR-0004](./docs/adr/0004-license-server-separado.md)).

**Pagos** ([`docs/strategy/payments-cl.md`](./docs/strategy/payments-cl.md), [ADR-0003](./docs/adr/0003-payments-webpay-first.md)):
- Webpay primario (Pro/Business sub + microtx CL).
- Stripe secundario (microtx con tarjeta internacional, Fase 11.1).
- Khipu para Enterprise (Fase 11.2).
- Mercado Pago para LATAM multi-país (Fase 11.3+).

## Roadmap (fases)

Renumerado 2026-05-20 post-pivote. Estado en `bitacora.md` ## BACKLOG.

- **Fase 9** — MSI vendible v1.0.0 (firma Authenticode + smoke VM Windows limpia). **BLOQUEADO por cert**.
- **Fase 10** — License/entitlement layer:
  - 10a `crates/license` crate nuevo (Ed25519 verify + parser, reusa `crates/agent`).
  - 10b Feature gate API (`entitled`/`require`) + `ApiError::payment_required` 402.
  - 10c CLI `pharma license import|status|features`.
  - 10d 1 feature gated POC (sugerencia: `reports.margins_daily`).
- **Fase 11** — Payment rails + license-server integration (repo separado, online activation):
  - 11a `pharma-license-server` skeleton (Next.js + Postgres).
  - 11b Webpay integration (Pro/Business sub).
  - 11c Stripe Checkout (microtx).
- **Fase 12** — Sync online opt-in entre nodos (paid tier).
- **Fase 13** — Marketplace federado B2B ([`docs/strategy/b2b-marketplace.md`](./docs/strategy/b2b-marketplace.md)). Capa de confianza/reputación.
- **Fase 14** — Cloud companion (web admin + mobile dashboard, opt-in).

## Scope de este repo (IMPORTANTE)

Este repo (`pharma-server`) es **exclusivamente para el servidor Rust on-prem genérico**. Producto a empaquetar como MSI y vender a farmacias que quieren todo local (sin cloud, vendor-agnostic, offline-first).

Repos relacionados, **separados a propósito**:

- `pabloalvarez99/pharma-server` (este, privado) → Servidor Rust on-prem experimental. Un MSI, N tenants en LAN, SurrealDB embedded.
- `pabloalvarez99/build-and-deploy-webdev-asap` → Tu Farmacia LOCAL REAL en Coquimbo. Next.js 14 + Cloud SQL Postgres 15 + Firebase + Vercel. Cliente real en producción. Ver `C:/Users/Administrator/Documents/GitHub/build-and-deploy-webdev-asap/`.

Regla: cualquier feature de servidor on-prem genérico va aquí. Cualquier cosa de la farmacia real de Coquimbo va al otro repo. **No cross-imports, no shared CI, no shared deploy, no mezclar deps.**

## Stack (esencial)

Versiones leídas de `Cargo.toml` (workspace). MSRV vs pin: `rust-version = 1.85` (MSRV — código compila desde 1.85+) y `rust-toolchain.toml = 1.95.0` (versión que usa dev/CI). Compatible por diseño.

- Rust 1.95.0 pin (`rust-toolchain.toml`) · MSRV 1.85 · edition 2021 · target `x86_64-pc-windows-msvc`
- axum 0.8 + tower 0.5 + tower-http 0.6 + hyper 1.5
- utoipa 5 + utoipa-axum 0.1 + utoipa-swagger-ui 8
- surrealdb 2.1 con feature `kv-surrealkv` (LSM puro Rust, evita libclang/RocksDB en Windows)
- jsonwebtoken 9 (HS256) + argon2 0.5 + uuid 1.11
- tokio 1.41 · tracing 0.1 · tracing-opentelemetry 0.28 · opentelemetry 0.27 · opentelemetry-otlp 0.27 · axum-prometheus 0.7
- tokio-cron-scheduler 0.13 · async-nats 0.38 (ambos sin uso real todavía)
- windows-service 0.7 · clap 4 · config 0.14 · chrono 0.4 · thiserror 2 · anyhow 1
- MSI: cargo-wix 0.3.9 + WiX v3.14 (`installer/wix/main.wxs` con `ServiceInstall` + `ServiceControl` + firewall TCP 8080; smoke install/uninstall verificado)
- CI: GitHub Actions windows-latest (`.github/workflows/ci.yml`)

Crates (`Cargo.toml` raíz):

| Crate | Rol |
|-------|-----|
| `core` | Domain types (`TenantId`), `Error` enum (thiserror), `AppConfig` loader (config crate) |
| `db` | SurrealDB client (`Surreal<LocalDb>` + SurrealKv), migración runner con tracking `_migrations` |
| `api` | Axum HTTP server. Bin `pharma-api`. Expone `lib::run` para que service lo embeba |
| `auth` | JWT issue/verify (HS256, validación issuer) + argon2id password hash/verify |
| `jobs` | Cron scheduler (vacío hoy) |
| `telemetry` | `tracing_subscriber` JSON + EnvFilter + OTLP gRPC tonic exporter (opt-in vía `PHARMA__OTLP__ENDPOINT`) |
| `service` | Windows service host, name `PharmaServer`, type `OWN_PROCESS`. Bin `pharma-service` |
| `cli` | Admin CLI. Bin `pharma`. Comandos: `migrate`, `config`, `tenant-create`, `user-create` (argon2id, password vía flag/`PHARMA_PASSWORD`/prompt) |

## Reglas siempre activas

1. **Build local antes de push**:
   ```powershell
   cargo build --workspace --release
   cargo test --workspace
   ```
   Cargo cache vía Swatinem en CI. Local: dejar `target/` cacheado, no borrar entre branches.

2. **Pre-commit obligatorio** (mismo set que CI con `-D warnings`):
   ```powershell
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

3. **Migraciones append-only**: NUNCA editar `migrations/NNNN_*.surql` ya aplicada. Naming `NNNN_descripcion.surql`. id en tracking = filename stem (e.g. `0001_init`). Para cambiar schema → nueva migración `NNNN+1_*`.

4. **Multi-tenant obligatorio**: cada tabla de dominio nueva DEBE llevar campo `tenant: record<tenant>` + índice compuesto que incluya `tenant`. Patrón en `migrations/0001_init.surql` (`user`, `session`). Todo filtrado por tenant via JWT claim `tenant_id`.

5. **Windows service**: probar ciclo completo `sc create / start / stop / delete` (o `pharma service ...` cuando se implemente) en VM antes de cortar MSI. Service install requiere admin elevado. Service y CLI no deben correr a la vez sobre `./data/surreal` (SurrealKv file lock).

6. **MSI**: SemVer estricto en `workspace.package.version`. Firmar con cert si está disponible (sin firmar = SmartScreen warning). Smoke-test instalación limpia + upgrade (MajorUpgrade ya en wxs). `installer/wix/main.wxs` `ServiceComponents` está vacío hoy → bloqueante M3.

7. **Bitácora dual**: cada cambio significativo → append en (a) `bitacora.md` (repo, commit history) y (b) `C:/Users/Administrator/Documents/obsidian-mind/work/active/pharma-server/bitacora.md` (vault, búsqueda). Después actualizar `work/active/pharma-server/decisions-log-index.md` con la línea nueva.

8. **Secrets**: nunca commitear `config/local.toml` ni `data/`. JWT secret de `config/default.toml` (`change-me-in-production`) es placeholder; producción inyecta vía env `PHARMA__JWT__SECRET`. Loader: `config/default.toml` → `config/local.toml` (opcional) → env `PHARMA__*` separator `__`.

## Vault Obsidian — leer bajo demanda

Ubicación: `C:/Users/Administrator/Documents/obsidian-mind/`

| Tarea actual | Leer primero |
|---|---|
| Tocar `crates/db/`, `migrations/` o `*.surql` | `reference/pharma-server-db.md` |
| Tocar `crates/api/` (rutas axum, middleware, handlers) | `reference/pharma-server-api.md` |
| Tocar `crates/cli/` | `reference/pharma-server-cli.md` |
| Tocar `crates/service/` | `reference/pharma-server-msi.md` + `brain/pharma-server-gotchas.md` |
| Tocar `installer/` o `*.wxs` | `reference/pharma-server-msi.md` |
| Tocar `.github/workflows/` | `reference/pharma-server-ci.md` |
| Tocar `config/`, `rust-toolchain.toml` o env | `reference/pharma-server-env.md` |
| Histórico / decisiones pasadas | `work/active/pharma-server/decisions-log-index.md` → `bitacora.md` |
| Patrones Rust del proyecto | `brain/pharma-server-patterns.md` |
| Antes de debug Windows-specific | `brain/pharma-server-gotchas.md` |
| Visión producto / por qué existe | `brain/pharma-server-north-star.md` |
| Arquitectura general (crates, flujo, multi-tenant) | `reference/pharma-server-architecture.md` |
| Decisiones técnicas (por qué X) | `brain/pharma-server-decisions.md` + repo `docs/adr/` |
| **Modelo de negocio / freemium / licencia / pagos** | repo `docs/strategy/` + `docs/adr/` |

SessionStart hook (`.claude/hooks/vault-hint.sh`) sugiere refs según archivos cambiados — leer hints, NO duplicar lectura.

## CLI-first (PRIORIDAD MÁXIMA)

**Toda operación → CLI primero.** No GUI, no MCP, no clicks. Si falta CLI → `cargo install <pkg>` o `choco install <pkg>` y continuar.

CLIs esperadas (verificar versiones en sesión inicial):

- `cargo` `rustup` `git` `gh` — toolchain + repo
- `cargo-wix` — MSI build (TODO: confirmar instalado: `cargo wix --version`)
- `rg` `fd` `jq` `bat` `glow` — search/file/render
- `obs` (`/c/Users/Administrator/bin/obs.exe`) — Obsidian vault CRUD
- `rtk` (si disponible, prefijo para comandos pesados)

Listar versiones reales antes de fallback manual.

## Vault tooling

- **`obs`** (Yakitrak/notesmd-cli):
  - `obs search-content "<term>"` — full-text search vault
  - `obs search "<note>"` — fuzzy find
  - `obs print "<note>"` — imprimir contenido
  - `obs create/move/delete/frontmatter/daily/list` — CRUD
- `glow <path>` — render markdown terminal
- `rg <q> C:/Users/Administrator/Documents/obsidian-mind/` — alt rápido
- Abrir en app Obsidian: `start "" "obsidian://open?vault=obsidian-mind&file=<NoteName>"`

## Workflow

- Plan mode para tareas no-triviales (3+ pasos / decisión arquitectural).
- Subagentes (Explore) para research/exploración paralela.
- Tras corrección del usuario → registrar lección en `tasks/lessons.md` (crear cuando exista primera) y/o `brain/pharma-server-gotchas.md`.
- Verificar antes de marcar completo. Senior dev standard.
- **CLI-first siempre**. Si falta CLI → recomendar `cargo/choco install <pkg>` y continuar.

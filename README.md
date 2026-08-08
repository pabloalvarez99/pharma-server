# RutBusiness

[![ci](https://github.com/pabloalvarez99/pharma-server/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/pabloalvarez99/pharma-server/actions/workflows/ci.yml)
[![audit](https://github.com/pabloalvarez99/pharma-server/actions/workflows/audit.yml/badge.svg?branch=main)](https://github.com/pabloalvarez99/pharma-server/actions/workflows/audit.yml)
[![license: Proprietary](https://img.shields.io/badge/license-Proprietary-red.svg)](./Cargo.toml)

> **Producto:** RutBusiness (multi-rubro). **Repo git:** `pharma-server` (nombre histórico; rename físico pendiente de go del fundador).

ERP/POS **offline-first** para microempresas chilenas: 1 RUT = 1 negocio = 1 agente.
Farmacia es el **beachhead**, no el límite ni la marca. Server Rust on-prem + clientes
operador + API HTTP `/api/v1`. Modelo freemium MSI (core gratis + tiers) - [ADR-0001](./docs/adr/0001-freemium-pivot.md).

## Qué hay hoy (main)

No es un scaffold. En `main` vive el ERP usable en desarrollo:

| Superficie | Path | Rol |
|---|---|---|
| **Server / API** | `crates/` (~14 crates) | axum + SurrealKV embebida, JWT, dominio ERP, DTE, license, agent, jobs, MSI service |
| **Android operador** | `client-android/` | Kotlin + Jetpack Compose nativo - cliente móvil de primera clase ([ADR-0021](./docs/adr/0021-android-compose-nativo.md)) |
| **Desktop Windows** | `client/` (Tauri 2) | MSI escritorio; mismo frontend TS que la web |
| **Web operador (PWA)** | `client/` + `crates/api/static/` | Operador en navegador / Free Web servido por el API |
| **CLI admin** | `crates/cli` | migrate, seed, usuarios, license |

Módulos de dominio en main (no exhaustivo): inventario multi-SKU/variants, POS, compras/OC,
caja, fiado, reportes, multi-sucursal, rubro-pack, country-pack (moneda/impuesto por tenant),
capa license Ed25519, rieles de agente/Fase 13 parciales. Suite workspace del orden de
**1000+ tests** (audit 2026-08-08: 1019 passed).

## Qué NO es

- No es un esqueleto vacío ni "solo API de farmacia".
- No es SaaS-only: **offline-first on-prem** es el core; cloud companion es opt-in.
- No es un solo frontend universal en móvil: Android es **Compose nativo**; Tauri/TS
  sigue vivo para **desktop + web** (ADR-0015 superado en Android por ADR-0021).
- No renombra crates/binarios/MSI sin go explícito del fundador (`pharma-api`,
  `PharmaServer`, etc. son nombres de repo/instalador).

## Cómo se levanta (dev)

```powershell
# Server (puerto 8080). En Windows, si sccache falla: $env:RUSTC_WRAPPER = ""
cargo run -p api
curl http://localhost:8080/health/live

# O wrapper con DB del árbol de trabajo (ajustá paths en el .cmd si hace falta):
# .\start-server.cmd
```

```powershell
# Desktop / web operador
cd client
npm install
npm run tauri dev    # escritorio
# npm run build      # typecheck + Vite (PWA/web)
```

```powershell
# Android (server ya corriendo). Emulador: http://10.0.2.2:8080
cd client-android
.\gradlew assembleDebug
# Instalá el APK del ABI del aparato (no hay APK universal - piso de hardware)
```

Primera cuenta: `POST /api/v1/setup` (`business_name`, `tenant_slug`, `email`, `password`).
No hay contraseña default commiteada. Demo histórico (si está seedado): ver docs/ops y CLI seed.

**DB:** SurrealKV en path configurable (`PHARMA__DB__PATH`). Servicio Windows e instancia
dev no pueden compartir el mismo directorio (file lock).

## Crates (server)

| Crate | Rol |
|---|---|
| `api` | Axum HTTP (`cargo run -p api`), OpenAPI, static Free Web |
| `domain` | Invariantes de negocio (inventario, ventas, caja, fiado, rubro, …) |
| `core` | Tipos, errores, config |
| `db` | SurrealDB client + repos + migraciones |
| `auth` | JWT + argon2id, claims multi-tenant |
| `dte` | Boleta/factura electrónica SII (nativo Rust) |
| `license` | Entitlements Ed25519 offline + 402 |
| `agent` | Identidad/envelopes firmados, assist |
| `assist` | Capa assist del agente |
| `jobs` | Cron / workers |
| `sync` | Sync / seams interop |
| `telemetry` | tracing + OTLP + Prometheus |
| `service` | Windows service + embebido del API |
| `cli` | Admin CLI |

## Stack

- Rust 1.95 pin (`rust-toolchain.toml`) · MSRV 1.85 · axum 0.8 · SurrealDB 2.x `kv-surrealkv`
- Cliente desktop/web: Tauri 2 + TypeScript + Vite (`client/`)
- Cliente móvil: Kotlin, Jetpack Compose, minSdk 23 (`client-android/`)

## MSI / servicio Windows

Release histórico: [v0.1.23](https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.23).
Build local: `cargo build --release -p service` + `cargo wix` (WiX v3). Detalle y smoke en
[`docs/ops/`](./docs/ops/) y `installer/`.

## Producción on-prem

- [`config/production.toml.example`](config/production.toml.example)
- [`docs/ops/production-checklist.md`](docs/ops/production-checklist.md)
- [`docs/ops/backup-restore.md`](docs/ops/backup-restore.md)

## Docs de entrada

| Doc | Para qué |
|---|---|
| [`CLAUDE.md`](./CLAUDE.md) | Contexto agentes, directivas del fundador, stack |
| [`docs/adr/`](./docs/adr/) | Decisiones arquitectónicas (inmutables) |
| [`docs/strategy/`](./docs/strategy/) | Visión, freemium, multi-rubro, Fase 13 |
| [`HANDOFF.md`](./HANDOFF.md) | **Archivado** (promote erp-parity 2026-07-19) - no es entrada viva |
| [`client-android/README.md`](./client-android/README.md) | Cómo correr la app Compose |

## Visión (una línea)

SO del independiente chileno + riel de confianza entre nodos (identidad, discovery,
negociación, liquidación). Detalle: [`docs/strategy/`](./docs/strategy/) - no re-derivar acá.

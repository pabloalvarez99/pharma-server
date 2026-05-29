# pharma-server

[![ci](https://github.com/pabloalvarez99/pharma-server/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/pabloalvarez99/pharma-server/actions/workflows/ci.yml)
[![audit](https://github.com/pabloalvarez99/pharma-server/actions/workflows/audit.yml/badge.svg?branch=main)](https://github.com/pabloalvarez99/pharma-server/actions/workflows/audit.yml)
[![license: Proprietary](https://img.shields.io/badge/license-Proprietary-red.svg)](./Cargo.toml)

On-prem Rust server for pharmacy ERP. SurrealDB embedded (RocksDB), axum HTTP API, Windows service deployment via MSI.

## Status
Scaffold — feature/pharma-server-scaffold.

## Build

```powershell
cargo build --release --workspace
```

## Run (dev)

```powershell
cargo run -p api
curl http://localhost:8080/health/live
```

## Crates

| Crate | Role |
|-------|------|
| core | Domain types, errors, config |
| db | SurrealDB client + repos |
| api | Axum HTTP server (entry: `cargo run -p api`) |
| auth | JWT + argon2id, tenant claims |
| jobs | Cron + NATS workers |
| telemetry | tracing + OTLP + Prometheus |
| service | Windows service entry, embeds api |
| cli | Admin CLI (migrate, seed, user create) |

## Stack

- Rust 1.95 (pin in `rust-toolchain.toml`)
- axum 0.8 + utoipa 5
- SurrealDB 2.1 embedded (kv-surrealkv — pure-Rust LSM, no libclang/bindgen dependency)
- jsonwebtoken 9 + argon2 0.5
- tracing + opentelemetry-otlp + axum-prometheus
- windows-service 0.7
- cargo-wix (MSI installer)

## Run as Windows service (manual smoke)

Requires elevated PowerShell.

```powershell
cargo build --release -p service

sc.exe create PharmaServer `
  binPath= "C:\Users\Administrator\Documents\GitHub\pharma-server\target\release\pharma-service.exe" `
  start= demand
sc.exe start PharmaServer
Get-Service PharmaServer
curl http://127.0.0.1:8080/health/live
sc.exe stop PharmaServer
sc.exe delete PharmaServer
```

Service and CLI/dev binary cannot run simultaneously against the same `./data/surreal` directory (SurrealKv file lock).

## MSI installer

Prereqs: `cargo install cargo-wix` and WiX v3 toolset (`choco install wixtoolset`). Add `C:\Program Files (x86)\WiX Toolset v3.14\bin` to PATH.

```powershell
cargo build --release -p service
cargo wix --package service --no-build --nocapture `
  -C -ext -C WixFirewallExtension `
  -L -ext -L WixFirewallExtension
# → target/wix/pharma-server-<version>-x86_64.msi
```

Install / uninstall (admin):

```powershell
msiexec /i target\wix\pharma-server-0.1.0-x86_64.msi /qn /l*v install.log
Get-Service PharmaServer
Get-NetFirewallRule -DisplayName "Pharma Server API"
msiexec /x target\wix\pharma-server-0.1.0-x86_64.msi /qn
```

The MSI installs to `%ProgramFiles%\PharmaServer\`, registers the `PharmaServer` service (auto-start, LocalSystem), creates `%ProgramData%\PharmaServer\`, and opens inbound TCP 8080 in Windows Firewall.

## Producción

Antes de poner el servidor en producción en una farmacia real (offline-first,
LAN-only), seguir la guía de operaciones:

- [`config/production.toml.example`](config/production.toml.example) — plantilla
  de configuración de producción (todo secreto como placeholder, se inyecta por
  `PHARMA__*`; bind LAN-only, telemetría OFF, backup programado).
- [`docs/ops/production-checklist.md`](docs/ops/production-checklist.md) —
  checklist de go-live: secreto JWT fuerte, token de métricas, firewall a la
  LAN, backup verificado + restauración probada, servicio auto-start, licencia,
  telemetría opt-in, DB sin datos demo.
- [`docs/ops/backup-restore.md`](docs/ops/backup-restore.md) — cómo funciona el
  backup SurrealKv programado/manual y cómo restaurar (copia manual; la
  restauración guiada es roadmap).

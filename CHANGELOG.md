# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned

- Fase 9.1 — DTEs completos (boleta + factura + NC + ND + GD + X/Z fiscales).
- Fase 9.2 — Multi-caja apertura/cierre/arqueo.
- Roadmap completo: [`docs/adr/0010-roadmap-fase-9-parity.md`](./docs/adr/0010-roadmap-fase-9-parity.md).

## [0.1.24] — 2026-05-21

### Added

- **MSI UX launcher** ([`installer/wix/main.wxs`](./installer/wix/main.wxs)): Start Menu shortcut "Pharma Server > Pharma Server Dashboard" + auto-launch del dashboard en browser default post-install (UI / passive modes only; silent `/quiet` no abre browser).
- **Healthcheck retry** ([`installer/wix/launch-wait.ps1`](./installer/wix/launch-wait.ps1)): PowerShell custom action que pollea `http://localhost:8080/` hasta 15s antes de abrir el browser. Evita race condition launch-vs-service-ready.
- **Competitor parity analysis** ([`docs/strategy/competitor-parity-analysis.md`](./docs/strategy/competitor-parity-analysis.md)) + [ADR-0010](./docs/adr/0010-roadmap-fase-9-parity.md) — secuencia Fase 9.x para paridad mínima vendible vs SICO/GOLAN/t-Farmacias/iFarmacias/Bsale.
- **`pharma license activate`** (Fase 11b): CLI fetch + verify + persist desde license-server.
- **Embedded prod pubkey** (Fase 11a) + cross-repo contract test con `pharma-license-server`.

### Changed

- License-server default URL: `pharma-license-server.vercel.app`.
- `crates/service/Cargo.toml`: `metadata.wix.extensions = []` (colisión con `WixUtilExtension` built-in; flags CLI siguen necesarios para `WixFirewallExtension`).

### Build

- MSI artifact: `target/wix/pharma-server-0.1.24-x86_64.msi` (12.36 MB).
- Build command: `cargo wix --package service --no-build --nocapture -C -ext -C WixFirewallExtension -L -ext -L WixFirewallExtension`.

### Known issues

- MSI no firmado: SmartScreen muestra warning. Bypass instructions: [`docs/install/smartscreen-warning.md`](./docs/install/smartscreen-warning.md). Compra Authenticode cert diferida a Fase 9.1.
- Smoke install en Windows Sandbox limpia pendiente — habilitar Sandbox feature requiere reboot. Procedure: [`docs/install/smoke-procedure.md`](./docs/install/smoke-procedure.md).

## [0.1.23] — 2026-05-19

### Added

- MSI release público GitHub Releases: https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.23 (12.30 MB).
- Endpoints ERP completos (inventario, POS, devoluciones, receta, interacciones medicamentosas, caja, gastos, compras, recepción, AP).
- Reportes (sales-daily, margins, top-products, stock-rotation, near-expiry).
- Agent inbox + license admin (reload, status).
- License layer MVP completo (`crates/license` + CLI `pharma license …`).
- POC gated endpoint: `GET /api/v1/reports/margins-daily` → 402 sin Pro tier.

## [0.1.4] — anterior

- Foundation: Windows service, SurrealDB embedded, axum HTTP API, multi-tenant JWT HS256, argon2id.
- Primer MSI verde end-to-end con WiX v3.14 + cargo-wix 0.3.9.

[Unreleased]: https://github.com/pabloalvarez99/pharma-server/compare/v0.1.24...HEAD
[0.1.24]: https://github.com/pabloalvarez99/pharma-server/compare/v0.1.23...v0.1.24
[0.1.23]: https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.23

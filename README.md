# pharma-server

**RutBusiness** on-prem ERP server (Rust). Multi-vertical product for Chilean businesses (1 RUT = 1 business); pharmacy is the beachhead vertical, not the product limit.

Single installable binary: **Axum HTTP API** + **SurrealDB embedded** (`kv-surrealkv`) + **Windows service** (MSI). Offline-first, freemium core.

| | |
|---|---|
| **Version** | `0.1.24` |
| **Repo** | [pabloalvarez99/pharma-server](https://github.com/pabloalvarez99/pharma-server) |
| **Latest MSI** | [v0.1.23](https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.23) |
| **Rust** | pinned in `rust-toolchain.toml` |

---

## Repository layout

```
pharma-server/
├── crates/           # Workspace members (see table below)
├── config/           # Default TOML config
├── migrations/       # SurrealQL schema migrations
├── docs/
│   ├── adr/          # Architecture Decision Records
│   ├── product/      # Product / parity notes
│   └── strategy/     # Roadmaps, freemium, license, payments
├── installer/wix/    # MSI (cargo-wix)
├── data/             # Runtime DB (gitignored)
├── local/            # Machine-only notes & secrets (gitignored)
├── Cargo.toml        # Workspace root
├── bitacora.md       # Chronological technical log
└── CLAUDE.md         # Agent / contributor project context
```

---

## Crates

| Crate | Role |
|-------|------|
| `core` | Config, errors, tenant |
| `db` | SurrealDB client + embedded migrations |
| `domain` | Business modules (catalog, inventory, sales, …) |
| `api` | Axum HTTP server (`cargo run -p api`) |
| `auth` | JWT + argon2id |
| `license` | Ed25519 offline entitlement gate |
| `agent` | Agent identity / signed envelopes |
| `dte` | Chile SII DTE (boleta XML, TED, CAF) |
| `jobs` | Cron + NATS workers |
| `telemetry` | tracing + OTLP + Prometheus |
| `service` | Windows service entry (embeds API) |
| `cli` | Admin CLI (migrate, seed, license, users) |

---

## Quick start

### Prerequisites

- Rust toolchain matching `rust-toolchain.toml`
- Windows (service / MSI) or any OS for API-only dev

### Build

```powershell
cargo build --release --workspace
```

### Run (dev)

```powershell
cargo run -p api
# health
curl http://localhost:8080/health/live
```

Swagger UI is served by the API when enabled in config (see `config/default.toml`).

### Tests

```powershell
cargo test --workspace
```

> **Disk note:** debug builds with SurrealDB are large. Prefer `cargo test -p <crate>` when iterating. `target/` is gitignored and safe to delete anytime (`cargo clean` or remove the folder).

---

## Windows service (manual smoke)

Elevated PowerShell:

```powershell
cargo build --release -p service

sc.exe create PharmaServer `
  binPath= "D:\path\to\pharma-server\target\release\pharma-service.exe" `
  start= demand
sc.exe start PharmaServer
curl http://127.0.0.1:8080/health/live
sc.exe stop PharmaServer
sc.exe delete PharmaServer
```

Service and CLI/dev binary cannot share the same `./data/surreal` directory (SurrealKv file lock).

---

## MSI installer

Prereqs: `cargo install cargo-wix`, WiX Toolset v3 on PATH.

```powershell
cargo build --release -p service
cargo wix --package service --no-build --nocapture `
  -C -ext -C WixFirewallExtension `
  -L -ext -L WixFirewallExtension
# → target/wix/pharma-server-<version>-x86_64.msi
```

Install / uninstall (admin):

```powershell
msiexec /i target\wix\pharma-server-0.1.24-x86_64.msi /qn /l*v install.log
msiexec /x target\wix\pharma-server-0.1.24-x86_64.msi /qn
```

Installs to `%ProgramFiles%\PharmaServer\`, registers service `PharmaServer`, data under `%ProgramData%\PharmaServer\`, firewall TCP 8080.

---

## Stack

- **axum** 0.8 + **utoipa** 5 (OpenAPI)
- **SurrealDB** 2.1 embedded (`kv-surrealkv`)
- **jsonwebtoken** 9 + **argon2** 0.5
- **ed25519-dalek** (license + agent envelopes)
- **tracing** + OpenTelemetry OTLP + Prometheus
- **windows-service** 0.7 + **cargo-wix**

---

## Documentation

| Path | Contents |
|------|----------|
| [`docs/README.md`](./docs/README.md) | Docs index |
| [`docs/adr/`](./docs/adr/) | ADRs (freemium, license, DTE, …) |
| [`docs/strategy/`](./docs/strategy/) | Product & commercial strategy |
| [`bitacora.md`](./bitacora.md) | Session / technical changelog |
| [`CLAUDE.md`](./CLAUDE.md) | Full project context for contributors/agents |

---

## Local-only files

Anything under `local/` stays on this machine (notes, test credentials, scratch). Never commit secrets. See `local/README.md`.

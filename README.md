# pharma-server

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

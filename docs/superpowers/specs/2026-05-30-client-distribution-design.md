# Pharma Client — distribution design (2026-05-30)

Status: **approved** (founder forks resolved via AskUserQuestion, 2026-05-30).

## Problem

The Tauri desktop client (`client/`) reached ERP parity (all views operable) but has no
distributable, signed installer and no defined distribution channel. The client is a
**separate product** from the server MSI: N clients (one per cashier PC) talk to 1 server
over the LAN.

## Decisions

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| a | Bundle vs separate installer | **Separate client installer** | N clients ↔ 1 server topology; two separately-versioned products. Bundling into the server MSI would couple them and break multi-client deploy. |
| b | Signing | **Reuse `installer/sign/pilot.pfx`** (self-signed, →2029) | Generic code-signing cert signs any Authenticode artifact (MSI + NSIS). $0 pre-revenue, same client-side trust-import flow as the server (ADR-0008). One signer, two products. |
| c | Channel | **Reuse `pharma-server-releases` mirror**, tag `client-v<ver>` | One download hub for pilots (server + client from one org). `client-` tag prefix separates the two product feeds. YAGNI on a second mirror repo + second PAT pre-revenue. |
| d | Server URL discovery | **Existing login form + localStorage** (+ "Probar conexión" button) | Already implemented: persisted > `VITE_SERVER_URL` build-time > loopback fallback; advanced panel opens on fresh install. Added a no-auth health-ping button to confirm reachability before login. |
| — | Installer format | **Both MSI + NSIS** | MSI for GPO/SCCM enterprise deploy; NSIS `-setup.exe` for per-user installs. Founder chose max pilot flexibility. |

## Components

1. **`installer/sign/sign-msi.ps1`** — generalized with a `-Description` param (default
   `"Pharma Server"`) so the client reuses it with `"Pharma Client"`. Already signs any
   Authenticode artifact (.msi/.exe).
2. **`installer/client/build-client.ps1`** — `npm ci` → `npx tauri build` (release; MSI +
   NSIS), resolve the bundle dir (root `target/` per `.cargo/config.toml`, in-crate
   fallback), optional `-Sign`, optional `-ServerUrl` to bake `VITE_SERVER_URL`.
3. **`installer/client/sign-client.ps1`** — signs MSI + NSIS via `sign-msi.ps1
   -Description "Pharma Client"`.
4. **`.github/workflows/client-release-publisher.yml`** — mirrors `release-publisher.yml`:
   fail-closed on missing secrets → rust 1.95 + node 20 → `npm ci` + `tauri build` → sign
   both → publish to `pharma-server-releases` tag `client-v<ver>` with MSI + NSIS +
   `pilot.cer`. `workflow_dispatch`, founder-gated.
5. **`client/src/views/login.ts`** — "Probar conexión" button → `serverHealth(url)` →
   reachability status (no auth). CSS `.conn-test` in `styles.css`.
6. **`installer/client/README.md`** — build/sign/distribute runbook + onboarding.

## Bundle path gotcha

The repo-root `.cargo/config.toml` sets `target-dir = "target"` (and `+crt-static`). Cargo
config discovery applies it to the client build too, so `tauri build` writes to the
**repo-root `target/release/bundle/`**, NOT `client/src-tauri/target/...`. Both the build
script and the workflow resolve the bundle dir at runtime (root first, in-crate fallback).

## Out of scope (YAGNI)

- mDNS/zeroconf server auto-discovery — login form + persisted URL suffices for LAN.
- Second mirror repo for the client — revisit if the product feeds need clean separation
  post-revenue.
- Real Authenticode/EV cert — shares the server's upgrade path (ADR-0008 §staging).
- Auto-update — Tauri updater deferred; pilots reinstall from the mirror.

## DoD

Reproducible build script + signed installer artifacts (MSI + NSIS) + documented
distribution + dual bitácora. **A public release is founder-gated** — the pipeline is
scaffolded and the local build verified, but `client-release-publisher.yml` is not
dispatched without explicit go.

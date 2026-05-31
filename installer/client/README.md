# installer/client — Pharma Client (Tauri desktop) build + distribution

Reproducible build, signing, and distribution for the **client** — the desktop POS/ERP
app that talks to `pharma-server` over the LAN. This is a **separate product** from the
server MSI: N clients (one per cashier PC) point at 1 server.

**Decisions**: distribution design → [`docs/superpowers/specs/2026-05-30-client-distribution-design.md`](../../docs/superpowers/specs/2026-05-30-client-distribution-design.md).
Signing reuses the pilot cert → [ADR-0008](../../docs/adr/0008-self-sign-pilot-msi.md).

## What gets built

`tauri build` (release) emits **both** Windows bundle targets. The repo's root
`.cargo/config.toml` sets `target-dir = "target"`, so the client bundle lands in the
**repo-root `target/`** (not `client/src-tauri/target/`):

| Format | Path | Use |
|---|---|---|
| MSI (WiX)  | `target/release/bundle/msi/Pharma Client_<ver>_x64_en-US.msi` | GPO / SCCM mass-deploy, enterprise |
| NSIS (.exe)| `target/release/bundle/nsis/Pharma Client_<ver>_x64-setup.exe` | per-user, consumer-friendly |

Both are signed with the **same** `installer/sign/pilot.pfx` as the server (one signer,
two products), SHA256 + RFC3161 timestamp, cert valid through 2029. The same config also
links `+crt-static`, so the client carries no external VCRUNTIME dependency.

> `client/src-tauri` is **excluded from the cargo workspace** (own `[workspace]`) → the
> workspace CI never compiles it. This script + `client-release-publisher.yml` are the only
> things that build the client. `build-client.ps1` resolves the bundle dir at runtime
> (root `target/` first, in-crate `client/src-tauri/target/` fallback) so a
> `CARGO_TARGET_DIR` override still works.

## Build locally

```powershell
# Plain build (no signing):
pwsh installer/client/build-client.ps1

# Build + sign both artifacts (needs the pilot cert + password):
$env:PHARMA_CERT_PASSWORD = "<strong-password>"
pwsh installer/client/build-client.ps1 -Sign

# Field install pre-pointed at a pharmacy's LAN server:
pwsh installer/client/build-client.ps1 -ServerUrl "http://192.168.1.50:8080" -Sign
```

`build-client.ps1` runs `npm ci` → `npx tauri build` (the frontend GATE,
`tsc --noEmit && vite build`, runs via Tauri's `beforeBuildCommand`), locates both
artifacts, and optionally signs them via `sign-client.ps1`.

## Sign only (artifacts already built)

```powershell
$env:PHARMA_CERT_PASSWORD = "<strong-password>"
pwsh installer/client/sign-client.ps1 `
  -MsiPath  "target/release/bundle/msi/Pharma Client_0.1.0_x64_en-US.msi" `
  -NsisPath "target/release/bundle/nsis/Pharma Client_0.1.0_x64-setup.exe"
```

`sign-client.ps1` is a thin wrapper over `../sign/sign-msi.ps1 -Description "Pharma Client"`
(that script signs any Authenticode artifact, `.msi` and `.exe` alike).

## Distribution

Published to the **same mirror as the server** — `pabloalvarez99/pharma-server-releases`
— under a `client-v<ver>` tag (the prefix keeps the two product feeds apart while pilots
download both from one place). Assets per release: the signed MSI, the signed NSIS
`-setup.exe`, and the public `pilot.cer`.

Trigger (founder-gated — a public release needs explicit go):

```bash
gh workflow run client-release-publisher.yml -f version=0.1.0
```

The workflow **fails closed** if the signing secrets (`PILOT_PFX_B64`,
`PHARMA_CERT_PASSWORD`, `MIRROR_RELEASE_TOKEN`) are absent — no unsigned installer ever
reaches clients.

## Server URL discovery (first launch)

The client does **not** bundle the server. It discovers the server URL via the login
screen (`client/src/views/login.ts`):

1. **Persisted** — last server that logged in successfully (`localStorage`), pre-filled
   on next launch.
2. **Build-time** — `VITE_SERVER_URL` baked via `-ServerUrl` above, for field installs.
3. **Loopback fallback** — `http://127.0.0.1:8080` (server on the same machine).

On a fresh install with no persisted/baked value, the **"Conexión avanzada"** panel opens
by default so a LAN client on a different machine isn't silently pointed at its own
localhost. A **"Probar conexión"** button pings the server health endpoint (no auth) so the
operator can confirm the URL before typing credentials.

## Pilot client onboarding (15-min, one-time)

Same as the server (shared cert): import `pilot.cer` to Trusted Publishers once to skip the
SmartScreen prompt. See [`../sign/README.md`](../sign/README.md). Without the import, the
installer still runs via "More info → Run anyway".

## Upgrade path

Shares the server's cert upgrade path (MSIX / Azure Trusted Signing / EV) —
see [`../sign/README.md`](../sign/README.md) §upgrade and [ADR-0008](../../docs/adr/0008-self-sign-pilot-msi.md).

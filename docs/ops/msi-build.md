# MSI build & smoke-test (ops runbook)

How to build, smoke-test, and (eventually) sign the Pharma Server MSI.

- Toolchain: `cargo-wix 0.3.9` + WiX v3.14 toolset (`candle.exe` / `light.exe`).
- Authoring: [`installer/wix/main.wxs`](../../installer/wix/main.wxs).
- WiX metadata: `[package.metadata.wix]` in [`crates/service/Cargo.toml`](../../crates/service/Cargo.toml)
  (`include` points at `main.wxs`, `extensions = ["WixFirewallExtension"]`).
- Target: `x86_64-pc-windows-msvc`. Build on Windows.

## What the MSI installs

- `pharma-service.exe` → `%ProgramFiles%\PharmaServer\`.
- Windows service **`PharmaServer`** — `Type=ownProcess`, `Start=auto`,
  `Account=LocalSystem`. Started on install, stopped + removed on uninstall
  (`ServiceControl Start=install Stop=both Remove=uninstall`).
- Data dir `%ProgramData%\PharmaServer\` — marked `Permanent="yes"`, so it
  (and the customer's SurrealDB) **survives uninstall and upgrade**. No
  `RemoveFolder`: never wipe customer data.
- Inbound Windows Firewall rule on **TCP 8080** (`WixFirewallExtension`).
- `MajorUpgrade` — clean in-place upgrade (newer version replaces older;
  downgrade is blocked with an error message).

## Prerequisites

```powershell
cargo install cargo-wix          # 0.3.9
choco install wixtoolset         # WiX v3.14
# Put the WiX toolset on PATH for this session (or add permanently):
$env:Path += ";C:\Program Files (x86)\WiX Toolset v3.14\bin"
```

## Build the MSI

`cargo wix` reads `[package.metadata.wix]` from the `service` crate, so run it
against that package. Build the release binary first; `--no-build` then tells
cargo-wix to reuse it instead of rebuilding.

```powershell
cargo build --release -p service

cargo wix --package service --no-build --nocapture `
  -C -ext -C WixFirewallExtension `
  -L -ext -L WixFirewallExtension
```

- `-C -ext ... WixFirewallExtension` → pass the extension to **candle** (compile).
- `-L -ext ... WixFirewallExtension` → pass it to **light** (link).
- Output lands at: `target\wix\pharma-server-<version>-x86_64.msi`
  (version comes from `workspace.package.version`).

### Validate the .wxs without a full build (dry-run)

Compiling the authoring with **candle** alone catches WiX-level errors without
linking the real binary (light needs the actual `pharma-service.exe`):

```powershell
$env:Path += ";C:\Program Files (x86)\WiX Toolset v3.14\bin"
$ext = "C:\Program Files (x86)\WiX Toolset v3.14\bin\WixFirewallExtension.dll"
candle.exe -nologo -arch x64 `
  "-dVersion=0.1.24" `
  "-dCargoTargetBinDir=$env:TEMP" `
  -ext "$ext" `
  -out "$env:TEMP\main.wixobj" `
  installer\wix\main.wxs
# Exit 0 + a main.wixobj => authoring is valid.
```

cargo-wix injects `Version` and `CargoTargetBinDir`; supply them by hand when
calling candle directly. (PowerShell tip: if `-dVersion=0.1.24` gets mangled,
pass the args via an array — `& candle.exe @args` — and quote the defines.)

## Smoke-test on a clean Windows VM

Use a fresh VM (no prior PharmaServer install). Run an **elevated** PowerShell;
service install requires admin. The `service` binary and the dev/CLI binary must
not run against the same `./data/surreal` at once (SurrealKv file lock) — a clean
VM avoids this.

```powershell
$msi = "target\wix\pharma-server-0.1.24-x86_64.msi"   # match actual version

# 1. Install (silent, verbose log)
msiexec /i $msi /qn /l*v install.log

# 2. Verify the service is registered and running
sc.exe query PharmaServer            # STATE should be 4 RUNNING
Get-Service PharmaServer             # Status: Running, StartType: Automatic

# 3. Verify the firewall rule
Get-NetFirewallRule -DisplayName "Pharma Server API"

# 4. Verify the API answers
curl http://127.0.0.1:8080/health/live

# 5. Service lifecycle (optional manual check)
sc.exe stop PharmaServer
sc.exe query PharmaServer            # STATE 1 STOPPED
sc.exe start PharmaServer
sc.exe query PharmaServer            # STATE 4 RUNNING

# 6. Uninstall (silent) — stops + removes the service
msiexec /x $msi /qn /l*v uninstall.log
sc.exe query PharmaServer            # => 1060: service does not exist
Test-Path "$env:ProgramData\PharmaServer"   # => True (data dir survives)
```

Pass criteria: install registers + starts `PharmaServer`, firewall rule exists,
`/health/live` responds, uninstall removes the service, **and the ProgramData
data dir is left intact**.

### Upgrade smoke (MajorUpgrade)

Install an older MSI, then install a newer one over it (no manual uninstall):

```powershell
msiexec /i pharma-server-0.1.23-x86_64.msi /qn
msiexec /i pharma-server-0.1.24-x86_64.msi /qn /l*v upgrade.log
sc.exe query PharmaServer            # still RUNNING, single instance
```

## Authenticode signing — PENDING CERT (Fase 9 blocker)

An unsigned MSI triggers a SmartScreen / UAC "unknown publisher" warning.
Signing is **blocked**: no code-signing certificate is provisioned yet (Fase 9
in the roadmap). When a cert exists (OV/EV `.pfx` or HSM/token), sign the built
MSI before distribution with `signtool.exe` (Windows SDK,
`C:\Program Files (x86)\Windows Kits\10\bin\<ver>\x64\signtool.exe`):

```powershell
# === PENDING CERT — DO NOT RUN until a code-signing cert is provisioned ===
$signtool = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe"
$msi = "target\wix\pharma-server-0.1.24-x86_64.msi"

& $signtool sign `
  /f   "C:\path\to\pharma-codesign.pfx" `        # or /sha1 <thumbprint> for a token/HSM cert
  /p   "$env:PHARMA_CODESIGN_PFX_PASSWORD" `      # never hardcode the password
  /fd  SHA256 `
  /tr  http://timestamp.digicert.com `            # RFC-3161 timestamp (use your CA's TSA)
  /td  SHA256 `
  /d   "Pharma Server" `
  $msi

# Verify the signature
& $signtool verify /pa /v $msi
```

Notes when the cert lands:
- Prefer an **EV** cert (instant SmartScreen reputation) over OV.
- Always timestamp (`/tr` + `/td`) so the signature stays valid after the cert
  expires.
- Keep the cert/token out of the repo and CI logs; inject via secret env var.
- Sign `pharma-service.exe` as well as the MSI for a fully trusted chain.

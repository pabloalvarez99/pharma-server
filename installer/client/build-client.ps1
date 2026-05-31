#Requires -Version 5.1
<#
.SYNOPSIS
    Build the Pharma Client (Tauri 2 desktop) into a Windows MSI + NSIS installer.

.DESCRIPTION
    Reproducible local build of the SEPARATE client product (not the server MSI).
    Runs the frontend GATE (tsc --noEmit + vite via `npm run build`, invoked by
    Tauri's beforeBuildCommand) then `tauri build`, which emits both bundle targets:
      - MSI  (WiX)  -> <bundle>/msi/Pharma Client_<ver>_x64_en-US.msi
      - NSIS (.exe) -> <bundle>/nsis/Pharma Client_<ver>_x64-setup.exe

    BUNDLE PATH: the repo's root .cargo/config.toml sets `target-dir = "target"`, so
    cargo config discovery sends the client build to the REPO-ROOT target/ (not
    client/src-tauri/target). This script resolves the bundle dir at runtime and also
    falls back to client/src-tauri/target in case CARGO_TARGET_DIR is overridden.

    The client is distributed via the SAME mirror as the server
    (pabloalvarez99/pharma-server-releases) under tag `client-v<ver>`, signed with
    the pilot self-signed cert (ADR-0008). See installer/client/README.md.

    `client/src-tauri` is EXCLUDED from the cargo workspace (own [workspace]), so
    workspace CI never builds it — this script +
    .github/workflows/client-release-publisher.yml are the only things that compile it.

.PARAMETER Sign
    After building, sign both artifacts with the pilot cert (calls sign-client.ps1).
    Requires PHARMA_CERT_PASSWORD env var and installer/sign/pilot.pfx present.

.PARAMETER ServerUrl
    Optional. Bakes VITE_SERVER_URL into the build so a field install pre-points at
    the pharmacy's LAN server IP. Omit for the default (loopback + login form).
#>
[CmdletBinding()]
param(
    [switch]$Sign,
    [string]$ServerUrl
)

$ErrorActionPreference = "Stop"
$repoRoot   = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$clientDir  = Join-Path $repoRoot "client"

if (-not (Test-Path (Join-Path $clientDir "package.json"))) {
    Write-Error "client/package.json not found at $clientDir"
    exit 1
}

Push-Location $clientDir
try {
    if ($ServerUrl) {
        Write-Host "Baking VITE_SERVER_URL=$ServerUrl into the build."
        $env:VITE_SERVER_URL = $ServerUrl
    }

    Write-Host "==> npm ci"
    npm ci
    if ($LASTEXITCODE -ne 0) { Write-Error "npm ci failed"; exit 1 }

    Write-Host "==> tauri build (release; MSI + NSIS)"
    npx tauri build
    if ($LASTEXITCODE -ne 0) { Write-Error "tauri build failed"; exit 1 }
}
finally {
    Pop-Location
}

# Resolve the bundle dir: root target/ (default per .cargo/config.toml) first, then
# the in-crate target as a fallback (covers a CARGO_TARGET_DIR override).
$candidates = @(
    (Join-Path $repoRoot "target\release\bundle"),
    (Join-Path $clientDir "src-tauri\target\release\bundle")
)
$bundleDir = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $bundleDir) {
    Write-Error "No bundle dir found. Looked in: $($candidates -join '; ')"
    exit 1
}

$msi  = Get-ChildItem -Path (Join-Path $bundleDir "msi")  -Filter *.msi -ErrorAction SilentlyContinue | Select-Object -First 1
$nsis = Get-ChildItem -Path (Join-Path $bundleDir "nsis") -Filter *.exe -ErrorAction SilentlyContinue | Select-Object -First 1

if (-not $msi)  { Write-Error "MSI not produced under $bundleDir\msi" ; exit 1 }
if (-not $nsis) { Write-Error "NSIS .exe not produced under $bundleDir\nsis" ; exit 1 }

Write-Host ""
Write-Host "Built (bundle dir: $bundleDir):"
Write-Host "  MSI : $($msi.FullName)  ($([math]::Round($msi.Length/1MB,2)) MB)"
Write-Host "  NSIS: $($nsis.FullName)  ($([math]::Round($nsis.Length/1MB,2)) MB)"

if ($Sign) {
    Write-Host ""
    Write-Host "==> signing both artifacts (pilot cert)"
    & (Join-Path $PSScriptRoot "sign-client.ps1") -MsiPath $msi.FullName -NsisPath $nsis.FullName
    if ($LASTEXITCODE -ne 0) { Write-Error "signing failed"; exit 1 }
}

Write-Host ""
Write-Host "DONE."

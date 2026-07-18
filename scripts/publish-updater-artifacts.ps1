#Requires -Version 5.1
<#
.SYNOPSIS
    Stage Tauri updater artifacts for the RutBusiness CDN layout (local only).

.DESCRIPTION
    Collects signed updater outputs from a prior `tauri build` (with
    createUpdaterArtifacts + TAURI_SIGNING_PRIVATE_KEY) into dist-updater/,
    writes a latest.json template, and prints the suggested CDN paths.

    NEVER uploads to cdn.pharma-server.cl. NEVER reads/writes the private key
    into the staging tree. See docs/ops/cdn-updater.md.

.PARAMETER Version
    Override product version (default: read client/src-tauri/tauri.conf.json).

.PARAMETER Notes
    Release notes string for latest.json.

.PARAMETER BundleDir
    Override path to tauri bundle dir (default: auto-detect root/in-crate target).
#>
[CmdletBinding()]
param(
    [string]$Version = "",
    [string]$Notes = "RutBusiness client update",
    [string]$BundleDir = ""
)

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$clientDir = Join-Path $repoRoot "client"
$tauriConf = Join-Path $clientDir "src-tauri\tauri.conf.json"
$stageRoot = Join-Path $repoRoot "dist-updater"
$cdnBase = "https://cdn.pharma-server.cl/updates/rutbusiness"
$platform = "windows-x86_64"

if (-not (Test-Path $tauriConf)) {
    Write-Error "tauri.conf.json not found: $tauriConf"
    exit 1
}

if (-not $Version) {
    $conf = Get-Content -Raw $tauriConf | ConvertFrom-Json
    $Version = [string]$conf.version
}
if (-not $Version) {
    Write-Error "Could not resolve version (pass -Version)."
    exit 1
}

if (-not $BundleDir) {
    $candidates = @(
        (Join-Path $repoRoot "target\release\bundle"),
        (Join-Path $clientDir "src-tauri\target\release\bundle")
    )
    $BundleDir = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $BundleDir) {
    Write-Error @"
No bundle dir found. Build first with signing env:

  `$env:RUSTC_WRAPPER = ''
  `$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw client\keys\rutbusiness-updater.key
  `$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''
  pwsh installer\client\build-client.ps1

See docs/ops/cdn-updater.md
"@
    exit 1
}

Write-Host "==> Bundle: $BundleDir"
Write-Host "==> Version: $Version"
Write-Host "==> Stage:   $stageRoot"

# Prefer NSIS zip (typical Tauri Windows updater payload), then MSI zip, then raw installers + .sig.
$searchRoots = @(
    (Join-Path $BundleDir "nsis"),
    (Join-Path $BundleDir "msi"),
    $BundleDir
) | Where-Object { Test-Path $_ }

$payload = $null
foreach ($root in $searchRoots) {
    $payload = Get-ChildItem -Path $root -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object {
            $_.Name -match '\.nsis\.zip$' -or
            $_.Name -match '\.msi\.zip$' -or
            ($_.Extension -eq '.zip' -and $_.Name -match 'RutBusiness|setup')
        } |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if ($payload) { break }
}

if (-not $payload) {
    # Fallback: newest setup.exe or .msi that has a sibling .sig
    foreach ($root in $searchRoots) {
        $candidates = Get-ChildItem -Path $root -Recurse -File -ErrorAction SilentlyContinue |
            Where-Object { $_.Extension -in '.exe', '.msi' } |
            Sort-Object LastWriteTime -Descending
        foreach ($c in $candidates) {
            $sig = "$($c.FullName).sig"
            if (-not (Test-Path $sig)) {
                $sig = Join-Path $c.DirectoryName ($c.BaseName + ".sig")
            }
            if (Test-Path $sig) {
                $payload = $c
                break
            }
        }
        if ($payload) { break }
    }
}

if (-not $payload) {
    Write-Error @"
No updater payload (.nsis.zip / .msi.zip / installer+.sig) under $BundleDir.
Confirm createUpdaterArtifacts=true and that the build ran with
TAURI_SIGNING_PRIVATE_KEY (+ empty TAURI_SIGNING_PRIVATE_KEY_PASSWORD).
"@
    exit 1
}

$sigPath = "$($payload.FullName).sig"
if (-not (Test-Path $sigPath)) {
    $alt = Join-Path $payload.DirectoryName ($payload.Name + ".sig")
    if (Test-Path $alt) { $sigPath = $alt }
}
if (-not (Test-Path $sigPath)) {
    Write-Error "Missing signature file for $($payload.Name). Expected: $sigPath"
    exit 1
}

$platformDir = Join-Path $stageRoot $platform
$verDir = Join-Path $platformDir $Version
New-Item -ItemType Directory -Force -Path $verDir | Out-Null

$destPayload = Join-Path $verDir $payload.Name
$destSig = Join-Path $verDir (Split-Path $sigPath -Leaf)
Copy-Item -Force $payload.FullName $destPayload
Copy-Item -Force $sigPath $destSig

$signature = (Get-Content -Raw $destSig).Trim()
$cdnUrl = "$cdnBase/$platform/$Version/$($payload.Name)"
$pubDate = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

$manifest = [ordered]@{
    version  = $Version
    notes    = $Notes
    pub_date = $pubDate
    platforms = [ordered]@{
        $platform = [ordered]@{
            signature = $signature
            url       = $cdnUrl
        }
    }
}

$json = $manifest | ConvertTo-Json -Depth 6
$latestPath = Join-Path $platformDir "latest.json"
# Also stage a copy named as the *previous* install targets would fetch —
# operator still must copy this JSON to each old version path on the CDN.
Set-Content -Path $latestPath -Value $json -Encoding utf8
$versionedManifest = Join-Path $verDir "latest.json"
Set-Content -Path $versionedManifest -Value $json -Encoding utf8

Write-Host ""
Write-Host "Staged (local only — nothing uploaded):"
Write-Host "  $destPayload"
Write-Host "  $destSig"
Write-Host "  $latestPath"
Write-Host ""
Write-Host "CDN layout to publish manually:"
Write-Host "  $cdnBase/$platform/$Version/$($payload.Name)"
Write-Host "  $cdnBase/$platform/$Version/$(Split-Path $sigPath -Leaf)"
Write-Host "  For each installed version that should upgrade, PUT the same JSON at:"
Write-Host "  $cdnBase/$platform/{current_version}"
Write-Host "  (endpoint template uses {{current_version}} — see docs/ops/cdn-updater.md)"
Write-Host ""
Write-Host "Example (do NOT run from CI without human review):"
Write-Host "  # aws s3 sync dist-updater/ s3://YOUR_BUCKET/updates/rutbusiness/ --dryrun"
Write-Host ""
Write-Host "latest.json preview:"
Write-Host $json

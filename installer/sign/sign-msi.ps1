#Requires -Version 5.1
<#
.SYNOPSIS
    Sign a pharma-server MSI with the pilot self-signed certificate + RFC3161 timestamp.

.DESCRIPTION
    Wraps signtool.exe. Reads the pfx password from env var PHARMA_CERT_PASSWORD.
    Always applies an RFC3161 timestamp so the signature stays valid after the cert
    expires (critical: without it, clients can't reinstall after 3 years).

    Decision context: ADR-0008 (self-sign pilot MSI).

.PARAMETER MsiPath
    Path to the .msi to sign (required).

.PARAMETER PfxPath
    Path to pilot.pfx. Default: alongside this script.

.PARAMETER TimestampUrl
    RFC3161 timestamp server. Default DigiCert (free, no account).

.NOTES
    signtool.exe ships with the Windows SDK. If not on PATH, the script probes common
    SDK locations under "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\".
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$MsiPath,
    [string]$PfxPath = (Join-Path $PSScriptRoot "pilot.pfx"),
    [string]$TimestampUrl = "http://timestamp.digicert.com"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $MsiPath)) {
    Write-Error "MSI not found: $MsiPath"
    exit 1
}
if (-not (Test-Path $PfxPath)) {
    Write-Error "pilot.pfx not found at $PfxPath. Run generate-pilot-cert.ps1 first."
    exit 1
}
if (-not $env:PHARMA_CERT_PASSWORD) {
    Write-Error "PHARMA_CERT_PASSWORD env var is not set."
    exit 1
}

function Find-SignTool {
    $cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }

    $kits = "C:\Program Files (x86)\Windows Kits\10\bin"
    if (Test-Path $kits) {
        $candidate = Get-ChildItem -Path $kits -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -like "*\x64\*" } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($candidate) { return $candidate.FullName }
    }
    return $null
}

$signtool = Find-SignTool
if (-not $signtool) {
    Write-Error "signtool.exe not found. Install the Windows SDK (winget install Microsoft.WindowsSDK or via Visual Studio Installer)."
    exit 1
}

Write-Host "Using signtool: $signtool"
Write-Host "Signing: $MsiPath"
Write-Host "Timestamp: $TimestampUrl"

& $signtool sign `
    /f $PfxPath `
    /p $env:PHARMA_CERT_PASSWORD `
    /fd sha256 `
    /tr $TimestampUrl `
    /td sha256 `
    /d "Pharma Server" `
    $MsiPath

if ($LASTEXITCODE -ne 0) {
    Write-Error "signtool sign failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

Write-Host ""
Write-Host "Signed OK. Verifying..."

# /pa = use the Default Authenticode policy. Self-signed certs won't chain to a trusted
# root, so verify reports the signature is present + timestamped, but trust will only
# resolve on machines that imported pilot.cer to Trusted Publishers (expected for pilot).
& $signtool verify /pa /v $MsiPath
$verifyExit = $LASTEXITCODE

if ($verifyExit -ne 0) {
    Write-Warning "signtool verify returned $verifyExit. For a SELF-SIGNED cert this is EXPECTED on a machine that has not imported pilot.cer to Trusted Publishers."
    Write-Warning "The signature + timestamp are still embedded. Verify on a machine with pilot.cer imported to confirm full trust."
}

Write-Host ""
Write-Host "DONE. The MSI is signed with the pilot cert."
Write-Host "Pilot clients must import pilot.cer to Trusted Publishers to avoid SmartScreen."

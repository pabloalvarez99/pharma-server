#Requires -Version 5.1
<#
.SYNOPSIS
    Sign the Pharma Client MSI + NSIS installer with the pilot cert + RFC3161 timestamp.

.DESCRIPTION
    Thin wrapper over installer/sign/sign-msi.ps1 (which works on any Authenticode-
    signable file, .msi and .exe alike) passing -Description "Pharma Client" so the
    signature names the right product. Reuses the SAME pilot.pfx as the server
    (ADR-0008) — one signer, two products. Reads the pfx password from
    PHARMA_CERT_PASSWORD.

.PARAMETER MsiPath
    Path to the client .msi (required).

.PARAMETER NsisPath
    Path to the client NSIS -setup.exe (required).
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$MsiPath,
    [Parameter(Mandatory = $true)][string]$NsisPath
)

$ErrorActionPreference = "Stop"
$signMsi = Resolve-Path (Join-Path $PSScriptRoot "..\sign\sign-msi.ps1")

foreach ($artifact in @($MsiPath, $NsisPath)) {
    if (-not (Test-Path $artifact)) { Write-Error "Artifact not found: $artifact"; exit 1 }
    Write-Host "==> signing $artifact"
    & $signMsi -MsiPath $artifact -Description "Pharma Client"
    if ($LASTEXITCODE -ne 0) { Write-Error "signing failed for $artifact"; exit 1 }
}

Write-Host ""
Write-Host "Both client artifacts signed (Pharma Client, pilot cert + timestamp)."

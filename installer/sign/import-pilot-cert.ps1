#Requires -Version 5.1
<#
.SYNOPSIS
    (CLIENT-SIDE) Import the pilot public certificate to Trusted Publishers + Trusted Root.

.DESCRIPTION
    Run on a PILOT CLIENT machine BEFORE installing a pilot-signed MSI. Imports pilot.cer
    so Windows trusts the pilot self-signed signature and skips the SmartScreen warning.

    Requires admin elevation (writes to LocalMachine cert store).

    Decision context: ADR-0008. This is the 15-min onboarding step for pilot clients.

.PARAMETER CerPath
    Path to pilot.cer. Default: alongside this script.
#>
[CmdletBinding()]
param(
    [string]$CerPath = (Join-Path $PSScriptRoot "pilot.cer")
)

$ErrorActionPreference = "Stop"

# Elevation check.
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Error "This script must run elevated (Run as Administrator). It writes to the LocalMachine certificate store."
    exit 1
}

if (-not (Test-Path $CerPath)) {
    Write-Error "pilot.cer not found at $CerPath. Download it from the release mirror or ask the pilot contact."
    exit 1
}

Write-Host "Importing pilot certificate from: $CerPath"

# Trusted Publishers — lets the signed MSI install without SmartScreen "unknown publisher".
Import-Certificate -FilePath $CerPath -CertStoreLocation "Cert:\LocalMachine\TrustedPublisher" | Out-Null
Write-Host "Imported to LocalMachine\TrustedPublisher."

# Trusted Root — lets the self-signed chain validate (so signtool verify reports Valid).
Import-Certificate -FilePath $CerPath -CertStoreLocation "Cert:\LocalMachine\Root" | Out-Null
Write-Host "Imported to LocalMachine\Root."

Write-Host ""
Write-Host "DONE. This machine now trusts pilot-signed MSIs. You can install pharma-server without the SmartScreen warning."
Write-Host "To remove later: Remove the 'Pharma Server Pilot' cert from certlm.msc (Trusted Publishers + Trusted Root)."

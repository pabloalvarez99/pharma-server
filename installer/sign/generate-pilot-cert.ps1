#Requires -Version 5.1
<#
.SYNOPSIS
    Generate a self-signed code-signing certificate for the pilot phase.

.DESCRIPTION
    Creates a self-signed Authenticode certificate (RSA 2048, 3-year validity) and
    exports it as:
      - pilot.pfx  (PRIVATE — gitignored, used to sign MSIs; protected by password)
      - pilot.cer  (PUBLIC  — distributable; pilot clients import to Trusted Publishers)

    Decision context: ADR-0008 (self-sign pilot MSI). Zero-cost path until first sale.
    Upgrade to MSIX MS Store ($19) or Azure Trusted Signing ($10/mo) when revenue allows.

.PARAMETER OutDir
    Where to write pilot.pfx / pilot.cer. Default: script directory.

.PARAMETER Subject
    Certificate subject. Default: "CN=Pharma Server Pilot, O=Pharma Server, C=CL".

.PARAMETER YearsValid
    Validity period. Default 3.

.NOTES
    Password is read from env var PHARMA_CERT_PASSWORD. NEVER pass it as a plain arg
    (it would leak into shell history / process list). If unset, the script aborts.

    Run once per dev machine. Re-running overwrites the cert (clients must re-import).
#>
[CmdletBinding()]
param(
    [string]$OutDir = $PSScriptRoot,
    [string]$Subject = "CN=Pharma Server Pilot, O=Pharma Server, C=CL",
    [int]$YearsValid = 3
)

$ErrorActionPreference = "Stop"

if (-not $env:PHARMA_CERT_PASSWORD) {
    Write-Error "PHARMA_CERT_PASSWORD env var is not set. Set it before running: `$env:PHARMA_CERT_PASSWORD = '<strong-password>'"
    exit 1
}

$pfxPath = Join-Path $OutDir "pilot.pfx"
$cerPath = Join-Path $OutDir "pilot.cer"

if (Test-Path $pfxPath) {
    Write-Warning "pilot.pfx already exists at $pfxPath."
    Write-Warning "Regenerating will invalidate all previously signed MSIs and force pilot clients to re-import the new cert."
    $confirm = Read-Host "Type 'REGENERATE' to overwrite, anything else to abort"
    if ($confirm -ne "REGENERATE") {
        Write-Host "Aborted. Existing cert preserved."
        exit 0
    }
}

Write-Host "Generating self-signed code-signing certificate..."
Write-Host "  Subject : $Subject"
Write-Host "  Validity: $YearsValid years"

$cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $Subject `
    -KeyAlgorithm RSA `
    -KeyLength 2048 `
    -KeyUsage DigitalSignature `
    -KeyExportPolicy Exportable `
    -NotAfter (Get-Date).AddYears($YearsValid) `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -HashAlgorithm SHA256

Write-Host "Created cert with thumbprint: $($cert.Thumbprint)"

$securePwd = ConvertTo-SecureString -String $env:PHARMA_CERT_PASSWORD -Force -AsPlainText

Export-PfxCertificate -Cert $cert -FilePath $pfxPath -Password $securePwd | Out-Null
Write-Host "Exported PRIVATE pfx -> $pfxPath  (KEEP SECRET, gitignored)"

Export-Certificate -Cert $cert -FilePath $cerPath -Type CERT | Out-Null
Write-Host "Exported PUBLIC  cer -> $cerPath  (distributable to pilot clients)"

# Remove from the personal store — the pfx is the source of truth from here on.
Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)" -Force
Write-Host "Removed cert from CurrentUser\My store (pfx is now the canonical copy)."

Write-Host ""
Write-Host "DONE. Next steps:"
Write-Host "  1. Commit pilot.cer (public). NEVER commit pilot.pfx (gitignored)."
Write-Host "  2. Sign an MSI:  pwsh installer/sign/sign-msi.ps1 -MsiPath <path-to.msi>"
Write-Host "  3. Pilot clients import pilot.cer to Trusted Publishers (see installer/sign/README.md)."

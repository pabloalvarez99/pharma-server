#Requires -Version 5.1
<#
.SYNOPSIS
    Verify the Authenticode signature on a signed MSI.

.DESCRIPTION
    Reports signer subject, timestamp, and trust status. For a pilot self-signed cert,
    full trust ("valid") only resolves on machines that imported pilot.cer to Trusted
    Publishers. On other machines the signature is present but untrusted (expected).

    Decision context: ADR-0008.

.PARAMETER MsiPath
    Path to the signed .msi (required).
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$MsiPath
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $MsiPath)) {
    Write-Error "MSI not found: $MsiPath"
    exit 1
}

$sig = Get-AuthenticodeSignature -FilePath $MsiPath

Write-Host "File         : $MsiPath"
Write-Host "Status       : $($sig.Status)"
Write-Host "StatusMessage: $($sig.StatusMessage)"

if ($sig.SignerCertificate) {
    Write-Host "Signer       : $($sig.SignerCertificate.Subject)"
    Write-Host "Thumbprint   : $($sig.SignerCertificate.Thumbprint)"
    Write-Host "NotAfter     : $($sig.SignerCertificate.NotAfter)"
} else {
    Write-Warning "No signer certificate found — MSI is UNSIGNED."
    exit 2
}

if ($sig.TimeStamperCertificate) {
    Write-Host "Timestamped  : YES ($($sig.TimeStamperCertificate.Subject))"
} else {
    Write-Warning "Timestamped  : NO — signature will expire with the cert. Re-sign with sign-msi.ps1 (it applies /tr)."
}

switch ($sig.Status) {
    "Valid" {
        Write-Host ""
        Write-Host "TRUSTED on this machine (pilot.cer is in Trusted Publishers, or cert chains to a trusted root)."
        exit 0
    }
    "UnknownError" {
        Write-Host ""
        Write-Host "Signature present but UNTRUSTED on this machine. For pilot self-signed this is EXPECTED unless pilot.cer was imported. Import it to test full trust."
        exit 0
    }
    default {
        Write-Host ""
        Write-Warning "Unexpected status: $($sig.Status). Investigate before distributing."
        exit 3
    }
}

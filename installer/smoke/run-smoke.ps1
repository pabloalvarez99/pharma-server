#Requires -Version 5.1
<#
.SYNOPSIS
    (RUNS ON HOST) Revert the smoke VM to baseline, copy the MSI in, run smoke-install.ps1, report.

.DESCRIPTION
    Orchestrates a clean-room smoke test:
      1. Revert Hyper-V VM to the "baseline" checkpoint (clean Windows).
      2. Start the VM, wait for WinRM.
      3. Copy the MSI + smoke-install.ps1 into the VM.
      4. Invoke smoke-install.ps1 inside the VM.
      5. Surface its exit code; revert the VM again to leave it clean.

    Decision context: regla #9 + zero-cost-launch-plan §3.

.PARAMETER VmName
    VM name. Default "PharmaSmoke".

.PARAMETER MsiPath
    Path to the MSI ON THE HOST. Required.

.PARAMETER VmCredential
    PSCredential for the VM local admin. If omitted, prompts.

.PARAMETER SnapshotName
    Baseline checkpoint to revert to. Default "baseline".

.NOTES
    Requires elevation + the VM created by setup-vm.ps1 with WinRM enabled inside.
    Exit 0 = smoke green. Non-zero = smoke failed (fail the release).
#>
[CmdletBinding()]
param(
    [string]$VmName = "PharmaSmoke",
    [Parameter(Mandatory = $true)]
    [string]$MsiPath,
    [System.Management.Automation.PSCredential]$VmCredential,
    [string]$SnapshotName = "baseline"
)

$ErrorActionPreference = "Stop"

$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) { Write-Error "Run elevated (Hyper-V cmdlets need admin)."; exit 1 }
if (-not (Test-Path $MsiPath)) { Write-Error "MSI not found on host: $MsiPath"; exit 1 }
if (-not (Get-VM -Name $VmName -ErrorAction SilentlyContinue)) {
    Write-Error "VM '$VmName' not found. Run setup-vm.ps1 first."
    exit 1
}
if (-not $VmCredential) {
    $VmCredential = Get-Credential -Message "VM local admin credentials for '$VmName'"
}

$smokeScript = Join-Path $PSScriptRoot "smoke-install.ps1"
if (-not (Test-Path $smokeScript)) { Write-Error "smoke-install.ps1 missing next to this script."; exit 1 }

Write-Host "=== Reverting '$VmName' to '$SnapshotName' ===" -ForegroundColor Cyan
Restore-VMSnapshot -VMName $VmName -Name $SnapshotName -Confirm:$false
Start-VM -Name $VmName | Out-Null

Write-Host "=== Waiting for WinRM ===" -ForegroundColor Cyan
$deadline = (Get-Date).AddMinutes(5)
$session = $null
while ((Get-Date) -lt $deadline) {
    try {
        $session = New-PSSession -VMName $VmName -Credential $VmCredential -ErrorAction Stop
        break
    } catch {
        Start-Sleep -Seconds 5
    }
}
if (-not $session) { Write-Error "Could not establish PSSession to '$VmName' within 5 min. Is WinRM enabled inside the VM?"; exit 1 }
Write-Host "PSSession established."

try {
    $vmMsi = "C:\Windows\Temp\pharma-smoke.msi"
    $vmSmoke = "C:\Windows\Temp\smoke-install.ps1"

    Write-Host "=== Copying MSI + smoke script into VM ===" -ForegroundColor Cyan
    Copy-Item -Path $MsiPath -Destination $vmMsi -ToSession $session -Force
    Copy-Item -Path $smokeScript -Destination $vmSmoke -ToSession $session -Force

    Write-Host "=== Running smoke-install.ps1 inside VM ===" -ForegroundColor Cyan
    $exit = Invoke-Command -Session $session -ScriptBlock {
        param($msi, $script)
        & powershell.exe -ExecutionPolicy Bypass -File $script -MsiPath $msi
        $LASTEXITCODE
    } -ArgumentList $vmMsi, $vmSmoke

    Write-Host ""
    if ($exit -eq 0) {
        Write-Host "=== SMOKE PASS (VM reported exit 0) ===" -ForegroundColor Green
    } else {
        Write-Host "=== SMOKE FAIL (VM reported exit $exit) ===" -ForegroundColor Red
    }
} finally {
    Remove-PSSession $session
    Write-Host "=== Reverting '$VmName' to clean baseline ===" -ForegroundColor Cyan
    Restore-VMSnapshot -VMName $VmName -Name $SnapshotName -Confirm:$false
    Stop-VM -Name $VmName -Force -TurnOff -ErrorAction SilentlyContinue | Out-Null
}

exit $exit

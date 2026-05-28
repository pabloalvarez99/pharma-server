#Requires -Version 5.1
<#
.SYNOPSIS
    (RUNS INSIDE THE VM) Smoke-test an MSI: install -> service up -> health 200 -> uninstall -> gone.

.DESCRIPTION
    Self-contained smoke test executed inside a clean Windows VM. Verifies the full
    install/uninstall lifecycle of the pharma-server MSI:
      1. msiexec /i  silent install                        -> exit 0
      2. Windows service "PharmaServer" reaches Running
      3. GET http://localhost:8080/health/ready            -> HTTP 200 (db ok)
      4. msiexec /x  silent uninstall                      -> exit 0
      5. Service "PharmaServer" no longer exists

    Decision context: regla #9 (smoke VM green before deploy) + zero-cost-launch-plan §3.

.PARAMETER MsiPath
    Path to the MSI inside the VM (copied in by run-smoke.ps1). Required.

.PARAMETER ServiceName
    Windows service name. Default "PharmaServer" (matches crates/service SERVICE_NAME).

.PARAMETER HealthUrl
    Readiness endpoint. Default http://localhost:8080/health/ready.

.PARAMETER TimeoutSec
    Max seconds to wait for the service + health. Default 60.

.NOTES
    Exit 0 = all green. Non-zero = failure (caller fails the release).
    Writes a verbose MSI log to $env:TEMP\pharma-smoke-install.log.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$MsiPath,
    [string]$ServiceName = "PharmaServer",
    [string]$HealthUrl = "http://localhost:8080/health/ready",
    [int]$TimeoutSec = 60
)

$ErrorActionPreference = "Stop"
$installLog = Join-Path $env:TEMP "pharma-smoke-install.log"
$uninstallLog = Join-Path $env:TEMP "pharma-smoke-uninstall.log"

function Fail($msg) {
    Write-Host "SMOKE FAIL: $msg" -ForegroundColor Red
    exit 1
}

if (-not (Test-Path $MsiPath)) { Fail "MSI not found in VM: $MsiPath" }

Write-Host "=== STEP 1: install ===" -ForegroundColor Cyan
$proc = Start-Process msiexec.exe -ArgumentList "/i `"$MsiPath`" /qn /norestart /l*v `"$installLog`"" -Wait -PassThru
if ($proc.ExitCode -ne 0) { Fail "msiexec install exited $($proc.ExitCode). See $installLog" }
Write-Host "install exit 0 OK"

Write-Host "=== STEP 2: service reaches Running ===" -ForegroundColor Cyan
$deadline = (Get-Date).AddSeconds($TimeoutSec)
$svc = $null
while ((Get-Date) -lt $deadline) {
    $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($svc -and $svc.Status -eq "Running") { break }
    Start-Sleep -Seconds 2
}
if (-not $svc) { Fail "service '$ServiceName' was not created by the MSI" }
if ($svc.Status -ne "Running") { Fail "service '$ServiceName' did not reach Running within ${TimeoutSec}s (status: $($svc.Status))" }
Write-Host "service '$ServiceName' Running OK"

Write-Host "=== STEP 3: health/ready -> 200 ===" -ForegroundColor Cyan
$healthy = $false
$deadline = (Get-Date).AddSeconds($TimeoutSec)
$lastErr = ""
while ((Get-Date) -lt $deadline) {
    try {
        $resp = Invoke-WebRequest -Uri $HealthUrl -UseBasicParsing -TimeoutSec 5
        if ($resp.StatusCode -eq 200) {
            $healthy = $true
            Write-Host "health body: $($resp.Content)"
            break
        }
    } catch {
        $lastErr = $_.Exception.Message
    }
    Start-Sleep -Seconds 2
}
if (-not $healthy) { Fail "health endpoint $HealthUrl did not return 200 within ${TimeoutSec}s (last error: $lastErr)" }
Write-Host "health 200 OK"

Write-Host "=== STEP 4: uninstall ===" -ForegroundColor Cyan
$proc = Start-Process msiexec.exe -ArgumentList "/x `"$MsiPath`" /qn /norestart /l*v `"$uninstallLog`"" -Wait -PassThru
if ($proc.ExitCode -ne 0) { Fail "msiexec uninstall exited $($proc.ExitCode). See $uninstallLog" }
Write-Host "uninstall exit 0 OK"

Write-Host "=== STEP 5: service removed ===" -ForegroundColor Cyan
Start-Sleep -Seconds 3
$svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($svc) { Fail "service '$ServiceName' still exists after uninstall (status: $($svc.Status))" }
Write-Host "service removed OK"

Write-Host ""
Write-Host "=== SMOKE PASS: install/health/uninstall all green ===" -ForegroundColor Green
exit 0

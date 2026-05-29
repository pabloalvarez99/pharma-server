$ErrorActionPreference = 'Continue'
$out = 'C:\out'
if (-not (Test-Path $out)) { New-Item -ItemType Directory -Path $out -Force | Out-Null }
$log = Join-Path $out 'api.out.log'
$err = Join-Path $out 'api.err.log'
$done = Join-Path $out 'diag.txt'
Remove-Item $log, $err, $done -Force -ErrorAction SilentlyContinue

# Run the API binary directly (same load_or_default + api::run as the service,
# but stdout is captured so the db-init error is visible to the host).
$p = Start-Process 'C:\bin\pharma-api.exe' -RedirectStandardOutput $log -RedirectStandardError $err -PassThru
Start-Sleep -Seconds 15

$summary = New-Object System.Collections.Generic.List[string]
try {
  $r = Invoke-WebRequest 'http://localhost:8080/health/ready' -UseBasicParsing -TimeoutSec 3
  $summary.Add("READY_STATUS=$($r.StatusCode)")
  $summary.Add("READY_BODY=$($r.Content)")
} catch {
  $summary.Add("READY_ERR=$_")
}

Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
$summary.Add('DIAG_DONE')
Set-Content -Path $done -Value $summary -Encoding utf8

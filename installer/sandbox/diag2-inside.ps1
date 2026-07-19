$ErrorActionPreference = 'Continue'
$out = 'C:\out'
if (-not (Test-Path $out)) { New-Item -ItemType Directory -Path $out -Force | Out-Null }
$log = Join-Path $out 'api-system.log'
$who = Join-Path $out 'who.txt'
$done = Join-Path $out 'diag2.txt'
Remove-Item $log, $who, $done -Force -ErrorAction SilentlyContinue

# Run pharma-api.exe AS LocalSystem (session 0) via Task Scheduler — same context
# as the Windows service — and capture stdout. Reproduces the service-only failure.
$tr = 'cmd /c whoami > C:\out\who.txt 2>&1 & C:\bin\pharma-api.exe > C:\out\api-system.log 2>&1'
schtasks /create /tn pdiag /tr $tr /sc once /st 23:59 /ru SYSTEM /rl HIGHEST /f | Out-Null
schtasks /run /tn pdiag | Out-Null
Start-Sleep -Seconds 15

$s = New-Object System.Collections.Generic.List[string]
try {
  $r = Invoke-WebRequest 'http://localhost:8080/health/ready' -UseBasicParsing -TimeoutSec 3
  $s.Add("READY_STATUS=$($r.StatusCode)")
  $s.Add("READY_BODY=$($r.Content)")
} catch { $s.Add("READY_ERR=$_") }

schtasks /end /tn pdiag 2>&1 | Out-Null
schtasks /delete /tn pdiag /f 2>&1 | Out-Null
$s.Add('DIAG2_DONE')
Set-Content -Path $done -Value $s -Encoding utf8

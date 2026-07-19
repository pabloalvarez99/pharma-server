$ErrorActionPreference = 'Stop'

# Result artifact written to the writable mapped folder so the HOST can read the
# verdict (sandbox console output is otherwise trapped inside the VM).
$out = 'C:\out'
if (-not (Test-Path $out)) { New-Item -ItemType Directory -Path $out -Force | Out-Null }
$result = Join-Path $out 'result.txt'
$lines = New-Object System.Collections.Generic.List[string]
function Log($m) { Write-Host $m; $lines.Add($m); Set-Content -Path $result -Value $lines -Encoding utf8 }

$pass = $true

$msi = Get-ChildItem 'C:\msi\pharma-server-*-x86_64.msi' | Sort-Object Name -Descending | Select-Object -First 1
if (-not $msi) { Log 'FAIL: NO MSI FOUND IN C:\msi'; Log 'SMOKE_DONE'; exit 1 }
Log "MSI: $($msi.Name)  ($([math]::Round($msi.Length/1MB,2)) MB)"

Log '[1/6] Install (/passive)'
$installLog = Join-Path $out 'install.log'
$p = Start-Process msiexec.exe -ArgumentList '/i', "`"$($msi.FullName)`"", '/passive', '/l*v', $installLog -Wait -PassThru
Log "  msiexec exit: $($p.ExitCode)"
if ($p.ExitCode -ne 0) { $pass = $false; Log "  FAIL: install exit $($p.ExitCode)" }

Log '[2/6] Service installed + RUNNING?'
$svc = sc.exe query PharmaServer 2>&1 | Out-String
Log ("  " + ($svc -replace "`r?`n","  "))
if ($svc -match 'RUNNING') { Log '  PASS: service RUNNING' } else { $pass = $false; Log '  FAIL: service not RUNNING' }

Log '[3/6] Wait /health/ready 200 (60s)'
$deadline = (Get-Date).AddSeconds(60)
$ready = $false
$lastBody = ''
$elapsed = 0
while ((Get-Date) -lt $deadline) {
  try {
    $r = Invoke-WebRequest 'http://localhost:8080/health/ready' -UseBasicParsing -TimeoutSec 10
    $lastBody = "$($r.StatusCode) $($r.Content)"
    if ($r.StatusCode -eq 200) { $ready = $true; break }
  } catch {
    $lastBody = "exc: $($_.Exception.Message)"
    if ($_.Exception.Response) { try { $lastBody = "$([int]$_.Exception.Response.StatusCode) (degraded)" } catch {} }
  }
  Start-Sleep -Seconds 1
  $elapsed++
}
Log "  last probe (@${elapsed}s): $lastBody"
if ($ready) { Log "  PASS: /health/ready 200 after ${elapsed}s" } else {
  $pass = $false; Log '  FAIL: /health/ready no 200 in 60s'
  Log '  --- DIAG on fail ---'
  $procs = Get-Process -Name pharma*,powershell,msedge* -ErrorAction SilentlyContinue | Select-Object Name,Id,Path
  foreach ($pr in $procs) { Log ("  proc: $($pr.Name) pid=$($pr.Id) path=$($pr.Path)") }
  $net = (netstat -ano | Select-String ':8080') -join ' || '
  Log "  netstat:8080: $net"
  $live = try { (Invoke-WebRequest 'http://localhost:8080/health/live' -UseBasicParsing -TimeoutSec 5).Content } catch { "ERR $($_.Exception.Message)" }
  Log "  /health/live: $live"
  $lockls = Get-ChildItem 'C:\ProgramData\PharmaServer\data' -Recurse -ErrorAction SilentlyContinue | Select-Object -First 20 | ForEach-Object { $_.FullName }
  Log ("  datadir: " + ($lockls -join ' | '))
  $r2 = try { $x=Invoke-WebRequest 'http://localhost:8080/health/ready' -UseBasicParsing -TimeoutSec 20; "$($x.StatusCode) $($x.Content)" } catch { "ERR $($_.Exception.Message)" }
  Log "  /health/ready retry(20s): $r2"
}

Log '[4/6] GET / 200?'
try {
  $root = Invoke-WebRequest 'http://localhost:8080/' -UseBasicParsing -TimeoutSec 3
  Log "  / StatusCode: $($root.StatusCode)"
  if ($root.StatusCode -ne 200) { $pass = $false; Log '  FAIL: / not 200' }
} catch { $pass = $false; Log "  FAIL: / unreachable: $_" }

Log '[5/6] Start Menu shortcut?'
$shortcut = 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Pharma Server\Pharma Server Dashboard.lnk'
if (Test-Path $shortcut) { Log '  PASS: shortcut present' } else { $pass = $false; Log '  FAIL: shortcut missing' }

Log '[6/6] Uninstall clean?'
$u = Start-Process msiexec.exe -ArgumentList '/x', "`"$($msi.FullName)`"", '/passive' -Wait -PassThru
Log "  uninstall exit: $($u.ExitCode)"
Start-Sleep -Seconds 2
$svc2 = sc.exe query PharmaServer 2>&1 | Out-String
if ($svc2 -match '1060') { Log '  PASS: service removed' } else { $pass = $false; Log '  FAIL: service still present after uninstall' }

if ($pass) { Log 'VERDICT: GREEN' } else { Log 'VERDICT: RED' }
Log 'SMOKE_DONE'

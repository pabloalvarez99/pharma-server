$ErrorActionPreference = 'Continue'
$out = 'C:\out'
if (-not (Test-Path $out)) { New-Item -ItemType Directory -Path $out -Force | Out-Null }
$done = Join-Path $out 'diag3.txt'
Remove-Item $done -Force -ErrorAction SilentlyContinue
$s = New-Object System.Collections.Generic.List[string]
function Add2($m){ $s.Add($m); Set-Content -Path $done -Value $s -Encoding utf8 }

# Register + start the REAL service binary (same as MSI does) as LocalSystem.
sc.exe create PharmaServer binPath= "C:\bin\pharma-service.exe" start= demand obj= LocalSystem | Out-Null
$start = sc.exe start PharmaServer 2>&1 | Out-String
Add2 ("sc start: " + ($start -replace "`r?`n"," "))

Start-Sleep -Seconds 6
$q = sc.exe query PharmaServer 2>&1 | Out-String
Add2 ("sc query: " + (($q -replace "`r?`n"," ") -replace '\s+',' '))

# /health/live — no DB, proves server is up
try { $l = Invoke-WebRequest 'http://localhost:8080/health/live' -UseBasicParsing -TimeoutSec 8; Add2 "LIVE=$($l.StatusCode) $($l.Content)" } catch { Add2 "LIVE_ERR=$($_.Exception.Message)" }

# / — static
try { $root = Invoke-WebRequest 'http://localhost:8080/' -UseBasicParsing -TimeoutSec 8; Add2 "ROOT=$($root.StatusCode)" } catch { Add2 "ROOT_ERR=$($_.Exception.Message)" }

# /health/ready — DB query; the suspect
try { $rd = Invoke-WebRequest 'http://localhost:8080/health/ready' -UseBasicParsing -TimeoutSec 12; Add2 "READY=$($rd.StatusCode) $($rd.Content)" }
catch {
  $code = ''
  if ($_.Exception.Response) { try { $code = [int]$_.Exception.Response.StatusCode } catch {} }
  Add2 "READY_ERR=$($_.Exception.Message) httpcode=$code"
}

sc.exe stop PharmaServer 2>&1 | Out-Null
Start-Sleep -Seconds 2
sc.exe delete PharmaServer 2>&1 | Out-Null
Add2 'DIAG3_DONE'

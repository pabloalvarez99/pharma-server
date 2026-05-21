$ErrorActionPreference = 'Stop'
$msi = Get-ChildItem 'C:\msi\pharma-server-*-x86_64.msi' | Sort-Object Name -Descending | Select-Object -First 1
if (-not $msi) { Write-Host 'NO MSI FOUND IN C:\msi'; exit 1 }
Write-Host "============================================"
Write-Host "Pharma Server smoke install (sandbox)"
Write-Host "MSI: $($msi.Name)"
Write-Host "Size: $([math]::Round($msi.Length / 1MB, 2)) MB"
Write-Host "============================================"
Write-Host ''

Write-Host '[1/6] Install (passive mode, auto-launches dashboard)'
$installLog = 'C:\Users\WDAGUtilityAccount\install.log'
Start-Process msiexec.exe -ArgumentList '/i', "`"$($msi.FullName)`"", '/passive', '/l*v', $installLog -Wait
Write-Host "Install log -> $installLog"
Write-Host ''

Write-Host '[2/6] Verify service installed'
sc.exe query PharmaServer
Write-Host ''

Write-Host '[3/6] Wait for /ready (15s timeout)'
$deadline = (Get-Date).AddSeconds(15)
$ok = $false
while ((Get-Date) -lt $deadline) {
  try {
    $r = Invoke-WebRequest 'http://localhost:8080/' -UseBasicParsing -TimeoutSec 2
    if ($r.StatusCode -eq 200) { $ok = $true; break }
  } catch {}
  Start-Sleep -Milliseconds 500
}
if (-not $ok) { Write-Host 'API DID NOT RESPOND IN 15s' -ForegroundColor Red } else { Write-Host 'API responded OK' -ForegroundColor Green }
Write-Host ''

Write-Host '[4/6] curl /'
Invoke-WebRequest 'http://localhost:8080/' -UseBasicParsing | Select-Object StatusCode, Content
Write-Host ''

Write-Host '[5/6] Start Menu shortcut present?'
$shortcut = 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Pharma Server\Pharma Server Dashboard.lnk'
if (Test-Path $shortcut) { Write-Host "Shortcut OK -> $shortcut" -ForegroundColor Green } else { Write-Host 'Shortcut MISSING' -ForegroundColor Red }
Write-Host ''

Write-Host '[6/6] DONE — manual uninstall when ready:'
Write-Host "    msiexec /x `"$($msi.FullName)`" /passive"

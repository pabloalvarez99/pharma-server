$ErrorActionPreference = 'SilentlyContinue'
$url = 'http://localhost:8080/'
$dashboard = 'http://localhost:8080/app'
$deadline = (Get-Date).AddSeconds(15)
while ((Get-Date) -lt $deadline) {
    try {
        $resp = Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 2
        if ($resp.StatusCode -eq 200) {
            Start-Process $dashboard
            exit 0
        }
    } catch {}
    Start-Sleep -Milliseconds 500
}
Start-Process $dashboard
exit 0

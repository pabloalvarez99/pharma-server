#Requires -Version 5.1
<#
.SYNOPSIS
    Pharma Server client launcher (League-of-Legends style).

.DESCRIPTION
    Double-click experience for opening the Pharma Server dashboard as a desktop
    "client", not a browser tab:

      1. Shows a borderless splash window (logo + status + progress) immediately.
      2. Ensures the backend is up:
           - If the Windows service "PharmaServer" exists but is stopped, starts it
             (self-elevates once if the start needs admin).
           - If no service is installed (dev box), just waits — assumes the operator
             ran `pharma-service`/`cargo run` manually.
      3. Polls http://<host>:<port>/health/ready until 200 (or timeout).
      4. Launches the dashboard chromeless via Edge/Chrome `--app=` (falls back to the
         default browser). Chromeless = no address bar/tabs → feels native.
      5. Closes the splash.

    No console window: invoke through pharma-launcher.vbs (hidden host). Running the
    .ps1 directly still works, it just flashes a console.

.PARAMETER Host
    Backend host. Default 127.0.0.1.

.PARAMETER Port
    Backend port. Default 8080 (matches config/default.toml bind).

.PARAMETER TimeoutSec
    Max seconds to wait for /health/ready. Default 40.

.NOTES
    Zero new dependencies — WinForms + System.Drawing ship with .NET on Windows.
#>
[CmdletBinding()]
param(
    [string]$BackendHost = "127.0.0.1",
    [int]$Port = 8080,
    [int]$TimeoutSec = 40
)

$ErrorActionPreference = "Stop"
$ServiceName = "PharmaServer"
$baseUrl  = "http://${BackendHost}:${Port}"
$healthUrl = "$baseUrl/health/ready"
$appUrl    = "$baseUrl/app"

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

# ---------------------------------------------------------------------------
# Splash window (borderless, centered, dark — client boot screen).
# ---------------------------------------------------------------------------
$brandGreen = [System.Drawing.Color]::FromArgb(16, 185, 129)   # emerald
$bgDark     = [System.Drawing.Color]::FromArgb(17, 24, 39)      # slate-900
$fgLight    = [System.Drawing.Color]::FromArgb(229, 231, 235)   # gray-200
$fgMuted    = [System.Drawing.Color]::FromArgb(148, 163, 184)   # slate-400

$form = New-Object System.Windows.Forms.Form
$form.FormBorderStyle = 'None'
$form.StartPosition   = 'CenterScreen'
$form.Size            = New-Object System.Drawing.Size(460, 280)
$form.BackColor       = $bgDark
$form.TopMost         = $true
$form.ShowInTaskbar   = $true
$form.Text            = "Pharma Server"

# Logo mark (green rounded square + white cross), drawn at runtime.
$logo = New-Object System.Windows.Forms.PictureBox
$logo.Size     = New-Object System.Drawing.Size(72, 72)
$logo.Location = New-Object System.Drawing.Point(194, 40)
$logo.BackColor = $bgDark
$logoBmp = New-Object System.Drawing.Bitmap 72, 72
$g = [System.Drawing.Graphics]::FromImage($logoBmp)
$g.SmoothingMode = 'AntiAlias'
$g.Clear($bgDark)
$brush = New-Object System.Drawing.SolidBrush $brandGreen
$g.FillRectangle($brush, 6, 6, 60, 60)
$white = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::White)
$g.FillRectangle($white, 31, 18, 10, 36)   # vertical bar of cross
$g.FillRectangle($white, 18, 31, 36, 10)   # horizontal bar of cross
$g.Dispose()
$logo.Image = $logoBmp

$title = New-Object System.Windows.Forms.Label
$title.Text      = "PHARMA SERVER"
$title.Font      = New-Object System.Drawing.Font("Segoe UI Semibold", 16, [System.Drawing.FontStyle]::Bold)
$title.ForeColor = $fgLight
$title.AutoSize  = $false
$title.TextAlign = 'MiddleCenter'
$title.Size      = New-Object System.Drawing.Size(460, 30)
$title.Location  = New-Object System.Drawing.Point(0, 124)
$title.BackColor = $bgDark

$status = New-Object System.Windows.Forms.Label
$status.Text      = "Iniciando cliente..."
$status.Font      = New-Object System.Drawing.Font("Segoe UI", 10)
$status.ForeColor = $fgMuted
$status.AutoSize  = $false
$status.TextAlign = 'MiddleCenter'
$status.Size      = New-Object System.Drawing.Size(460, 24)
$status.Location  = New-Object System.Drawing.Point(0, 158)
$status.BackColor = $bgDark

$bar = New-Object System.Windows.Forms.ProgressBar
$bar.Style    = 'Marquee'
$bar.MarqueeAnimationSpeed = 30
$bar.Size     = New-Object System.Drawing.Size(360, 8)
$bar.Location = New-Object System.Drawing.Point(50, 198)

$form.Controls.AddRange(@($logo, $title, $status, $bar))
$form.Show()
$form.Refresh()
[System.Windows.Forms.Application]::DoEvents()

function Set-Status([string]$text) {
    $status.Text = $text
    $status.Refresh()
    [System.Windows.Forms.Application]::DoEvents()
}

# ---------------------------------------------------------------------------
# Step 1: ensure a backend is up. Priority:
#   (a) already listening on the port -> reuse it (manual run / prior launch).
#   (b) Windows service installed     -> start it (customer/MSI path; elevate).
#   (c) dev box (no service)          -> spawn the locally-built pharma-api.exe.
# ---------------------------------------------------------------------------
function Test-Listening([int]$p) {
    $null -ne (Get-NetTCPConnection -LocalPort $p -State Listen -ErrorAction SilentlyContinue)
}

if (Test-Listening $Port) {
    Set-Status "Servidor ya activo..."
}
elseif ($svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) {
    if ($svc.Status -ne 'Running') {
        Set-Status "Iniciando servicio PharmaServer..."
        try {
            Start-Service -Name $ServiceName -ErrorAction Stop
        } catch {
            # Needs elevation — relaunch this start once, elevated.
            Set-Status "Solicitando permisos para iniciar el servicio..."
            try {
                Start-Process -FilePath "powershell.exe" `
                    -ArgumentList @("-NoProfile", "-WindowStyle", "Hidden", "-Command",
                        "Start-Service -Name $ServiceName") `
                    -Verb RunAs -Wait
            } catch {
                Set-Status "No se pudo iniciar el servicio automaticamente."
            }
        }
    }
}
else {
    # Dev mode: no service installed → boot the locally-built API binary.
    # Prefer release (fast), fall back to debug. CWD = repo root so
    # config/default.toml + ./data/surreal resolve. Left running on purpose:
    # the next launch sees the port already listening and opens instantly.
    $repo = Split-Path $PSScriptRoot -Parent
    $bin = $null
    foreach ($rel in @("target\release\pharma-api.exe", "target\debug\pharma-api.exe")) {
        $cand = Join-Path $repo $rel
        if (Test-Path $cand) { $bin = $cand; break }
    }
    if ($bin) {
        Set-Status "Iniciando servidor local (dev)..."
        # Dev box only: the API refuses to boot on the placeholder JWT secret
        # (crates/api check_jwt_secret). This is a localhost-only dev launch, so
        # opt into the insecure-dev escape hatch. Production (service/MSI path b)
        # injects a real PHARMA__JWT__SECRET instead — never reaches here.
        $env:PHARMA_ALLOW_INSECURE_JWT = "1"
        $logDir = Join-Path $env:LOCALAPPDATA "PharmaServer"
        if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Path $logDir -Force | Out-Null }
        $log = Join-Path $logDir "dev-server.log"
        try {
            Start-Process -FilePath $bin -WorkingDirectory $repo -WindowStyle Hidden `
                -RedirectStandardOutput $log -RedirectStandardError "$log.err"
        } catch {
            Start-Process -FilePath $bin -WorkingDirectory $repo -WindowStyle Hidden
        }
    } else {
        Set-Status "Servidor no encontrado."
        [System.Windows.Forms.MessageBox]::Show(
            "No hay servicio 'PharmaServer' instalado ni binario compilado.`n`nInstala el MSI, o compila el servidor:`n    cargo build -p api --release",
            "Pharma Server", 'OK', 'Warning') | Out-Null
        $form.Close()
        exit 1
    }
}

# ---------------------------------------------------------------------------
# Step 2: poll readiness.
# ---------------------------------------------------------------------------
Set-Status "Esperando servidor ($baseUrl)..."
$deadline = (Get-Date).AddSeconds($TimeoutSec)
$ready = $false
while ((Get-Date) -lt $deadline) {
    try {
        $resp = Invoke-WebRequest -Uri $healthUrl -UseBasicParsing -TimeoutSec 3
        if ($resp.StatusCode -eq 200) { $ready = $true; break }
    } catch { }
    Start-Sleep -Milliseconds 600
    [System.Windows.Forms.Application]::DoEvents()
}

if (-not $ready) {
    $bar.Style = 'Continuous'; $bar.Value = 0
    Set-Status "No se pudo conectar al servidor."
    [System.Windows.Forms.MessageBox]::Show(
        "No se pudo conectar a $baseUrl en $TimeoutSec s.`n`nVerifica que el servicio 'PharmaServer' este instalado y corriendo, o inicia el servidor manualmente.",
        "Pharma Server", 'OK', 'Warning') | Out-Null
    $form.Close()
    exit 1
}

# ---------------------------------------------------------------------------
# Step 3: launch chromeless app window.
# ---------------------------------------------------------------------------
Set-Status "Abriendo cliente..."

function Find-Browser {
    $candidates = @(
        "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
        "${env:ProgramFiles}\Microsoft\Edge\Application\msedge.exe",
        "${env:ProgramFiles}\Google\Chrome\Application\chrome.exe",
        "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe",
        "${env:LOCALAPPDATA}\Google\Chrome\Application\chrome.exe"
    )
    foreach ($c in $candidates) { if ($c -and (Test-Path $c)) { return $c } }
    return $null
}

$browser = Find-Browser
if ($browser) {
    # --app gives a chromeless, single-purpose window (no tabs/omnibox) = native feel.
    $profileDir = Join-Path $env:LOCALAPPDATA "PharmaServer\client-profile"
    Start-Process -FilePath $browser -ArgumentList @(
        "--app=$appUrl",
        "--window-size=1280,800",
        "--user-data-dir=`"$profileDir`"",
        "--no-first-run",
        "--no-default-browser-check"
    )
} else {
    # No Chromium browser — fall back to whatever handles http.
    Start-Process $appUrl
}

Start-Sleep -Milliseconds 800
$form.Close()
exit 0

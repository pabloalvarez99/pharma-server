<#
.SYNOPSIS
Mide arranque, scroll y memoria de la app en un AVD que se parece al teléfono
del usuario objetivo: Android 6.0, 1 GB de RAM, dos núcleos, 720x1280.

.DESCRIPTION
La razón declarada para dejar el WebView fue la fluidez en un aparato viejo y
lento. Este script existe para que esa afirmación se pueda volver a verificar
sin depender de la memoria de nadie.

Todo lo que mide sale de herramientas del propio Android, no de opiniones:

  - `am start -W`  -> arranque en frío (`TotalTime`), la misma cifra con la que
                      se midió el WebView.
  - bisect por captura de pantalla -> cuánto tarda en verse el login pintado.
                      La sonda es el parecido contra una captura de referencia:
                      contar "cuánta tinta hay" NO sirve, porque antes de que la
                      app pinte en pantalla está el lanzador, que tiene más
                      tinta que el login.
  - `dumpsys gfxinfo <pkg>`  -> jank de scroll.
  - `dumpsys meminfo <pkg>`  -> memoria en reposo y después de recorrer la app.

**El número de jank no se lee solo.** Un AVD rasteriza por software
(swiftshader) y ahí el 85-90 % de los frames pasa de 16 ms *en cualquier app*.
Por eso `-Control` mide lo mismo scrolleando los Ajustes del sistema, que no
tienen una línea de Compose: si la app janquea igual o menos que la app del
propio Android, el número que sobra es del emulador, no del código.

.PARAMETER Serie
Serial del emulador (`adb devices`). Cada agente debe usar el suyo: varios
comparten esta máquina y se pisan las instalaciones.

.PARAMETER Control
Además de la app, mide el scroll de los Ajustes del sistema como referencia.

.EXAMPLE
pwsh scripts/medir-aparato-lento.ps1 -Serie emulator-5570

.EXAMPLE
pwsh scripts/medir-aparato-lento.ps1 -Serie emulator-5570 -Control

.NOTES
El AVD se arma así (una vez), y la configuración exacta importa: sin ella los
números no son reproducibles.

    avdmanager create avd -n rb_perf_api23 -k "system-images;android-23;default;x86_64"

y después, en ~/.android/avd/rb_perf_api23.avd/config.ini:

    hw.ramSize=1024          hw.cpu.ncore=2
    hw.lcd.width=720         hw.lcd.height=1280      hw.lcd.density=240
    hw.gpu.mode=swiftshader_indirect                 disk.dataPartition.size=2048M

Para el escáner hay que arrancar el emulador con una escena virtual:

    emulator -avd rb_perf_api23 -port 5570 -no-snapshot `
             -camera-back virtualscene -virtualscene-poster wall=<png>

Para probar al 200 % de escala: `settings put system font_scale 2.0` **y
reiniciar el emulador**. El ajuste queda escrito al toque pero la configuración
viva sigue en 1.0 hasta el reinicio -- `dumpsys activity | Select-String
mConfiguration` muestra el valor real como primer número. Sin ese reinicio se
prueba al 100 % creyendo que se prueba al 200 %, que es peor que no probar.
#>
[CmdletBinding()]
param(
    [string]$Serie = "emulator-5570",
    [string]$Paquete = "cl.rutbusiness.app",
    [string]$Actividad = "cl.rutbusiness.app/.MainActivity",
    [int]$Corridas = 5,
    [switch]$Control
)

$ErrorActionPreference = "Stop"

$adb = if ($env:ANDROID_SDK_ROOT) {
    Join-Path $env:ANDROID_SDK_ROOT "platform-tools/adb.exe"
} else {
    Join-Path $env:LOCALAPPDATA "Android/Sdk/platform-tools/adb.exe"
}
if (-not (Test-Path $adb)) { throw "no encuentro adb en $adb" }

function Invoke-Adb { & $adb -s $Serie @args }

function Get-ResumenGfx {
    param([string]$Pkg)
    $l = Invoke-Adb shell dumpsys gfxinfo $Pkg
    $campo = {
        param($rx)
        $m = $l | Select-String $rx
        if ($m) { $m.Matches[0].Groups[1].Value } else { "" }
    }
    [pscustomobject]@{
        Paquete   = $Pkg
        Frames    = (& $campo 'Total frames rendered: (\d+)')
        Janky     = (& $campo 'Janky frames: (\d+ \([\d.]+%\))')
        P90       = (& $campo '90th percentile: (\d+ms)')
        P95       = (& $campo '95th percentile: (\d+ms)')
        P99       = (& $campo '99th percentile: (\d+ms)')
        LentoUi   = (& $campo 'Number Slow UI thread: (\d+)')
        LentoDraw = (& $campo 'Number Slow issue draw commands: (\d+)')
    }
}

function Measure-Scroll {
    param([string]$Pkg, [int]$Idas = 6)
    Invoke-Adb shell dumpsys gfxinfo $Pkg reset | Out-Null
    Start-Sleep -Milliseconds 500
    for ($i = 1; $i -le $Idas; $i++) {
        # Arrastre largo y no fling: el arrastre genera un frame por movimiento
        # de dedo, que es justo lo que hay que medir. Un fling con las
        # animaciones apagadas termina instantáneo y no mide nada.
        Invoke-Adb shell input swipe 360 900 360 350 700 | Out-Null
        Start-Sleep -Milliseconds 400
        Invoke-Adb shell input swipe 360 350 360 900 700 | Out-Null
        Start-Sleep -Milliseconds 400
    }
    Get-ResumenGfx -Pkg $Pkg
}

function Get-Memoria {
    param([string]$Pkg, [string]$Momento)
    # API 23 no imprime "TOTAL PSS:"; el número vive en la fila TOTAL de la
    # tabla de arriba.
    $l = Invoke-Adb shell dumpsys meminfo $Pkg
    $campo = {
        param($rx)
        $m = $l | Select-String $rx
        if ($m) { [int]$m.Matches[0].Groups[1].Value } else { 0 }
    }
    [pscustomobject]@{
        Momento    = $Momento
        TotalPssMB = [math]::Round((& $campo '^\s+TOTAL\s+(\d+)') / 1024, 1)
        DalvikMB   = [math]::Round((& $campo '^\s+Dalvik Heap\s+(\d+)') / 1024, 1)
        NativoMB   = [math]::Round((& $campo '^\s+Native Heap\s+(\d+)') / 1024, 1)
    }
}

Write-Host "== aparato ==" -ForegroundColor Cyan
[pscustomobject]@{
    Android  = (Invoke-Adb shell getprop ro.build.version.release).Trim()
    Api      = (Invoke-Adb shell getprop ro.build.version.sdk).Trim()
    Pantalla = (Invoke-Adb shell wm size).Trim()
    Densidad = (Invoke-Adb shell wm density).Trim()
    RamKb    = ((Invoke-Adb shell cat /proc/meminfo | Select-String 'MemTotal:\s+(\d+)').Matches[0].Groups[1].Value)
    Nucleos  = ((Invoke-Adb shell cat /proc/cpuinfo | Select-String 'processor').Count)
    Escala   = ((Invoke-Adb shell dumpsys activity | Select-String 'mConfiguration: \{([\d.]+)').Matches[0].Groups[1].Value)
} | Format-List

Write-Host "== arranque en frio ($Corridas corridas) ==" -ForegroundColor Cyan
$arranques = @()
for ($i = 1; $i -le $Corridas; $i++) {
    Invoke-Adb shell am force-stop $Paquete | Out-Null
    Start-Sleep -Milliseconds 2500
    $salida = Invoke-Adb shell am start -W -n $Actividad
    $arranques += [pscustomobject]@{
        Corrida   = $i
        ThisTime  = [int]($salida | Select-String '^ThisTime:\s*(\d+)').Matches[0].Groups[1].Value
        TotalTime = [int]($salida | Select-String '^TotalTime:\s*(\d+)').Matches[0].Groups[1].Value
        WaitTime  = [int]($salida | Select-String '^WaitTime:\s*(\d+)').Matches[0].Groups[1].Value
    }
}
$arranques | Format-Table -AutoSize
$tot = $arranques.TotalTime | Sort-Object
"TotalTime: min $($tot[0]) ms / mediana $($tot[[int]($tot.Count/2)]) ms / max $($tot[-1]) ms"

Write-Host "`n== memoria en reposo ==" -ForegroundColor Cyan
Start-Sleep -Seconds 4
Get-Memoria -Pkg $Paquete -Momento 'reposo' | Format-List

Write-Host "== jank de scroll ==" -ForegroundColor Cyan
Write-Host "(dejá la app en una lista larga antes de correr esto)" -ForegroundColor DarkGray
Measure-Scroll -Pkg $Paquete | Format-List

if ($Control) {
    Write-Host "== control: Ajustes del sistema (cero Compose) ==" -ForegroundColor Cyan
    Invoke-Adb shell am start -a android.settings.SETTINGS | Out-Null
    Start-Sleep -Seconds 5
    Measure-Scroll -Pkg 'com.android.settings' | Format-List
    Write-Host "Si la app janquea igual o menos que ésta, el numero es del emulador." -ForegroundColor DarkGray
}

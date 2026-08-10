<#
.SYNOPSIS
    Cross-compila artefactos Rust del server embebido para Android (H1+).

.DESCRIPTION
    Deriva el NDK de ANDROID_HOME (o NDK_HOME / ANDROID_NDK_HOME si ya están)
    y exporta CARGO_TARGET_*_LINKER a los wrappers clang del NDK API 23
    (minSdk del cliente). No escribe rutas absolutas en .cargo/config.toml
    (archivo versionado: otra máquina no tiene el mismo usuario/path).

    Por defecto compila el example android_kv_probe (H1) en release para las
    tres ABIs. Con -Crate / -Profile se reutiliza para el cdylib (H2+).

.PARAMETER Targets
    Lista de triples Rust. Default: aarch64, armv7, x86_64 (emulador).

.PARAMETER Crate
    Paquete cargo a construir. Default: db (el probe vive ahí).

.PARAMETER Example
    Si se indica, `cargo build -p $Crate --example $Example`.

.PARAMETER Lib
    Si se indica, construye la lib del crate (cdylib).

.PARAMETER Profile
    Perfil cargo (release, release-android, …). Default: release.

.PARAMETER NdkVersion
    Subcarpeta bajo $ANDROID_HOME\ndk. Default: 27.2.12479018
    (mismo pin que client-android/app/build.gradle.kts).

.PARAMETER ApiLevel
    API de los wrappers clang del NDK. Default: 23 (= minSdk).

.EXAMPLE
    pwsh client-android/scripts/compilar-servidor.ps1

.EXAMPLE
    pwsh client-android/scripts/compilar-servidor.ps1 -Crate servidor-android -Lib -Profile release-android
#>
[CmdletBinding()]
param(
    [string[]]$Targets = @(
        'aarch64-linux-android',
        'armv7-linux-androideabi',
        'x86_64-linux-android'
    ),
    [string]$Crate = 'db',
    [string]$Example = 'android_kv_probe',
    [switch]$Lib,
    [string]$Profile = 'release',
    [string]$NdkVersion = '27.2.12479018',
    [int]$ApiLevel = 23
)

$ErrorActionPreference = 'Stop'

# Repo root = parent of client-android/
$raizRepo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $raizRepo

function Resolve-NdkRoot {
    if ($env:NDK_HOME -and (Test-Path $env:NDK_HOME)) {
        return (Resolve-Path $env:NDK_HOME).Path
    }
    if ($env:ANDROID_NDK_HOME -and (Test-Path $env:ANDROID_NDK_HOME)) {
        return (Resolve-Path $env:ANDROID_NDK_HOME).Path
    }
    $sdk = $env:ANDROID_HOME
    if (-not $sdk) {
        $sdk = $env:ANDROID_SDK_ROOT
    }
    if (-not $sdk) {
        throw "ANDROID_HOME / ANDROID_SDK_ROOT / NDK_HOME / ANDROID_NDK_HOME vacíos. Seteá ANDROID_HOME al SDK de Android."
    }
    $candidate = Join-Path $sdk "ndk\$NdkVersion"
    if (-not (Test-Path $candidate)) {
        throw "NDK no encontrado en '$candidate'. Instalalo o pasá -NdkVersion."
    }
    return (Resolve-Path $candidate).Path
}

function Get-ClangWrapper {
    param(
        [Parameter(Mandatory)][string]$NdkRoot,
        [Parameter(Mandatory)][string]$RustTarget,
        [Parameter(Mandatory)][int]$Api
    )
    $prebuilt = Join-Path $NdkRoot 'toolchains\llvm\prebuilt\windows-x86_64\bin'
    if (-not (Test-Path $prebuilt)) {
        throw "Toolchain NDK windows-x86_64 no está en '$prebuilt'."
    }

    # Rust triple → prefijo del wrapper clang del NDK
    $clangPrefix = switch ($RustTarget) {
        'aarch64-linux-android' { "aarch64-linux-android$Api" }
        'armv7-linux-androideabi' { "armv7a-linux-androideabi$Api" }
        'x86_64-linux-android' { "x86_64-linux-android$Api" }
        'i686-linux-android' { "i686-linux-android$Api" }
        default { throw "Target no mapeado a clang NDK: $RustTarget" }
    }

    # Prefer .cmd on Windows so cargo (which shells out) finds a runnable file
    $cmd = Join-Path $prebuilt "$clangPrefix-clang.cmd"
    $exe = Join-Path $prebuilt "$clangPrefix-clang"
    if (Test-Path $cmd) { return (Resolve-Path $cmd).Path }
    if (Test-Path $exe) { return (Resolve-Path $exe).Path }
    throw "No hay clang para $RustTarget (API $Api) en $prebuilt"
}

function Get-LinkerEnvName {
    param([Parameter(Mandatory)][string]$RustTarget)
    # CARGO_TARGET_<TRIPLE_UPPER_UNDERSCORE>_LINKER
    $key = $RustTarget.ToUpperInvariant() -replace '-', '_'
    return "CARGO_TARGET_${key}_LINKER"
}

function Get-LlvmTool {
    param(
        [Parameter(Mandatory)][string]$NdkRoot,
        [Parameter(Mandatory)][string]$Name
    )
    $prebuilt = Join-Path $NdkRoot 'toolchains\llvm\prebuilt\windows-x86_64\bin'
    $exe = Join-Path $prebuilt "$Name.exe"
    if (Test-Path $exe) { return (Resolve-Path $exe).Path }
    $bare = Join-Path $prebuilt $Name
    if (Test-Path $bare) { return (Resolve-Path $bare).Path }
    throw "No hay $Name en $prebuilt"
}

function Set-TargetCcEnv {
    param(
        [Parameter(Mandatory)][string]$RustTarget,
        [Parameter(Mandatory)][string]$Clang,
        [Parameter(Mandatory)][string]$Ar
    )
    # cc-rs / ring leen CC_<triple>, CC_<triple_underscore>, TARGET_CC y AR_*.
    # El linker de cargo NO alcanza: ring compila asm C en su build.rs.
    $under = $RustTarget -replace '-', '_'
    Set-Item -Path "Env:CC_$RustTarget" -Value $Clang
    Set-Item -Path "Env:CC_$under" -Value $Clang
    Set-Item -Path "Env:CXX_$RustTarget" -Value ($Clang -replace '-clang(\.cmd)?$', '-clang++$1')
    # Prefer same clang as CXX if ++ wrapper missing later; keep simple:
    $cxx = $Clang -replace 'clang\.cmd$', 'clang++.cmd' -replace 'clang$', 'clang++'
    Set-Item -Path "Env:CXX_$RustTarget" -Value $cxx
    Set-Item -Path "Env:CXX_$under" -Value $cxx
    Set-Item -Path "Env:AR_$RustTarget" -Value $Ar
    Set-Item -Path "Env:AR_$under" -Value $Ar
    # Also TARGET_CC for the active build (cc-rs checks this)
    $env:TARGET_CC = $Clang
    $env:TARGET_AR = $Ar
}

$ndkRoot = Resolve-NdkRoot
Write-Host "NDK: $ndkRoot"
Write-Host "API: $ApiLevel"
Write-Host "Repo: $raizRepo"
Write-Host "Crate: $Crate  Profile: $Profile  Lib: $Lib  Example: $(if ($Lib) { '(lib)' } else { $Example })"
Write-Host ""

# Guard: nunca correr cargo desde un shell cuyo PATH ponga el link.exe de Git
# por delante del de MSVC (error disfrazado: "link: extra operand '...rcgu.o'").
$gitLink = 'C:\Program Files\Git\usr\bin\link.exe'
if (Test-Path $gitLink) {
    $pathParts = $env:PATH -split ';' | Where-Object { $_ -and ($_ -notmatch '\\Git\\usr\\bin') }
    $env:PATH = ($pathParts -join ';')
    Write-Host "PATH: quitado Git\\usr\\bin (evita link.exe de coreutils)"
}

# Put NDK llvm bin on PATH so bare tool names (clang, llvm-ar) resolve if a
# crate ignores CC_* env. Does not put absolute user paths into versioned files.
$ndkBin = Join-Path $ndkRoot 'toolchains\llvm\prebuilt\windows-x86_64\bin'
if ($env:PATH -notlike "*$ndkBin*") {
    $env:PATH = "$ndkBin;$env:PATH"
    Write-Host "PATH: prepend NDK llvm bin"
}

$ar = Get-LlvmTool -NdkRoot $ndkRoot -Name 'llvm-ar'
Write-Host "AR: $ar"

$failed = @()
$built = @()

foreach ($t in $Targets) {
    $linker = Get-ClangWrapper -NdkRoot $ndkRoot -RustTarget $t -Api $ApiLevel
    $envName = Get-LinkerEnvName -RustTarget $t
    Set-Item -Path "Env:$envName" -Value $linker
    Set-TargetCcEnv -RustTarget $t -Clang $linker -Ar $ar
    Write-Host "=== $t ==="
    Write-Host "  $envName = $linker"
    Write-Host "  CC_$t = $linker"

    $cargoArgs = @('build', '-p', $Crate, '--target', $t, '--profile', $Profile)
    if ($Lib) {
        $cargoArgs += '--lib'
    } elseif ($Example) {
        $cargoArgs += @('--example', $Example)
    }

    Write-Host ("  cargo " + ($cargoArgs -join ' '))
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAIL $t (exit $LASTEXITCODE)" -ForegroundColor Red
        $failed += $t
        continue
    }

    # Locate artefact. cargo puts examples under <profile>/examples/, libs under <profile>/.
    $profileDir = if ($Profile -eq 'dev') { 'debug' } else { $Profile }
    $outDir = Join-Path $raizRepo "target\$t\$profileDir"

    $artefacts = @()
    if ($Lib) {
        $artefacts = @(Get-ChildItem $outDir -Filter 'lib*.so' -ErrorAction SilentlyContinue)
        if (-not $artefacts -or $artefacts.Count -eq 0) {
            # Some crate names drop the lib- prefix mapping; also check exact names
            $artefacts = @(Get-ChildItem $outDir -Filter '*.so' -ErrorAction SilentlyContinue)
        }
    } elseif ($Example) {
        $examplesDir = Join-Path $outDir 'examples'
        $candidate = Join-Path $examplesDir $Example
        if (-not (Test-Path $candidate)) {
            $candidate = Join-Path $examplesDir ($Example -replace '-', '_')
        }
        # Fallback: profile root (older cargo layouts)
        if (-not (Test-Path $candidate)) {
            $candidate = Join-Path $outDir $Example
        }
        if (Test-Path $candidate) { $artefacts = @(Get-Item $candidate) }
    }

    # Rust triple → carpeta ABI de jniLibs (AGP)
    $abi = switch ($t) {
        'aarch64-linux-android' { 'arm64-v8a' }
        'armv7-linux-androideabi' { 'armeabi-v7a' }
        'x86_64-linux-android' { 'x86_64' }
        'i686-linux-android' { 'x86' }
        default { $null }
    }

    foreach ($a in $artefacts) {
        $mb = [math]::Round($a.Length / 1MB, 2)
        Write-Host ("  OK {0}  {1} MB  ({2} bytes)" -f $a.Name, $mb, $a.Length) -ForegroundColor Green
        $built += [pscustomobject]@{ Target = $t; Path = $a.FullName; Bytes = $a.Length; MB = $mb }

        # H3+: copiar .so a jniLibs del módulo Gradle :servidor (no versionado).
        if ($Lib -and $abi -and $a.Name -like 'libservidor_android.so') {
            $jniDir = Join-Path $raizRepo "client-android\servidor\src\main\jniLibs\$abi"
            New-Item -ItemType Directory -Force -Path $jniDir | Out-Null
            $dest = Join-Path $jniDir $a.Name
            Copy-Item -Force $a.FullName $dest
            Write-Host "  → jniLibs/$abi/$($a.Name)" -ForegroundColor Cyan
        }
    }
    if (-not $artefacts -or $artefacts.Count -eq 0) {
        Write-Host "  WARN: build OK pero no encontré artefacto en $outDir" -ForegroundColor Yellow
    }
    Write-Host ""
}

Write-Host "======== resumen ========"
$built | Format-Table -AutoSize | Out-String | Write-Host
if ($failed.Count -gt 0) {
    Write-Host ("FALLÓ: " + ($failed -join ', ')) -ForegroundColor Red
    exit 1
}
Write-Host "H1/build: los tres targets OK" -ForegroundColor Green
exit 0

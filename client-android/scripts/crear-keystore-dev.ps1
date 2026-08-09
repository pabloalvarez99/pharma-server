<#
.SYNOPSIS
    Crea un keystore de DESARROLLO para firmar builds de release y deja
    `client-android/keystore.properties` apuntando a el.

.DESCRIPTION
    Sirve para sideloadear un APK de release en un telefono real. NO es el
    keystore de publicacion: el de Play Store lo genera el capitan, se guarda
    donde se guardan las cosas que no se pueden perder, y no lo crea un script.

    Regla 3: el keystore se escribe FUERA del repo (por defecto en
    %LOCALAPPDATA%\RutBusiness\keys) y `keystore.properties` esta gitignorado.
    Ni uno ni otro entran a git, al vault ni a Notion.

    Si el keystore ya existe no lo pisa: perder un keystore es perder la
    capacidad de actualizar una app ya instalada.

.PARAMETER Clave
    Clave del keystore y del alias. Si se omite se genera una al azar y queda
    escrita solo en keystore.properties.

.PARAMETER Destino
    Carpeta donde vive el .jks. Por defecto %LOCALAPPDATA%\RutBusiness\keys.

.EXAMPLE
    pwsh client-android/scripts/crear-keystore-dev.ps1
#>
[CmdletBinding()]
param(
    [string]$Clave,
    [string]$Destino = (Join-Path $env:LOCALAPPDATA 'RutBusiness\keys'),
    [string]$Alias = 'rutbusiness-dev'
)

$ErrorActionPreference = 'Stop'

$raizAndroid = Split-Path -Parent $PSScriptRoot
$archivoPropiedades = Join-Path $raizAndroid 'keystore.properties'

# --- keytool ---------------------------------------------------------------
$keytool = (Get-Command keytool -ErrorAction SilentlyContinue).Source
if (-not $keytool -and $env:JAVA_HOME) {
    $candidato = Join-Path $env:JAVA_HOME 'bin\keytool.exe'
    if (Test-Path $candidato) { $keytool = $candidato }
}
if (-not $keytool) {
    $candidato = 'C:\Program Files\Android\Android Studio\jbr\bin\keytool.exe'
    if (Test-Path $candidato) { $keytool = $candidato }
}
if (-not $keytool) {
    throw "No se encontro keytool. Instala un JDK 17 o define JAVA_HOME."
}

# --- verificar que el destino este fuera del repo ---------------------------
if (-not (Test-Path $Destino)) {
    New-Item -ItemType Directory -Path $Destino -Force | Out-Null
}
$destinoAbs = (Resolve-Path $Destino).Path
$repo = (& git -C $raizAndroid rev-parse --show-toplevel 2>$null)
if ($repo) {
    $repoAbs = (Resolve-Path $repo).Path
    if ($destinoAbs.StartsWith($repoAbs, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Regla 3: el destino '$destinoAbs' esta DENTRO del repo '$repoAbs'. Elegi otra carpeta."
    }
}

$keystore = Join-Path $destinoAbs 'rutbusiness-dev.jks'

if (Test-Path $keystore) {
    Write-Host "Ya existe: $keystore (no se toca)."
} else {
    if (-not $Clave) {
        # 32 chars al azar. Nunca se imprime; queda solo en keystore.properties.
        $bytes = [byte[]]::new(24)
        [System.Security.Cryptography.RandomNumberGenerator]::Fill($bytes)
        $Clave = [Convert]::ToBase64String($bytes)
    }

    & $keytool -genkeypair `
        -keystore $keystore `
        -storetype PKCS12 `
        -storepass $Clave `
        -keypass $Clave `
        -alias $Alias `
        -keyalg RSA `
        -keysize 4096 `
        -validity 10950 `
        -dname 'CN=RutBusiness Dev, OU=Desarrollo, O=RutBusiness, C=CL'
    if ($LASTEXITCODE -ne 0) { throw "keytool fallo con codigo $LASTEXITCODE" }

    Write-Host "Keystore creado: $keystore"
}

# --- keystore.properties ---------------------------------------------------
if ((Test-Path $archivoPropiedades) -and -not $Clave) {
    Write-Host "keystore.properties ya existe y no hay clave nueva: no se toca."
} elseif (-not $Clave) {
    throw "El keystore ya existia y no se paso -Clave, asi que no se puede escribir keystore.properties. Pasa -Clave o borra el keystore."
} else {
    $rutaParaGradle = $keystore -replace '\\', '/'
    @(
        '# Generado por scripts/crear-keystore-dev.ps1. Gitignorado a proposito.',
        '# REGLA 3: no copiar estos valores a ningun archivo versionado, al vault ni a Notion.',
        "storeFile=$rutaParaGradle",
        "storePassword=$Clave",
        "keyAlias=$Alias",
        "keyPassword=$Clave"
    ) | Set-Content -Path $archivoPropiedades -Encoding UTF8
    Write-Host "Escrito: $archivoPropiedades"
}

# --- red de seguridad: que git no lo vea -----------------------------------
$ignorado = & git -C $raizAndroid check-ignore -q 'keystore.properties'; $codigo = $LASTEXITCODE
if ($codigo -ne 0) {
    throw "keystore.properties NO esta gitignorado. Arregla .gitignore antes de seguir."
}

Write-Host ''
Write-Host 'Listo. Ahora:  ./gradlew assembleRelease bundleRelease'
Write-Host 'La clave vive solo en keystore.properties. No la copies a ningun lado.'

<#
.SYNOPSIS
Regenera el cliente Kotlin desde el OpenAPI del server.

.DESCRIPTION
El cliente de `core/src/commonMain/kotlin/cl/rutbusiness/core/api/` NO se edita
a mano: se genera desde `/docs/openapi.json`. Este script baja el spec del
server corriendo, corre openapi-generator y reemplaza el directorio completo.

Cualquier cambio hecho a mano ahí se pierde en la próxima corrida. Si algo del
cliente está mal, se arregla el `#[utoipa::path]` en `crates/api/` y se
regenera.

.PARAMETER BaseUrl
Server del que se baja el spec. Tiene que estar corriendo con `docs.enabled`.

.EXAMPLE
pwsh scripts/generate-api-client.ps1
pwsh scripts/generate-api-client.ps1 -BaseUrl http://192.168.1.10:8080
#>
[CmdletBinding()]
param(
    [string]$BaseUrl = "http://127.0.0.1:8080",
    [string]$GeneratorVersion = "7.24.0"
)

$ErrorActionPreference = "Stop"

$raiz = Split-Path -Parent $PSScriptRoot
$destino = Join-Path $raiz "core/src/commonMain/kotlin/cl/rutbusiness/core/api"
$trabajo = Join-Path ([System.IO.Path]::GetTempPath()) "rutbusiness-openapi"
$jar = Join-Path $trabajo "openapi-generator-cli-$GeneratorVersion.jar"
$spec = Join-Path $trabajo "openapi.json"

New-Item -ItemType Directory -Force -Path $trabajo | Out-Null

Write-Host "1/4 Bajando el spec de $BaseUrl/docs/openapi.json"
Invoke-WebRequest -Uri "$BaseUrl/docs/openapi.json" -OutFile $spec -UseBasicParsing -TimeoutSec 60

if (-not (Test-Path $jar)) {
    Write-Host "2/4 Bajando openapi-generator $GeneratorVersion (una sola vez)"
    $url = "https://repo.maven.apache.org/maven2/org/openapitools/openapi-generator-cli/$GeneratorVersion/openapi-generator-cli-$GeneratorVersion.jar"
    Invoke-WebRequest -Uri $url -OutFile $jar -UseBasicParsing -TimeoutSec 600
} else {
    Write-Host "2/4 openapi-generator ya está en cache"
}

$salida = Join-Path $trabajo "out"
if (Test-Path $salida) { Remove-Item $salida -Recurse -Force }

Write-Host "3/4 Generando"
# --skip-validate-spec: el spec del server tiene hoy dos defectos que el
#   validador rechaza y que se arreglan en `crates/api/src/openapi.rs`, no acá:
#     1. `operationId` "get_product" repetido en GET /api/v1/products/{id} y en
#        GET /api/v1/public/{slug}/catalog/{product_slug}.
#     2. `info.license` sin `identifier` ni `url` (OpenAPI 3.1 exige uno).
# --type-mappings AnyType=JsonElement: los bodies que el spec declara `object`
#   opaco llegarían como `kotlin.Any`, que kotlinx.serialization no sabe
#   serializar. `JsonElement` sí, y no pierde nada.
& java -jar $jar generate `
    -i $spec `
    -g kotlin `
    --library multiplatform `
    --skip-validate-spec `
    -o $salida `
    --additional-properties="packageName=cl.rutbusiness.core.api,dateLibrary=string" `
    --type-mappings "AnyType=JsonElement" `
    --import-mappings "JsonElement=kotlinx.serialization.json.JsonElement" | Out-Null

if ($LASTEXITCODE -ne 0) { throw "openapi-generator falló con código $LASTEXITCODE" }

$generado = Join-Path $salida "src/commonMain/kotlin/cl/rutbusiness/core/api"
if (-not (Test-Path $generado)) { throw "no se generó $generado" }

Write-Host "4/4 Reemplazando $destino"
if (Test-Path $destino) { Remove-Item $destino -Recurse -Force }
Copy-Item $generado -Destination (Split-Path -Parent $destino) -Recurse -Force

$archivos = (Get-ChildItem $destino -Recurse -File).Count
Write-Host "Listo: $archivos archivos. Revisa el diff antes de commitear."

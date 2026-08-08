# client-android

App Android nativa en Kotlin + Jetpack Compose. Habla HTTP contra `pharma-api`,
igual que la PWA. Decisión: [ADR-0021](../docs/adr/0021-android-compose-nativo.md).

`client/` (TypeScript) **sigue vivo** y no se toca: de ahí salen la PWA web y el
MSI de escritorio.

## Correr

```powershell
# 1. El server, desde la raíz del repo
$env:PHARMA_ALLOW_INSECURE_JWT = "1"   # solo desarrollo local
cargo run -p api

# 2. La app
cd client-android
./gradlew assembleDebug
adb install -r app/build/outputs/apk/debug/app-x86_64-debug.apk
```

No hay APK universal: `assembleDebug` produce un APK por ABI. Instala el que
corresponda al aparato (`adb shell getprop ro.product.cpu.abi`).

En el emulador el server del PC es `http://10.0.2.2:8080`. En un teléfono
físico, la IP del PC en la red (`http://192.168.x.x:8080`).

La primera cuenta se crea con `POST /api/v1/setup` (`business_name`,
`tenant_slug`, `email`, `password`). Elige tu propia contraseña; no hay ninguna
por defecto ni queda escrita en el repo.

## Cómo está armado

```
core/    Kotlin Multiplataforma. Red, modelos, sesión, errores.
         `commonMain` no sabe nada de Android; lo de plataforma va en
         `androidMain` detrás de `expect/actual`.
app/     Aplicación Android. Compose, ViewModels, y el único `Activity`.
```

Tres cosas son `expect/actual`: el motor HTTP (`defaultHttpClientEngine`), el
almacenamiento persistente (`AlmacenamientoPlataforma`) y el monitor de red
(`MonitorDeRed`). El día que se sume iOS, se agrega `iosArm64()` en
`core/build.gradle.kts` y se escriben esos tres `actual` para Darwin, Keychain y
`NWPathMonitor`. Nada más de la capa de red cambia.

**Regla que sostiene eso**: ningún composable importa `android.*`.
`MainActivity` es la única frontera, y lo de plataforma que la app necesita vive
en paquetes propios fuera de `ui/` (`cl.rutbusiness.app.camara` para CameraX,
`cl.rutbusiness.app.impresion` para el Bluetooth). `FronteraDePlataformaTest`
rompe el build si un `import android.` se cuela dentro de `ui/`.

## Sin señal

El local tiene wifi que se cae y datos móviles que van y vienen, así que la app
no asume que el server contesta. Todo eso vive en `core/.../offline/`:

| Qué | Dónde |
|---|---|
| ¿Estamos llegando al sistema del negocio? | `ConexionConElNegocio` (enlace del SO **+** cómo le fue a la última llamada) |
| Lo último que contestó el server | `CacheDeLecturas` (archivos sueltos, no DataStore: no quedan residentes) |
| Ventas cobradas que todavía no llegaron | `ColaDeVentas` (disco, escritura atómica) |
| Reintento solo, con backoff | `DespachadorDeVentas` |

**Lo que no se pierde es la venta.** Se escribe a disco **antes** del primer
intento, con la misma clave de idempotencia que usa el cobro normal, y el
reintento sale con esa misma clave: el server contesta 200 con la orden que ya
creó en vez de cobrar de nuevo. La dueña ve la cola desde la franja de arriba.

Sin señal **se puede** mirar lo cacheado, armar el carrito y cobrar en efectivo.
**No se puede** fiar, cobrar por transferencia ni tocar la caja, y cada una lo
dice antes de que la dueña lo intente. Ningún monto se calcula en el teléfono:
donde no hay número del server, la pantalla dice "se confirma al enviarse".

## El cliente de la API se genera

`core/src/commonMain/kotlin/cl/rutbusiness/core/api/` sale de
`/docs/openapi.json` vía openapi-generator (Ktor + kotlinx.serialization). **No
se edita a mano.** Para regenerarlo, con el server corriendo:

```powershell
pwsh scripts/generate-api-client.ps1
```

Dos endpoints quedan escritos a mano en `core/.../session/AuthApi.kt`:
`POST /api/v1/login` y `GET /api/v1/me`. No están anotados con
`#[utoipa::path]` en `crates/api/src/routes.rs`, así que no aparecen en el spec.
Cuando se anoten, ese archivo se borra.

## Piso de hardware

El aparato objetivo tiene 1-2 GB de RAM y puede ser un Android 5. Ver el bloque
"PISO DE HARDWARE" en el [`CLAUDE.md`](../CLAUDE.md) de la raíz. Lo que eso
impone acá:

| Regla | Dónde vive |
|---|---|
| `minSdk 21` | `app/build.gradle.kts`, y fija las versiones de `gradle/libs.versions.toml` |
| Nunca APK universal | `splits.abi` con `isUniversalApk = false` |
| Baseline Profile | `app/src/main/baseline-prof.txt` + `androidx.profileinstaller` |
| Virtualizar listas | `LazyColumn`, nunca `Column` con scroll sobre datos del server |
| Objetivos táctiles ≥ 56 dp | los botones de acción, no los 48 dp de Material |

`minSdk 21` es lo que fija las versiones de todas las librerías: AndroidX dejó
de soportar API 21/22 a mitad de 2025. El detalle está comentado arriba de
`gradle/libs.versions.toml`.

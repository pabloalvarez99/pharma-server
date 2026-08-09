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
físico, la IP del PC en la red (`http://192.168.x.x:8080`) — ver
[Instalar en un teléfono de verdad](#instalar-en-un-teléfono-de-verdad).

La primera cuenta se crea con `POST /api/v1/setup` (`business_name`,
`tenant_slug`, `email`, `password`). Elige tu propia contraseña; no hay ninguna
por defecto ni queda escrita en el repo.

## Firma y versión

### Firma de release

El keystore y sus claves viven **fuera del repo**. La firma se lee, en este
orden, de `keystore.properties` (gitignorado) o de las variables de entorno
`RB_KEYSTORE_FILE`, `RB_KEYSTORE_PASSWORD`, `RB_KEY_ALIAS` y `RB_KEY_PASSWORD`.

Si no encuentra ninguna, el build de release **falla** y dice qué falta. Nunca
cae a la firma de debug: un APK firmado con la clave de debug se instala igual y
parece sano, pero después no se puede actualizar con el APK de verdad porque la
firma no coincide — y eso se descubre con la app ya instalada en el teléfono de
la dueña.

Para armar un keystore de **desarrollo** (sirve para sideloadear, no para
publicar):

```powershell
pwsh client-android/scripts/crear-keystore-dev.ps1
```

Lo escribe en `%LOCALAPPDATA%\RutBusiness\keys\` y deja `keystore.properties`
apuntando ahí. Si el destino cae dentro del repo, el script se niega.

El keystore de **publicación** es otra cosa: lo genera el capitán, se guarda
donde se guardan las cosas que no se pueden perder, y no lo crea un script.
Perderlo es perder para siempre la capacidad de actualizar la app publicada.

> **Regla 3.** Ni el keystore ni su clave entran a git, al vault ni a Notion. Se
> archiva el puntero — dónde está y cómo se obtiene — nunca el valor.
> `keystore.properties.ejemplo` es la plantilla, sin valores.

### Versión

| Qué | De dónde sale |
|---|---|
| `versionName` | `version.properties`, a mano, cuando cambia lo que la app hace |
| `versionCode` | cantidad de commits de `HEAD` (`git rev-list --count`) |

Play Store rechaza para siempre un `versionCode` repetido y no deja bajarlo.
Atarlo al historial lo hace subir solo, y dos builds del mismo commit dan el
mismo número. `RB_VERSION_CODE=<entero>` lo fuerza — hace falta si algún día un
CI clona con `depth=1`, porque ahí contar commits da 1.

Lo que se ve en Ajustes del teléfono es `0.1.0 (<versionCode>)`: el número entre
paréntesis es lo único que distingue dos APK sideloadeados de commits distintos.

### Construir el release

```powershell
cd client-android
./gradlew assembleRelease   # APK por ABI, para sideload
./gradlew bundleRelease     # AAB, lo que pide Play Store
```

**Dos corridas separadas, a propósito.** AGP no acepta `splits.abi` junto con el
bundle, y apagar los splits produciría un APK universal — prohibido por el piso
de hardware. Pedir las dos cosas en la misma corrida falla con ese mensaje.

Salidas:

```
app/build/outputs/apk/release/app-<abi>-release.apk
app/build/outputs/bundle/release/app-release.aab
```

## Instalar en un teléfono de verdad

Lo que sigue se hace una vez, con el teléfono en la mano y el PC del negocio
prendido en la misma red.

### 1. El APK que corresponde

```powershell
adb shell getprop ro.product.cpu.abi
```

Casi todo teléfono Android de los últimos diez años contesta `arm64-v8a`; los
muy viejos o muy baratos, `armeabi-v7a`. Los `x86*` son de emulador. Instala el
APK de ese ABI y ningún otro: no existe uno universal.

### 2. Instalar

Con cable, si el teléfono tiene **Depuración por USB** prendida (Ajustes >
Opciones de programador):

```powershell
adb install -r client-android/app/build/outputs/apk/release/app-arm64-v8a-release.apk
```

Sin cable: copia el `.apk` al teléfono (correo, WhatsApp, pendrive, carpeta
compartida), ábrelo desde Archivos y acepta **Instalar apps desconocidas** para
la app desde la que lo abriste. Android va a advertir; es lo esperado en una app
que no viene de Play Store.

Si dice "aplicación no instalada", casi siempre es que ya hay una versión
instalada firmada con otra clave (una de debug, por ejemplo). Se desinstala y se
vuelve a instalar:

```powershell
adb uninstall cl.rutbusiness.app
```

**Desinstalar borra los datos locales de la app**: la sesión, lo cacheado y —
esto importa — las ventas encoladas que todavía no llegaron al server. Antes de
desinstalar, mira la franja de arriba de la app y espera a que la cola esté
vacía.

### 3. Que el teléfono llegue al server

El server escucha en `0.0.0.0:8080` desde `config/default.toml`, así que ya
acepta conexiones de la red y no sólo del propio PC. No hay nada que cambiar
salvo que alguien lo haya puesto en `127.0.0.1`, que sólo se ve desde el PC.

En el PC del negocio:

```powershell
# La IP en la red. La que empieza en 192.168 o en 10.
ipconfig | Select-String IPv4

# Dejar pasar el puerto por el firewall de Windows (una vez, como admin)
New-NetFirewallRule -DisplayName "RutBusiness API" -Direction Inbound `
  -Protocol TCP -LocalPort 8080 -Action Allow -Profile Private
```

`-Profile Private` a propósito: el puerto se abre en la red del local, no en la
del café donde el PC se conectó una vez.

En el teléfono, **conectado al mismo wifi**, abre el navegador en
`http://192.168.x.x:8080/health/ready`. Si contesta un JSON, el camino está.
Si no contesta, el problema es de red y se arregla ahí, no en la app: teléfono
en otra red (datos móviles en vez de wifi), firewall, o el server apagado.

### 4. Apuntar la app

Al abrirla, en "¿Dónde está el computador del negocio?", escribe la IP con el
puerto, **sin `http://`**:

```
192.168.1.10:8080
```

"Probar la dirección" contesta antes de pedir la clave, y distingue tres casos:
que no conteste nadie, que conteste otra cosa, y que conteste el sistema.

La IP del PC puede cambiar sola cuando el router se reinicia. Si un día la app
deja de conectar sin que nadie haya tocado nada, es eso: hay que reservarle la
IP al PC en el router, o volver a escribir la nueva.

### 5. Impresora y cámara

- **Impresora**: se empareja desde los ajustes Bluetooth del teléfono, no desde
  la app — la app sólo lee las que ya están emparejadas y nunca escanea.
- **Cámara**: el permiso se pide la primera vez que se toca "Escanear".

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
| `minSdk 23` | `app/build.gradle.kts`, y fija las versiones de `gradle/libs.versions.toml` |
| Nunca APK universal | `splits.abi` con `isUniversalApk = false` |
| Firma v1 además de v2 | Android 6 no entiende el esquema v2, que llegó con Android 7 |
| Baseline Profile | `app/src/main/baseline-prof.txt` + `androidx.profileinstaller` |
| Virtualizar listas | `LazyColumn`, nunca `Column` con scroll sobre datos del server |
| Objetivos táctiles ≥ 56 dp | los botones de acción, no los 48 dp de Material |

`minSdk 23` es lo que fija las versiones de todas las librerías: AndroidX dejó
de soportar API 21/22 a mitad de 2025. El detalle está comentado arriba de
`gradle/libs.versions.toml` y en `app/build.gradle.kts`.

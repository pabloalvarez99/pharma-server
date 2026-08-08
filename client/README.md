# RutBusiness Client

Frontend único (TS vanilla + Vite) con dos targets:

- **Desktop (Tauri 2)** — `npm run dev` / `npm run build` (+ `npm run tauri dev|build`).
  `invoke` va por IPC a los 73 comandos Rust de `src-tauri/src/commands/*`.
- **Web / PWA (SP3, ADR-0015 P2)** — `npm run dev:web` / `npm run build:web`.
  El MISMO frontend corriendo en browser: Vite en `--mode web` aliasea
  `@tauri-apps/api/core` → `src/web-transport/` (shim `invoke`→`fetch` que
  replica 1:1 los 73 comandos, mismos errores en español) y
  `@tauri-apps/plugin-updater` → stub no-op. Cero cambios en las 18 vistas ni
  en `src/api/*`.

## Web build (PWA)

```powershell
npm run build:web            # tsc + vite build --mode web → dist-web/
npm run dev:web              # dev server modo web (shim activo)
npx vite preview --outDir dist-web   # probar el build localmente
```

- **Server por defecto**: `https://api.rutbusiness.cl`, inyectable en build con
  `VITE_DEFAULT_SERVER_URL` (mientras el dominio no exista:
  `https://136.67.83.70.nip.io`, la VM `pharma-prod`). El shim siembra
  `localStorage["pharma:last-server"]` solo si está vacío; el operador puede
  cambiar la URL en el login igual que en desktop.
- **Token**: en memoria + `sessionStorage` (nunca `localStorage`); F5 mantiene la
  sesión, cerrar la pestaña la bota.
- **PWA**: `public/manifest.webmanifest` + `public/sw.js` (service worker
  shell-only: cachea solo assets estáticos del mismo origen; los datos SIEMPRE
  van a la red). Íconos en `public/icons/` (copiados de `src-tauri/icons/`).
- **Desktop-only**: `print_ticket` / `open_cash_drawer` (ESC/POS) y el updater
  degradan con el error controlado `"Disponible en la app de escritorio"`; las
  vistas ya hacen fallback (POS imprime con `window.print()`).

## Deploy web

Estático puro (`dist-web/`). Dos opciones:

1. **Vercel**:
   ```powershell
   $env:VITE_DEFAULT_SERVER_URL = "https://136.67.83.70.nip.io"  # o api.rutbusiness.cl cuando exista
   npm run build:web
   npx vercel deploy dist-web --prod
   ```
2. **Caddy de SP1** (misma VM): servir `dist-web/` como site estático junto al
   reverse-proxy de `pharma-api` (ver `docs/product/saas-web-cloud-ops.md`).

**CORS (obligatorio)**: el server bloquea cross-origin por defecto. En la VM,
agregar el origen del deploy en `config/local.toml` del working dir de
`pharma-api` (las listas NO se pueden setear por env `PHARMA__*`):

```toml
[cors]
allowed_origins = ["https://<deploy>.vercel.app"]
```

y reiniciar el servicio.

## Móvil nativo (Android / iOS)

Tercer target del MISMO frontend (meta founder: el celular es el dispositivo
primario del usuario real). Tauri 2 compila las 18 vistas a app nativa; **no hay
código de UI aparte** y el shim `src/web-transport/` NO se usa acá — en móvil el
`invoke` real va por IPC a los mismos comandos Rust que en desktop.

### Prerequisitos (Windows, Android)

| Pieza | Versión verificada | Cómo |
|---|---|---|
| JDK | Temurin 17.0.19 / MS OpenJDK 17.0.20 | `JAVA_HOME` debe apuntar al JDK |
| **MSVC Build Tools** | VS 2022 BuildTools 17.14 + workload VCTools | ver "El linker MSVC" más abajo — sin esto no compila **nada** |
| Android SDK cmdline-tools | latest | `sdkmanager` en `$ANDROID_HOME/cmdline-tools/latest/bin` |
| platform + build-tools | android-34 / 34.0.0 | `sdkmanager "platforms;android-34" "build-tools;34.0.0"` |
| **NDK** | 27.2.12479018 | `sdkmanager "ndk;27.2.12479018"` — sin esto `tauri android init` falla |
| emulator + system image | android-34 google_apis x86_64 | `sdkmanager "emulator" "system-images;android-34;google_apis;x86_64"` |
| targets Rust | 4 ABIs | `rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android` |

Variables de entorno (permanentes, `setx` o Panel de control):

```powershell
# Si el SDK vino con Android Studio el path real es AppData\Local, no ~\Android
setx ANDROID_HOME "C:\Users\<user>\AppData\Local\Android\Sdk"
setx NDK_HOME     "C:\Users\<user>\AppData\Local\Android\Sdk\ndk\27.2.12479018"
setx JAVA_HOME    "C:\Program Files\Eclipse Adoptium\jdk-17.0.19.10-hotspot"
```

`tauri android init` aborta con `failed to ensure Android environment: Skipping
Android Studio command line tools installation` cuando **falta `NDK_HOME`** — el
mensaje habla de cmdline-tools pero la causa real es el NDK.

### El linker MSVC (falla que no menciona Android)

Cross-compilar a Android igual compila los **build scripts y proc-macros para el
host**, y el host es `x86_64-pc-windows-msvc`. Sin MSVC el build muere con:

```
error: linker `link.exe` not found
```

Instalar (también lo necesita `cargo run -p api`, así que no es deuda del carril
móvil):

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools `
  --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

`where link.exe` **nunca** responde: `link.exe` sólo entra al PATH dentro de un
shell de `vcvars64.bat`. No es síntoma de nada — rustc lo resuelve solo vía
vswhere. Verificar así en su lugar:

```powershell
"fn main(){}" | Set-Content probe.rs; rustc probe.rs   # linkea = MSVC OK
```

### `cargo` directo al target Android no funciona

`cargo build --target x86_64-linux-android` falla con
`failed to find tool "x86_64-linux-android-clang"`: quien inyecta CC/AR/linker
del NDK es el wrapper. Siempre `npm run android:build` / `android:dev`, nunca
`cargo` a mano.

### El APK sale sin firmar

`tauri android build --apk` deja `app-universal-release-unsigned.apk`, y
`adb install` lo rechaza. Para probar en emulador, firmar con el debug keystore:

```powershell
$bt = "$env:ANDROID_HOME\build-tools\34.0.0"
& "$bt\zipalign.exe" -p -f 4 app-universal-release-unsigned.apk aligned.apk
& "$bt\apksigner.bat" sign --ks "$env:USERPROFILE\.android\debug.keystore" `
  --ks-pass pass:android --ks-key-alias androiddebugkey --key-pass pass:android `
  --out rutbusiness-signed.apk aligned.apk
```

Para Play Store va keystore propio, no el de debug.

### Correr en emulador

```powershell
# 1. AVD (una vez)
avdmanager create avd -n rutbusiness -k "system-images;android-34;google_apis;x86_64" -d pixel_6
# 2. scaffolding Android (una vez; genera src-tauri/gen/android, se commitea)
npm run android:init
# 3. arrancar emulador + app (dev server con HMR)
emulator -avd rutbusiness
npm run android:dev
```

`npm run android:build` corta el APK (`--apk`; usar `--aab` para Play Store).

`npm run android:build -- --target x86_64` corta un solo ABI (el del emulador) y
tarda la mitad; sin `--target` compila los 4.

Verificado 2026-08-06 (AVD `rutbusiness`, android-34 google_apis x86_64): APK
x86_64 de **17.5 MB** en **10m35s** en frío; login contra `10.0.2.2:8080` y POS
listando el seed `minimarket`.

**Aceleración por hardware (Windows)**: el emulador x86_64 necesita **WHPX**
(feature `HypervisorPlatform`). Si en la máquina está Hyper-V activo pero WHPX
apagado, el emulador no arranca. Chequeo rápido, sin elevación:

```powershell
& "$env:ANDROID_HOME\emulator\emulator-check.exe" accel   # "WHPX … installed and usable"
```

Si no está, activarlo pide consola elevada y **reboot**:

```powershell
Enable-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform -All
```

Con Hyper-V encendido NO sirve AEHD/HAXM — WHPX es el único acelerador válido.

### Server: `10.0.2.2`, no `localhost`

Dentro del emulador `localhost` es **el emulador mismo**, no el PC. El loopback
del host se ve como **`10.0.2.2`**. Por lo tanto en la pantalla de login hay que
escribir:

```
http://10.0.2.2:8095        # server local del PC en el puerto 8095
```

El botón **Probar conexión** de la pantalla de login confirma la ruta antes de
tipear credenciales: verde "Servidor accesible" = el `10.0.2.2` resuelve.

Levantar el server para esta prueba (desde la raíz del worktree):

```powershell
$env:PHARMA_ALLOW_INSECURE_JWT = "1"   # dev: el secreto JWT es el placeholder
$env:PHARMA__DB__PATH = "<worktree>\data\surreal"
cargo run -p api
```

`PHARMA__DB__PATH` **absoluto no es opcional**: con path relativo el server lo
ancla a `C:\ProgramData\PharmaServer\data\surreal` (dir de instalación) y abre
una base vacía distinta de la que sembró el CLI — el login da "Credenciales
inválidas" con el usuario recién creado. El log lo dice:
`db path anchored to install data dir`.

Datos y usuario para la prueba:

```powershell
cargo run -p cli -- migrate
cargo run -p cli -- tenant-create "Principal" --slug principal
cargo run -p cli -- seed-demo --tenant principal --vertical minimarket
$env:PHARMA_PASSWORD = "<pass>"        # nunca commitear el valor
cargo run -p cli -- user-create --tenant principal --email pos@principal.cl --roles owner
```

El campo **SUCURSAL** de la pantalla de login es el slug del tenant (default
`principal`). Crear los usuarios **antes** de arrancar el server: el CLI y el
server abren handles separados de la misma base embebida y el server no ve
escrituras posteriores a su arranque.

`localhost:8095` da "no se pudo conectar". En **teléfono físico** en la misma
WiFi, la URL es la IP LAN del server (ej. `http://192.168.1.20:8095`) y el
server debe escuchar en `0.0.0.0`, no en `127.0.0.1`. La URL la tipea el
operador y queda en `localStorage["pharma:last-server"]` — no está hardcodeada
(cada comando Rust recibe `server_url` como parámetro).

El Vite dev server sí se resuelve solo: `tauri android dev` hace `adb reverse`
del puerto 5173, y `vite.config.ts` ya respeta `TAURI_DEV_HOST` para el caso de
dispositivo físico (HMR por WS en el 5183).

### Diseño en pantalla de teléfono (deuda conocida)

Las 18 vistas se diseñaron para desktop (ventana 1100×720, layout de tablas +
atajos de teclado, `app.windows` de `tauri.conf.json`). En móvil compilan y
corren, pero:

- las tablas anchas (inventario, compras, libro de compras, auditoría) piden
  scroll horizontal;
- el POS y la barra de comandos asumen teclado físico (`Ctrl+K`, F-keys);
- no hay targets táctiles de 44px ni layout de una mano.

Adaptar eso es trabajo aparte y **explícitamente NO parte del init móvil** — la
meta acá era que la misma UI compile y haga login. El principio #7 de la vara UX
(accesible + táctil) es lo que cierra esa deuda.

### iOS

**Requiere macOS + Xcode** — no se puede desde Windows ni Linux (el toolchain de
Apple no cross-compila). En un Mac:

```bash
xcode-select --install
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
npm run ios:init && npm run ios:dev
```

Falta además un `developmentTeam` (Apple Developer, USD 99/año) para firmar en
dispositivo físico; el simulador no lo pide. Estado hoy: **no iniciado**.

### Notas de código

- `tauri-plugin-updater` es **desktop-only** (su metadata declara android/ios
  `level = "none"`): la dependencia está gateada por target en
  `src-tauri/Cargo.toml` y su registro con `#[cfg(desktop)]` en `lib.rs`. En
  móvil el canal de update es la store. `src/updater.ts` se traga el error, así
  que la UI no se rompe.
- `print_ticket` / `open_cash_drawer` (ESC/POS por spooler Windows) ya están
  gateados `cfg(windows)`; en Android devuelven el error controlado en español.
- `src-tauri/gen/android` **se commitea** (es scaffolding, no build output);
  `.gitignore` sólo excluye `build/`, `.gradle/` y `local.properties`.

## Gate

`npm run gate` = build desktop + vitest (incluye unit del shim
`src/web-transport/shim.test.ts`) + `build:web` + e2e.

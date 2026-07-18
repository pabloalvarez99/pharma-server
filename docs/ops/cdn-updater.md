# Runbook: CDN updater (RutBusiness client)

Cómo firmar y preparar el manifest de auto-update del cliente Tauri 2.
**No sube nada al CDN real** — solo documenta el layout y el script de staging
local. La publicación a `cdn.pharma-server.cl` es un paso humano aparte.

## Endpoint (ya cableado en el cliente)

`client/src-tauri/tauri.conf.json` → `plugins.updater`:

```
https://cdn.pharma-server.cl/updates/rutbusiness/{{target}}-{{arch}}/{{current_version}}
```

Tauri sustituye `{{target}}` / `{{arch}}` / `{{current_version}}` en runtime
(ej. `windows-x86_64` + versión del binario instalado). El GET debe devolver
JSON del update (o 204/404 si no hay update). Fallos de red/CDN son
**silenciosos** en `client/src/updater.ts` (nunca bloquean el login).

Con `bundle.createUpdaterArtifacts: true`, `tauri build` emite el instalador
**y** los artefactos de firma del updater (`.sig` + payload zip/msi según
target).

## Claves de firma (minisign / Tauri)

| Archivo | Rol | Git |
|---|---|---|
| `client/keys/rutbusiness-updater.key` | Privada (sin password) | **gitignore** (`client/.gitignore` → `keys/`) |
| `client/keys/rutbusiness-updater.key.pub` | Pública | gitignore junto con keys/ |
| `plugins.updater.pubkey` en `tauri.conf.json` | Pública embebida (base64) | **sí** se commitea |

⚠️ La privada es la **única** copia de firma de updates. Respaldo fuera del
repo. **Nunca** commitear `.key` ni pegarla en issues/CI logs.

Password vacío a propósito (máquina de release controlada):

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw "client\keys\rutbusiness-updater.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
```

Sin esas variables, `tauri build` con `createUpdaterArtifacts: true` **falla**
al firmar (o no produce `.sig` usable).

## Build firmado (local)

Desde la raíz del worktree/repo (mismo layout que `installer/client/build-client.ps1`):

```powershell
cd "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\assist-b2"

# sccache roto en esta máquina — siempre vacío
$env:RUSTC_WRAPPER = ""

$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw "client\keys\rutbusiness-updater.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""

# Opción A: script de build existente (MSI + NSIS + updater artifacts)
pwsh installer\client\build-client.ps1

# Opción B: solo cliente
cd client
npx tauri build
```

Salida típica (repo-root `target/` por `.cargo/config.toml`):

- `target/release/bundle/msi/*.msi` (+ `.sig` si Tauri emite junto al MSI)
- `target/release/bundle/nsis/*-setup.exe` (+ `.sig`)
- Artefactos updater: a menudo `*.nsis.zip` / `*.msi.zip` + `.sig` en el mismo
  árbol de bundle (nombre exacto depende de la versión de Tauri CLI).

## Staging local (sin subir)

```powershell
pwsh scripts\publish-updater-artifacts.ps1
# → escribe dist-updater/ con layout listo para copiar al CDN
# → imprime el latest.json y el comando de upload sugerido (NO lo ejecuta)
```

El script **solo** copia/firma-layout en disco. Flags de upload real no
existen a propósito.

## Layout CDN objetivo

Raíz lógica del producto:

```
https://cdn.pharma-server.cl/uploads/...   # no
https://cdn.pharma-server.cl/updates/rutbusiness/
  windows-x86_64/
    latest.json                 # o un JSON por current_version (ver abajo)
    0.1.1/
      RutBusiness_0.1.1_x64-setup.nsis.zip
      RutBusiness_0.1.1_x64-setup.nsis.zip.sig
```

### ¿Un JSON por versión o un solo `latest.json`?

El endpoint del cliente incluye `{{current_version}}`. Opciones operativas:

1. **Archivo por versión** (simple con hosting estático):
   path `.../windows-x86_64/0.1.0` → cuerpo JSON que apunta a **0.1.1**
   (la versión *siguiente*). Cada release publica un JSON nuevo en la path de
   la versión *instalada* que quieres actualizar.
2. **Rewrite / edge function** que ignore el path y devuelva siempre el
   manifest de la última release (misma forma JSON).

Para el piloto, (1) basta: al publicar `0.1.1`, copias el mismo
`latest.json` a `.../windows-x86_64/0.1.0` (y a cualquier otra versión vieja
que aún deba recibir el update).

### Forma de `latest.json` (Tauri 2)

```json
{
  "version": "0.1.1",
  "notes": "Fix impresora térmica + cajón opcional",
  "pub_date": "2026-07-18T15:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<contenido del archivo .sig, una sola línea>",
      "url": "https://cdn.pharma-server.cl/updates/rutbusiness/windows-x86_64/0.1.1/RutBusiness_0.1.1_x64-setup.nsis.zip"
    }
  }
}
```

- `signature`: texto del `.sig` generado con la privada
  (`TAURI_SIGNING_PRIVATE_KEY`), **no** Authenticode del MSI.
- `url`: HTTPS público del zip/msi de update (el plugin descarga y verifica
  la firma minisign antes de instalar).
- `pubkey` en el cliente debe coincidir con la clave que firmó el `.sig`.

## Checklist de release

1. Bump `version` en `client/src-tauri/tauri.conf.json` (y `package.json` si
   se mantiene al día).
2. Cargar privada + password vacío (arriba).
3. `RUSTC_WRAPPER=""`, build release.
4. `pwsh scripts/publish-updater-artifacts.ps1` → revisar `dist-updater/`.
5. Subir a CDN **manualmente** (S3/R2/rsync) el zip + `.sig` + JSONs de
   versiones anteriores que deban migrar.
6. En un cliente instalado con la versión vieja: abrir app; el check silencioso
   debe descargar e instalar (installMode `passive` → reinicio manual).
7. **Nunca** rotar la privada sin actualizar `plugins.updater.pubkey` y
   redistribuir un instalador “semilla” firmado con la nueva clave.

## Qué no hacer

- Subir `rutbusiness-updater.key` al CDN, CI secrets públicos, o el repo.
- Publicar instaladores **sin** `.sig` del updater (el cliente los rechaza).
- Confundir la firma **minisign del updater** con la firma **Authenticode**
  del MSI (`installer/sign/pilot.pfx`) — son dos capas distintas.
- Depender del updater en red LAN offline: el fall-silent es by design; el
  cajero re-instala desde USB/mirror si hace falta.

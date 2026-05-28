# Coquimbo Go-Live Playbook — pharma-server

Operator runbook for the first real production install of `pharma-server` at the
Coquimbo pharmacy. Audience: the operator (likely the founder) at install time,
on the pharmacy's Windows host, with admin rights.

Branch in flight: `release/tufarmacia-golive`. Pinned MSI release: **v0.1.23**
(see "Known limitations" — newer versions CI-blocked).

> **Invariantes irrenunciables (ADR-0005).** Core ERP siempre opera offline. La
> license es opcional — sin license, el server arranca en tier Free y todo el
> POS/inventario/caja funciona. Nada en este playbook puede romper esa promesa.

---

## 1. Pre-install checklist

Antes de tocar la máquina, validar lo siguiente. Todo es bloqueante.

- **Windows**: Windows 10 22H2 o Windows 11 (x64). Windows Server 2019+ OK.
- **Hardware mínimo** (per `CLAUDE.md`): Intel i3 / Ryzen 3 o mejor, **SSD**
  (NVMe ideal — SurrealKv hace muchos fsync), **8 GB RAM**. POS budget: <50 ms
  p99 endpoints. HDD mecánico = no instalar.
- **Disco**: 20 GB libres mínimos para datos + backups + logs (`%ProgramData%\PharmaServer\`).
- **Red LAN**: el server escucha en `0.0.0.0:8080` y el firewall MSI abre TCP
  8080 perfil `all` (`installer/wix/main.wxs` → `fire:FirewallException`). Las
  cajas POS deben estar en la misma LAN. **No exponer 8080 a internet.**
- **Permisos admin**: `ServiceInstall` + `FirewallException` requieren UAC
  elevado. Iniciar el MSI con "Ejecutar como administrador".
- **Hora del sistema**: NTP sincronizado. JWT TTL y license `expires_at`
  dependen del reloj. Verificar `w32tm /query /status`.
- **Antivirus**: pharma-service.exe corre como `LocalSystem` desde
  `%ProgramFiles%\PharmaServer\`. Agregar a exclusiones de Defender / antivirus
  del cliente si causa false-positive (binario no firmado — ver §11).
- **Backup pre-existente**: si hay un install previo, **antes de tocar nada**:
  - `Stop-Service PharmaServer`
  - Copiar `%ProgramData%\PharmaServer\data\` y `%ProgramData%\PharmaServer\backups\`
    a una USB externa.
  - Anotar versión actual: `Get-ItemProperty "C:\Program Files\PharmaServer\pharma-service.exe" | Select VersionInfo`.

---

## 2. Install steps

1. **Descargar MSI** desde GitHub Releases:
   `https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.23`
   (`pharma-server-0.1.23-x86_64.msi`, ~12.30 MB).
2. **Validar SHA256** contra el release notes (SmartScreen va a quejarse — el
   binario no está firmado todavía, ver §11). Click "Más información" →
   "Ejecutar de todas formas" si confías en el hash.
3. **Ejecutar como administrador**:
   ```powershell
   Start-Process msiexec -ArgumentList '/i', 'pharma-server-0.1.23-x86_64.msi', '/L*v', 'install.log' -Verb RunAs
   ```
4. **Verificar instalación**:
   ```powershell
   Get-Service PharmaServer                    # Status = Running, StartType = Automatic
   Get-NetFirewallRule -DisplayName 'Pharma Server API' # Enabled = True
   Test-NetConnection 127.0.0.1 -Port 8080     # TcpTestSucceeded = True
   ```
5. **Logs de primer arranque**: el service escribe a tracing stderr → Event
   Viewer → Windows Logs → Application (source `PharmaServer`). Migrations
   corren automáticamente al primer boot (`api/src/lib.rs` `run_embedded`).

---

## 3. Initial config

El service corre como `LocalSystem` con CWD `C:\Windows\System32`. El binario
**no embebe `config/`**, así que los valores vienen de defaults +
`config/local.toml` (opcional) + env vars `PHARMA__*` (separator `__`).

Path canónico de overrides:
```
%ProgramData%\PharmaServer\config\local.toml
```
(crear el dir si no existe). El loader busca `./config/local.toml` relativo al
CWD — para que el service lo lea, pasa los overrides por **env vars de service**,
que es más confiable.

### Env vars OBLIGATORIAS

- `PHARMA__JWT__SECRET` — string aleatorio ≥ 32 bytes. **Nunca dejar el
  placeholder `change-me-in-production`.** Generar:
  ```powershell
  [Convert]::ToBase64String((1..48 | ForEach-Object {Get-Random -Max 256}))
  ```

Setear como env vars del service (persisten en reboot):
```powershell
$svc = 'PharmaServer'
sc.exe stop $svc
[Environment]::SetEnvironmentVariable('PHARMA__JWT__SECRET','<base64-secret>','Machine')
sc.exe start $svc
```

### Env vars OPCIONALES

- `PHARMA__OTLP__ENDPOINT` — si el cliente quiere telemetría OTLP gRPC (e.g.
  `http://localhost:4317`). Default: telemetría OFF (opt-in per ADR-0005 §3).
- `PHARMA__METRICS__TOKEN` — bearer token para `/metrics` Prometheus. Sin él,
  `/metrics` devuelve 401.
- `PHARMA__PUBLIC_ORDERS__HMAC_SECRET` — si se va a aceptar pedidos públicos
  firmados (intake externo).
- `PHARMA__STOCK_WEBHOOK__HMAC_SECRET` — si se conectan webhooks externos de
  stock.
- `PHARMA__BIND` — default `0.0.0.0:8080`. Cambiar sólo si hay conflicto.

> **TBD operator input**: ¿se va a usar OTLP o quedar 100% offline? Default
> recomendado para Coquimbo: offline, no setear OTLP.

---

## 4. First-run admin bootstrap

Migrations ya corrieron al arrancar el service (idempotente). Si el service no
está corriendo y se quiere migrar a mano:
```powershell
Stop-Service PharmaServer    # libera el lock SurrealKv
& 'C:\Program Files\PharmaServer\pharma.exe' migrate
Start-Service PharmaServer
```

> El CLI `pharma` y el service **no pueden correr a la vez** sobre el mismo
> `data/surreal/` (SurrealKv file lock — `CLAUDE.md` §5).

### Crear tenant + usuario admin

```powershell
Stop-Service PharmaServer
& 'C:\Program Files\PharmaServer\pharma.exe' tenant-create 'Tu Farmacia Coquimbo' --slug coquimbo-centro
$env:PHARMA_PASSWORD = '<password-fuerte-aqui>'
& 'C:\Program Files\PharmaServer\pharma.exe' user-create `
  --tenant coquimbo-centro `
  --email <admin-email-TBD> `
  --roles admin,owner
Remove-Item env:PHARMA_PASSWORD
Start-Service PharmaServer
```

> **TBD operator input**:
> - Slug definitivo (`coquimbo-centro` sugerido).
> - Email del admin.
> - Password (mín 12 chars; almacenar en password manager).

Verificar:
```powershell
& 'C:\Program Files\PharmaServer\pharma.exe' tenant-list
& 'C:\Program Files\PharmaServer\pharma.exe' user-list --tenant coquimbo-centro
```

---

## 5. Catalog seed (opcional)

Si la build incluye `feat/migration-full-catalog`:
```powershell
cd 'C:\Program Files\PharmaServer'
& 'C:\path\to\scripts\import_full_catalog.ps1' -Tenant coquimbo-centro
```
Si no, omitir — el operador carga catálogo desde el cliente más adelante.

> **TBD operator input**: confirmar si el branch `release/tufarmacia-golive`
> incluye `scripts/import_full_catalog.ps1` y un CSV/JSON fuente del catálogo
> real de Coquimbo.

---

## 6. License activation

**Opcional.** Sin license, el server arranca Free (ADR-0005). Coquimbo es
piloto interno → puede empezar Free y activar Business después.

1. Obtener el `.lic` o `.json` firmado del license-server (Fase 11; hoy emisión
   manual con la pubkey del licenser).
2. Copiar a `%ProgramData%\PharmaServer\license-incoming.json`.
3. Importar:
   ```powershell
   & 'C:\Program Files\PharmaServer\pharma.exe' license import `
     %ProgramData%\PharmaServer\license-incoming.json
   & 'C:\Program Files\PharmaServer\pharma.exe' license status
   ```
4. Hot-reload sin reiniciar (PR #50, requiere token admin):
   ```powershell
   $env:PHARMA_ADMIN_TOKEN = '<bearer-JWT-admin>'
   & 'C:\Program Files\PharmaServer\pharma.exe' license reload `
     --url http://127.0.0.1:8080
   ```

`pharma license status` debe imprimir `Tier: business` (o el tier importado)
con `Status: active`. Sin license: `Tier: free`, todo el POS sigue operativo.

> **TBD operator input**: archivo `.lic` real para Coquimbo (si aplica).

---

## 7. Client install (POS / admin UI)

El cliente Tauri todavía no tiene MSI propio (PR #72 en flight para la UX de
"server LAN URL"). Para go-live:

- **Opción A — dev build del cliente** (`client/` en este repo): copiar a cada
  caja, configurar `VITE_API_BASE=http://<lan-ip-server>:8080`.
- **Opción B — abrir `http://<lan-ip-server>:8080/app`** en el navegador
  (`api/src/lib.rs::app_index` sirve el SPA estático embebido).

Flujo login: el admin creado en §4 hace login en `/app`, JWT queda en
localStorage, todas las requests subsecuentes llevan `Authorization: Bearer …`.

> **TBD operator input**: IP LAN del server (fijar DHCP reservation por MAC).

---

## 8. Smoke tests

Ejecutar en orden. Todos deben pasar antes de cortar acceso a la operadora:

```powershell
# 1. health
Invoke-RestMethod http://127.0.0.1:8080/health/ready
# → {"status":"ok", ...}

# 2. login admin
$body = @{ email='<admin>'; password='<pwd>' } | ConvertTo-Json
$tok = (Invoke-RestMethod -Method POST -Uri http://127.0.0.1:8080/api/v1/auth/login `
  -ContentType 'application/json' -Body $body).token

# 3. crear una venta de prueba (vía UI o curl/IRM al endpoint POS).

# 4. backup manual
Invoke-RestMethod -Method POST -Uri http://127.0.0.1:8080/api/v1/admin/backup `
  -Headers @{ Authorization = "Bearer $tok" }
# → {"path":"%ProgramData%\\PharmaServer\\backups\\<ts>.tar","bytes":...,"sha256":"..."}
```

Si **alguno** falla → no entregar a operadora; revisar Event Viewer.

---

## 9. Backups

- **Schedule built-in**: `crates/api/src/lib.rs::spawn_scheduler_hub` lanza
  `backup_job` si `cfg.backup.schedule` está seteado. Default config de prod
  recomendado: cron nocturno `0 0 3 * * *` (03:00 AM local).
- **Ubicación**: `%ProgramData%\PharmaServer\backups\<YYYYMMDD-HHMMSS>.tar`
  (tar del SurrealKv data dir + agent.key + license.json).
- **Retención**: `cfg.backup.retention_days` (default 30). `prune_backups`
  elimina tars más viejos.
- **Manual**: `POST /api/v1/admin/backup` (ver §8 #4) — útil pre-cambio.
- **Restore**:
  1. `Stop-Service PharmaServer`
  2. Renombrar `%ProgramData%\PharmaServer\data\` → `data.bad/`.
  3. Extraer el `.tar` deseado en su lugar.
  4. `Start-Service PharmaServer`; verificar `/health/ready`.

> **TBD operator input**: ¿hay un disco externo / NAS para off-site de los
> backups? Recomendado: copia diaria del `.tar` más reciente a USB encriptada.

---

## 10. Rollback

Si una versión nueva rompe algo en producción:

```powershell
# 1. detener service
Stop-Service PharmaServer

# 2. desinstalar versión actual
$prod = Get-WmiObject Win32_Product | Where-Object {$_.Name -like 'Pharma Server*'}
$prod.Uninstall()

# 3. reinstalar MSI previo (mantener copia local del MSI anterior siempre)
Start-Process msiexec -ArgumentList '/i','pharma-server-<prev>-x86_64.msi','/L*v','rollback.log' -Verb RunAs

# 4. restaurar último backup (ver §9)

# 5. start + smoke (§8)
Start-Service PharmaServer
```

> **MajorUpgrade** está habilitado en `main.wxs` line 27 — instalar un MSI
> nuevo sobre uno viejo es safe. Downgrade requiere uninstall explícito (el
> `DowngradeErrorMessage` bloquea instalar versión menor encima).

---

## 11. Known limitations

- **No firmado Authenticode**: SmartScreen muestra warning en primer arranque
  del MSI. Cert pendiente (bloqueo Fase 9 — ver `bitacora.md`). Validar SHA256
  del MSI contra el release.
- **MSI release pinned v0.1.23**: CI billing-blocked para releases nuevos
  (`CLAUDE.md` header). Versiones 0.1.24+ están en git pero **sin MSI**. Para
  Coquimbo: usar v0.1.23 hasta resolver CI billing.
- **License-server no existe aún**: Fase 11 en flight (`pharma-license-server`
  repo separado, ADR-0004). Hoy las licenses se emiten a mano si se necesitan.
  Coquimbo puede operar Free.
- **Sin GUI installer extra**: el MSI no pide config — toda la config va por
  env vars post-install (§3).
- **Service y CLI son exclusivos** sobre `data/surreal/` (SurrealKv lock).
  Siempre `Stop-Service PharmaServer` antes de correr `pharma migrate`,
  `pharma user-create`, etc.
- **JWT secret placeholder**: si se olvida setear `PHARMA__JWT__SECRET`, el
  default `change-me-in-production` queda activo → todos los tokens son
  forjeables. **Bloqueo de seguridad**: no entregar a operadora sin §3.
- **Telemetría OFF por default**: si el cliente quiere métricas/traces, hay que
  setear `PHARMA__OTLP__ENDPOINT` y `PHARMA__METRICS__TOKEN` explícitamente
  (ADR-0005 §3 — opt-in).

---

## 12. Support contacts / next steps

- **Source of truth técnica**: este repo (`pabloalvarez99/pharma-server`) +
  `CLAUDE.md` + `bitacora.md`.
- **Vault Obsidian (operador interno)**:
  `C:\Users\Administrator\Documents\obsidian-mind\work\active\pharma-server\`.
- **ADRs relevantes**:
  - [ADR-0001](../adr/0001-freemium-pivot.md) — por qué freemium MSI.
  - [ADR-0002](../adr/0002-license-ed25519-offline.md) — license Ed25519
    offline.
  - [ADR-0005](../adr/0005-core-gratis-no-locked-in.md) — invariantes (core
    gratis, offline-first, sin kill-switch, sin lock-in).
- **Strategy docs**:
  - [`docs/strategy/freemium-master-plan.md`](../strategy/freemium-master-plan.md) — tier matrix completa.
  - [`docs/strategy/license-architecture.md`](../strategy/license-architecture.md) — arquitectura del licenciamiento.
- **MSI build interno**: [`docs/ops/msi-build.md`](./msi-build.md).
- **Backup/restore profundo**: [`docs/ops/backup-restore.md`](./backup-restore.md).
- **Performance budgets**: [`docs/ops/performance.md`](./performance.md).
- **Production checklist genérico**: [`docs/ops/production-checklist.md`](./production-checklist.md).

> Si algo en Coquimbo se comporta distinto a este playbook → registrar en
> `brain/pharma-server-gotchas.md` del vault + abrir issue en el repo con label
> `coquimbo-prod`.

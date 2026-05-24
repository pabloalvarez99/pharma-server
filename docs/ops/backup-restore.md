# Backup y restauración

Cómo funciona el backup de la base SurrealKv en pharma-server y cómo restaurar.

> Implementación: `crates/api/src/v1/backup.rs` (`backup_now`, `prune_backups`)
> y el hub de scheduler en `crates/api/src/lib.rs` (`spawn_scheduler_hub` →
> `backup_job`). Config: sección `[backup]` (ver
> [`config/production.toml.example`](../../config/production.toml.example)).

## Qué se respalda

Un backup es un **tarball gzip** que contiene **todo el data dir del install**:

- `surreal/` — el directorio SurrealKv completo (todos los tenants, todas las
  tablas). El dump NO es por-tenant: quien restaure ve los datos de todos.
- `agent.key` — la identidad Ed25519 del nodo (para que la identidad de
  federación sobreviva a la restauración).

Salida: `<data_dir>/backups/pharma-backup-<YYYYMMDDtHHMMSSZ>.tar.gz`, donde
`<data_dir>` es el **padre** de `[db].path` (en el MSI:
`%ProgramData%\PharmaServer\data\`, salvo que se haya movido el data dir).

SurrealKv es un store LSM: un backup con el servicio corriendo es un snapshot
que puede ir unos ms atrás del último commit, pero siempre es
crash-recoverable al restaurar (replay del WAL). Para un backup totalmente
quiescido, **detén el servicio** antes (`Stop-Service PharmaServer`).

## Backup automático (programado)

Lo dispara el cron de `[backup].schedule` (6 campos UTC). Recomendado:
`"0 0 3 * * *"` (03:00 UTC diario). Vacío = desactivado. Tras cada corrida se
podan los backups con más de `[backup].retention_days` días (`0` = infinito).
Al arrancar, el log indica `backup scheduler started` con `schedule` y
`retention_days`; cada corrida loguea `scheduled backup completed` con `path`,
`bytes` y `sha256`.

## Backup manual (on-demand)

Endpoint admin (rol `admin` u `owner`):

```text
POST /api/v1/admin/backup
Authorization: Bearer <jwt>
```

Devuelve `201 Created` con un JSON `{ path, bytes, sha256, started_at,
duration_ms }`. Ejemplo PowerShell:

```powershell
$h = @{ Authorization = "Bearer $jwt" }
Invoke-RestMethod -Method Post -Headers $h `
  -Uri http://127.0.0.1:8080/api/v1/admin/backup
```

El `sha256` del reporte permite verificar integridad del tarball más tarde:

```powershell
(Get-FileHash <ruta>\pharma-backup-XXXX.tar.gz -Algorithm SHA256).Hash.ToLower()
```

## Restauración

> **No hay restauración guiada todavía.** No existe endpoint ni comando CLI de
> restore: el código sólo implementa backup y poda. La restauración guiada
> (verificar archivo, parar servicio, swap atómico, re-arrancar) está en el
> **roadmap**. Hoy se hace por **copia manual de archivos**.

Procedimiento manual (requiere PowerShell **como Administrador**):

1. **Detener el servicio** para soltar el file lock de SurrealKv:

   ```powershell
   Stop-Service PharmaServer
   ```

2. **Resguardar el data dir actual** (por si la restauración sale mal):

   ```powershell
   $data = "C:\ProgramData\PharmaServer\data"
   Rename-Item "$data\surreal" "surreal.bak-$(Get-Date -Format yyyyMMddHHmmss)"
   ```

3. **Extraer el tarball** del backup elegido a una carpeta temporal y verificar
   que contiene `surreal/` (+ opcional `agent.key`):

   ```powershell
   $tmp = New-Item -ItemType Directory "$env:TEMP\pharma-restore"
   tar -xzf <ruta>\pharma-backup-XXXX.tar.gz -C $tmp     # tar viene con Win10/11
   ```

4. **Copiar** el contenido restaurado al data dir:

   ```powershell
   Copy-Item "$tmp\surreal" $data -Recurse -Force
   # Restaurar agent.key sólo si quieres recuperar la identidad de federación
   if (Test-Path "$tmp\agent.key") { Copy-Item "$tmp\agent.key" $data -Force }
   ```

5. **Arrancar el servicio** y verificar:

   ```powershell
   Start-Service PharmaServer
   Invoke-RestMethod http://127.0.0.1:8080/health/ready   # 200 OK
   ```

6. Validado el funcionamiento, eliminar el `surreal.bak-*` del paso 2.

> Restaurar SOBRE un servicio en marcha corrompe el store (dos escritores sobre
> los archivos SurrealKv). Siempre detén el servicio primero (paso 1).

## Recomendaciones

- Copiar los `pharma-backup-*.tar.gz` a un medio externo o NAS de la LAN; el
  backup local en el mismo disco no protege contra falla de hardware.
- Probar la restauración en una VM/equipo aparte **antes** del go-live
  (checklist punto (d)) y periódicamente. Un backup no probado no es un backup.

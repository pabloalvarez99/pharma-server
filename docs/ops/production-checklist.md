# Checklist de puesta en producción (go-live)

Lista de verificación para el operador **antes** de poner pharma-server en
producción en una farmacia real (offline-first, LAN-only). Marca cada punto.

> Plantilla de configuración: [`config/production.toml.example`](../../config/production.toml.example).
> Backup y restauración: [`backup-restore.md`](./backup-restore.md).

Convención: los comandos PowerShell se ejecutan en una consola **como
Administrador**. `setx ... /M` escribe la variable a nivel de **máquina** para
que el servicio (LocalSystem) la vea. Tras cambiar variables de entorno de
máquina, **reinicia el servicio** (`Restart-Service PharmaServer`).

---

## (a) Secreto JWT fuerte

El default `change-me-in-production` y el demo `demo-secret-do-not-use-in-production`
NO se usan en producción. Generar uno aleatorio de 32 bytes e inyectarlo por
entorno (nunca en un archivo commiteado):

```powershell
# Opción openssl
$secret = openssl rand -hex 32
# Opción sólo-PowerShell (sin openssl)
$secret = -join ((1..32) | ForEach-Object { '{0:x2}' -f (Get-Random -Maximum 256) })

setx PHARMA__JWT__SECRET $secret /M
```

- [ ] `PHARMA__JWT__SECRET` seteado, valor != placeholder, 64 chars hex.
- [ ] Servicio reiniciado para tomar la variable.

## (b) Token de métricas

`GET /metrics` (Prometheus) exige bearer. Vacío = responde 401 (al arranque el
log avisa `metrics token not configured`). Setear uno:

```powershell
$mtok = openssl rand -hex 32
setx PHARMA__METRICS__TOKEN $mtok /M
```

- [ ] `PHARMA__METRICS__TOKEN` seteado.
- [ ] `GET /metrics` sin token → 401; con `Authorization: Bearer <token>` → 200.

## (c) Firewall — sólo la LAN llega al 8080

El MSI abre TCP 8080 inbound. Restringirlo al rango LAN; nunca exponer a
internet directo. Si se requiere acceso remoto: reverse-proxy con TLS en VPN.

```powershell
# Limitar la regla del instalador al rango LAN (ajustar al subnet real)
Get-NetFirewallRule -DisplayName "Pharma Server API" |
  Set-NetFirewallRule -RemoteAddress 192.168.1.0/24
```

- [ ] Regla de firewall existe y su `RemoteAddress` = subnet LAN (no `Any`).
- [ ] Verificado desde otra caja de la LAN: `curl http://<IP-LAN>:8080/health/live` → 200.
- [ ] Verificado que el 8080 NO responde desde fuera de la LAN.

## (d) Backup verificado + restauración probada

Config en `[backup]`: `schedule = "0 0 3 * * *"`, `retention_days = 30`.

- [ ] `[backup].schedule` no vacío; al arranque el log dice `backup scheduler started`.
- [ ] Forzar un backup manual: `POST /api/v1/admin/backup` (rol admin/owner) → 201
      y aparece `pharma-backup-*.tar.gz` en `<data_dir>/backups/`.
- [ ] **Restauración probada** en un equipo/VM aparte siguiendo
      [`backup-restore.md`](./backup-restore.md). Nota: la restauración guiada NO
      está implementada todavía (roadmap) → hoy es **copia manual de archivos**.

## (e) Data dir en disco no-de-sistema (opcional)

Por defecto los datos viven en `%ProgramData%\PharmaServer\data\surreal`. Para
aislar del disco del SO, mover a otro disco vía `[db].path` (ej. `D:\PharmaData\surreal`)
o `PHARMA__DB__PATH`. El servicio y la CLI no deben tocar ese dir a la vez
(file lock SurrealKv).

- [ ] (Opcional) `[db].path` apunta a disco dedicado con espacio suficiente.
- [ ] Carpeta `backups/` (hermana de `surreal/`) cae en ese mismo disco.

## (f) Servicio Windows con auto-arranque

El MSI registra el servicio `PharmaServer` (LocalSystem, auto-start).

```powershell
Get-Service PharmaServer            # Status = Running
sc.exe qc PharmaServer              # START_TYPE = AUTO_START
```

- [ ] Servicio `PharmaServer` instalado, `Running`, `START_TYPE = AUTO_START`.
- [ ] Reinicio del equipo → el servicio levanta solo y `GET /health/live` → 200.

## (g) Archivo de licencia

El servidor lee `<data_dir>\license.json` al arrancar; si falta o es inválida,
cae a **tier Free** (el core ERP siempre funciona offline — ADR-0005). La
licencia firmada real es de **Fase 11**; por ahora Free cubre el core.

```powershell
# Sólo cuando exista una licencia firmada:
pharma license import C:\ruta\a\licencia.lic
pharma license status
# Sin reiniciar el servicio: POST /api/v1/admin/license/reload (rol admin)
```

- [ ] Sin licencia: `pharma license status` confirma Free (operación normal, OK).
- [ ] Con licencia (si aplica): importada, `status` muestra tier/expiry correctos,
      y reload aplicado sin reiniciar.

## (h) Telemetría APAGADA salvo opt-in

Invariante (Ley 19.628): telemetría **opt-in, default OFF, sin PII**.
`[otlp].endpoint` vacío = exporter deshabilitado (no sale tráfico).

- [ ] `[otlp].endpoint` vacío (y `PHARMA__OTLP__ENDPOINT` no seteado), salvo que
      el cliente haya **consentido explícitamente** un colector propio.

## (i) Sin tenants/usuarios demo en la DB de producción

La demo usa secreto JWT de juguete y tenants/usuarios de prueba. La DB de
producción arranca **limpia**: sin datos demo.

- [ ] DB de producción es nueva (no copiada de la demo).
- [ ] Tenant(s) real(es) y usuarios creados con `pharma tenant-create` /
      `pharma user-create` (password vía prompt o `PHARMA_PASSWORD`, argon2id).
- [ ] Ningún usuario con password placeholder ni tenant `demo`/`test`.

---

## Verificación final de humo

- [ ] `GET /health/live` y `GET /health/ready` → 200.
- [ ] Login real emite JWT (firmado con el secreto de producción).
- [ ] Una venta/operación de POS responde rápido (<100ms percibido).
- [ ] Backup manual genera tarball; restauración probada al menos una vez.

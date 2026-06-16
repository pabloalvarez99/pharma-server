# MSI local dry-run (unsigned) — de-risk piloto

Procedimiento verificado para construir y smoke-testear el MSI **local sin firmar**.
NO es release: no firma Authenticode, no se promueve a Latest, no se publica al mirror.
El corte de release sigue siendo acción manual del fundador (regla #9/#10).

**Verificado**: 2026-06-16, `pharma-server-0.1.24-x86_64.msi` (12.35 MB), Windows 11 Pro,
WiX v3.14.1.8722, cargo-wix 0.3.9. Todo verde. Service Running + `/health/ready` 200.

## Prereqs

- `cargo install cargo-wix` (0.3.9 verificado).
- WiX v3 toolset: `choco install wixtoolset`. Binarios en
  `C:\Program Files (x86)\WiX Toolset v3.14\bin` (candle.exe, light.exe).
- PowerShell **elevado** (perMachine install requiere admin).

## Build

Dos pasos. Primero compilar el binario release; luego empaquetar **sin rebuild**.

```powershell
# 1. binario release (desde la raíz del repo)
cargo build --release -p service

# 2. empaquetar MSI — IMPORTANTE: correr desde crates/service, NO desde la raíz
cd crates\service
cargo wix --package service --no-build --nocapture `
  -C -ext -C WixFirewallExtension `
  -L -ext -L WixFirewallExtension
# → target\wix\pharma-server-<version>-x86_64.msi
```

### Gotchas de build (root cause documentado)

1. **`cargo wix` debe correr desde `crates/service`, NO desde la raíz.**
   El `include` en `crates/service/Cargo.toml` (`[package.metadata.wix]`) es
   `../../installer/wix/main.wxs`, y cargo-wix resuelve `include` **relativo al cwd**,
   no al manifest. Desde la raíz falla con
   `The '../../installer/wix/main.wxs' file does not exist`.
   (El README muestra invocación desde la raíz — está **desactualizado** en este punto.)

2. **Usar `--no-build`.** Sin él, cargo-wix re-dispara `cargo build` en un contexto que
   puede regenerar artefactos stale de `utoipa-swagger-ui` y romper la compilación
   (`SwaggerUiDist::get` not found — mismatch rust-embed 8.11 vs swagger-ui 8.1 cuando
   se regenera `out/embed.rs` con `#[folder]` apuntando a un checkout viejo).
   `cargo build --release -p service` directo **sí** compila limpio (EXIT 0); el
   problema es solo el rebuild interno de cargo-wix. Compilar primero + `--no-build`
   lo evita.

3. **Target stale entre checkouts.** Si el repo se movió de ruta (este se movió de
   `C:\Users\...\Documents\GitHub\pharma-server` → `D:\Respaldo...`), los `#[folder]`
   absolutos embebidos por build scripts en `target/` quedan apuntando a la ruta vieja.
   `cargo build --release -p service` los regenera correctamente.

## Smoke test (install / uninstall)

Todos los comandos en PowerShell **elevado**.

```powershell
$msi = "target\wix\pharma-server-0.1.24-x86_64.msi"

# INSTALL (silent + verbose log)
msiexec /i $msi /qn /l*v install.log

Get-Service PharmaServer                                   # Status=Running, StartType=Automatic
Get-NetFirewallRule -DisplayName "Pharma Server API"       # Inbound / Allow / Enabled
Invoke-WebRequest http://127.0.0.1:8080/health/live  -UseBasicParsing   # 200
Invoke-WebRequest http://127.0.0.1:8080/health/ready -UseBasicParsing   # 200 (DB abierta)

# UNINSTALL
msiexec /x $msi /qn /l*v uninstall.log
```

### Resultado esperado (verificado 2026-06-16)

| Check                        | Install            | Uninstall              |
|------------------------------|--------------------|------------------------|
| Service `PharmaServer`       | Running, Automatic | Removed                |
| Firewall "Pharma Server API" | Inbound/Allow/On   | Removed                |
| `C:\Program Files\PharmaServer\pharma-service.exe` | Presente | Removed |
| `C:\ProgramData\PharmaServer\` (datos) | Creado (`data\`) | **Retenido** (a propósito) |
| binPath del servicio         | `"C:\Program Files\PharmaServer\pharma-service.exe"` | — |

**Datos retenidos al desinstalar es correcto**: el componente `PharmaDataDir` usa
`<CreateFolder/>` sin remoción → la BD SurrealKv del cliente NO se borra. Cumple
invariante de continuidad (core gratis sigue, sin lock-in de datos).

### Reinstall sobre datos retenidos (verificado)

Tras uninstall (datos retenidos), reinstalar el mismo MSI → service Running +
`/health/ready` 200: SurrealKv reabre la BD existente sin lock ni corrupción. OK.

## Estado de bloqueantes conocidos

- **ServiceComponents vacío (ex-bloqueante M3): RESUELTO.** `installer/wix/main.wxs`
  ya define el `<Component>` completo con `ServiceInstall` + `ServiceControl` +
  `FirewallException`. La nota "ServiceComponents está vacío hoy" en CLAUDE.md/README
  está **desactualizada**.

## Residuales (NO bloqueantes para piloto)

1. **MajorUpgrade real no probado en este dry-run.** El elemento `<MajorUpgrade>` está
   en el wxs (l.27) pero testearlo de verdad requiere un MSI de versión mayor
   (ej. 0.1.25 sobre 0.1.24). El reinstall same-version verifica la reapertura de BD,
   no el `RemoveExistingProducts` con downgrade-guard. TODO al próximo bump de versión.
2. **Sin firma Authenticode** → SmartScreen warning en máquina limpia. Gate de release
   (no de dry-run). Bloqueado por cert (Fase 9).
3. **README desactualizado**: muestra `cargo wix` desde la raíz; correr desde
   `crates/service` (ver gotcha #1).

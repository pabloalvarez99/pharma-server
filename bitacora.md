# bitácora — pharma-server

Registro cronológico de decisiones técnicas, cambios significativos e incidentes.
Formato: `## YYYY-MM-DD — título corto` + bullets `qué / por qué / archivos / commit`.
Espejada en vault: `C:/Users/Administrator/Documents/obsidian-mind/work/active/pharma-server/bitacora.md`.

---

## 2026-05-07 — scaffold inicial + sistema de memoria/contexto

- **Qué**: estado actual del repo tras 4 commits iniciales + setup del sistema de memoria/contexto (CLAUDE.md, esta bitácora, vault hint hook, notas Obsidian).
- **Por qué**: dejar a futuras sesiones Claude (y al dev humano) un punto de entrada claro al proyecto sin tener que reconstruir contexto desde cero.
- **Estado del scaffold**:
  - 8 crates: `core`, `db`, `api`, `auth`, `jobs`, `telemetry`, `service`, `cli`.
  - axum 0.8 API + JWT HS256 auth (`/api/me` con extractor `AuthUser` ya funciona y testeado).
  - SurrealDB embedded `kv-surrealkv` en `./data/surreal` (ns `pharma`, db `main`).
  - Migración `0001_init.surql` aplicada via CLI: tablas `tenant`, `user` (argon2id), `session` (jti UNIQUE).
  - CLI `pharma migrate` IMPLEMENTADO con tracking `_migrations` SCHEMAFULL.
  - CLI `pharma config` IMPLEMENTADO. `tenant-create` y `user-create` son **TODO stubs**.
  - Service Windows funcional (`pharma-service`, `OWN_PROCESS`, name `PharmaServer`) — embeds `api::run` en runtime tokio.
  - WiX skeleton (`installer/wix/main.wxs`) con MajorUpgrade y dirs INSTALLFOLDER/DATAFOLDER pero `ServiceComponents` **vacío** — bloqueante MSI.
  - CI windows-latest: fmt + clippy + build --release + test, sube `pharma-api.exe` como artifact (no MSI todavía).
  - Telemetry: `tracing_subscriber` JSON + EnvFilter funciona. **OTLP wiring NO implementado** (config existe, código no exporta).
  - Jobs: scheduler vacío. NATS no usado.
- **Archivos creados en este chunk**:
  - `CLAUDE.md` (raíz)
  - `bitacora.md` (raíz, este archivo)
  - `.claude/hooks/vault-hint.sh`
- **Notas Obsidian creadas** (vault, no repo): `work/active/pharma-server/{index,bitacora,decisions-log-index}.md`, `reference/pharma-server-{architecture,db,api,cli,msi,ci,env}.md`, `brain/pharma-server-{patterns,decisions,gotchas,north-star}.md`.
- **Commits relevantes pre-bitácora**:
  - `a6207c5` feat(cli): implement migrate command with _migrations tracking
  - `234ee1b` refactor(api): expose lib::run; service hosts api in-process
  - `737ad79` feat(scaffold): initial pharma-server workspace
  - `95a60aa` chore: initial commit
- **Commit de este chunk**: `8e80f62` — `chore: scaffold project memory (CLAUDE.md, bitacora, vault hooks)`. Pushed a `origin/feature/pharma-server-scaffold`.
- **Próximos pasos sugeridos** (no compromiso):
  1. Implementar `pharma tenant-create` y `pharma user-create` (CLI stubs).
  2. Llenar `installer/wix/main.wxs` `ServiceComponents` con ServiceInstall + ServiceControl + firewall rule.
  3. Wire OTLP exporter en `crates/telemetry`.
  4. `/health/ready` debería pingear SurrealDB en lugar de devolver `db: "skipped"`.

---

## 2026-05-07 — MSI installer end-to-end + Windows service smoke

- **Qué**: WiX installer completo y verificado. MSI instala servicio + abre firewall + crea data dir. Smoke directo `sc.exe` también verificado.
- **Por qué**: cerrar M3 (MSI shippeable). Antes `ServiceComponents` estaba vacío, ahora produce instalación funcional one-shot.
- **Cambios**:
  - `installer/wix/main.wxs`: `ServiceInstall` (LocalSystem, auto-start, ownProcess), `ServiceControl` (start install / stop both / remove uninstall), `fire:FirewallException` TCP 8080, `DataDirComponents` con GUID explícito (CreateFolder no permite GUID `*`), Version `$(var.Version)` (cargo-wix lo inyecta).
  - `crates/service/Cargo.toml`: `[package.metadata.wix]` con `upgrade-guid`, `path-guid`, `include = ["../../installer/wix/main.wxs"]`, `extensions = ["WixFirewallExtension"]`.
  - Comando build: `cargo wix --package service --no-build --nocapture -C -ext -C WixFirewallExtension -L -ext -L WixFirewallExtension`.
  - WiX v3.14 vía `choco install wixtoolset` (no estaba). cargo-wix 0.3.9 ya estaba.
  - README: secciones "Run as Windows service" + "MSI installer" con install/uninstall msiexec.
- **Smoke directo (sc.exe)**: `create / start / Get-Service Running / curl /health/live → 200 / curl /api/me → 401 / stop / delete` ✓.
- **Smoke MSI**: `msiexec /i pharma-server-0.1.0-x86_64.msi /qn` → service Running, `/health/live` 200, firewall rule "Pharma Server API" Inbound Allow Enabled. `msiexec /x ... /qn` → service y dir y firewall borrados ✓.
- **Gotchas (→ vault `brain/pharma-server-gotchas.md`)**:
  - WiX comments **no pueden contener `--`**. Usar texto sin double-dash.
  - cargo-wix metadata `extensions` no se honra siempre; pasar via CLI `-C -ext -C <Name>` para candle y `-L -ext -L` para light.
  - Componentes con `<CreateFolder/>` (Directory KeyPath) **requieren GUID explícito**, no `Guid="*"`.
  - msiexec: `cmd.exe /c "msiexec /i ..."` necesario en bash MSYS para evitar mangling de paths con `//`.
- **Commits**:
  - `66d0967` feat(installer): WiX ServiceInstall + firewall rule for TCP 8080
  - `db6d71d` fix(installer): MSI builds — explicit data-dir GUID, slim ext list, valid XML comments
  - `4b4d667` docs(readme): add Windows service smoke + MSI install/uninstall sections
- **CI**: run 25525854049 verde con MSI fix commit. README-only commit run en curso (no afecta build).
- **Estado scaffold tras este chunk**: M3 MSI cerrado. Pendiente: `pharma tenant-create` / `user-create`, OTLP wiring, `/health/ready` real DB ping, firmar MSI con cert (sin firma → SmartScreen warning).

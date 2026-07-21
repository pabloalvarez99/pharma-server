# SP1 — pharma-api Linux en GCE VM (Caddy TLS + systemd + backup GCS)

Ingeniero en **pharma-server** (Rust workspace). Objetivo: el server multi-tenant
corriendo público en una VM GCE Linux, base del SaaS web (spec
`docs/superpowers/specs/2026-07-21-saas-web-cloud-design.md` — NO leerla, todo lo
necesario está aquí).

## Setup

- Worktree del lane (ya existe): `D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\saas-web`,
  branch `feature/saas-web`, base `origin/feature/erp-parity`. Trabajar AHÍ (checkout
  principal está sucio en otra branch — no tocarlo).
- gcloud 572.0.0 instalado, cuenta `timadapa@gmail.com`, proyecto activo
  `tu-farmacia-prod` = farmacia real → **PROHIBIDO tocarlo**. Todo con
  `--project rutbusiness-cloud` explícito, jamás cambiar el default.

## Leyes

1. GATE antes de cada commit: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
2. Secrets nunca al repo: JWT secret y provisioning key van por env `PHARMA__*` en la VM.
3. Firewall GCP solo 80/443; `pharma-api` escucha loopback detrás de Caddy.
4. Si sccache da error de build: `CARGO_INCREMENTAL=0`. Puerto 8080 local suele estar
   ocupado → para pruebas locales usar 8090.
5. Push + PR tras GATE verde (este SP abre el PR del lane contra `feature/erp-parity`).

## Hechos verificados (2026-07-21)

- Crates: `agent api auth cli core db domain dte jobs license service telemetry`.
  `service` usa `windows-service` 0.7 → Windows-only, queda FUERA del build Linux.
  Binarios necesarios en la VM: `pharma-api` (crate `api`) + `pharma` (crate `cli`,
  para `migrate` / `tenant-create` / `user-create` / `seed-demo`).
- Toolchain pin `rust-toolchain.toml` = 1.95.0, edition 2021. Target actual
  `x86_64-pc-windows-msvc`; Linux = `x86_64-unknown-linux-gnu` (verificar que compila:
  GATE 0 abajo).
- SurrealDB 2.1 embedded `kv-surrealkv` (archivo en disco — por eso VM con disco
  persistente, NO Cloud Run). Config loader: `config/default.toml` →
  `config/local.toml` → env `PHARMA__*` separator `__` (ej. `PHARMA__JWT__SECRET`).
- SurrealKv file lock: CLI y api NO pueden correr a la vez sobre el mismo data dir.
- e2-micro free tier: us-central1/us-west1/us-east1, 1 vCPU compartida, 1GB RAM,
  30GB disco standard. Requiere billing account linkeada al proyecto igual.

## GATE 0 — build Linux (bloqueante de todo el pack)

```powershell
rustup target add x86_64-unknown-linux-gnu
cargo check -p api -p cli --target x86_64-unknown-linux-gnu
```
`check` no linkea → detecta cfg-gates rotos sin necesitar linker Linux. Si falla por
código Windows-only fuera de `service`, cfg-gatear (`#[cfg(windows)]`) mínimo y quirúrgico.
Reportar al founder qué hubo que tocar.

## Build del binario Linux (elegir primera que funcione)

1. **Docker Desktop** (si `docker --version` responde): `rust:1.95` container montando
   el workspace, `cargo build --release -p api -p cli`, extraer binarios.
2. **WSL2** (si `wsl -l -v` muestra distro): rustup dentro + build.
3. **VM de build temporal**: `gcloud compute instances create build-tmp
   --project rutbusiness-cloud --machine-type e2-standard-4 --zone us-central1-a`
   (spot si posible), clonar/rsync source, build, bajar binarios, **borrar la VM**.
   (e2-micro NO compila el workspace: 1GB RAM hace OOM.)

## Infra GCP (todo con `--project rutbusiness-cloud`)

```
gcloud projects create rutbusiness-cloud
gcloud billing projects link rutbusiness-cloud --billing-account <PEDIR AL FOUNDER>
gcloud services enable compute.googleapis.com --project rutbusiness-cloud
# VM prod: e2-micro us-central1-a, debian-12, 30GB pd-standard, IP estática
# firewall: allow 80,443 (tag http-server,https-server); nada más
# bucket backup: gsutil mb -p rutbusiness-cloud gs://rutbusiness-backups
```

En la VM:
- `pharma-api` como systemd unit (`After=network.target`, `Restart=on-failure`,
  env file `/etc/pharma/env` con `PHARMA__JWT__SECRET` fuerte generado, data dir
  `/var/lib/pharma/data`, listen `127.0.0.1:8080`).
- Caddy (apt `caddy`): reverse proxy `:443` → `127.0.0.1:8080`, TLS automático.
  Dominio: **verificar con founder si `rutbusiness.cl` existe/está en Vercel DNS**;
  mientras no, usar IP o `<ip>.nip.io` para TLS provisional.
- Migraciones: correr `pharma migrate` (con api DETENIDO — file lock) en cada deploy.
- Backup: cron diario `tar czf` del data dir (api detenido brevemente o snapshot de
  disco GCE — preferir `gcloud compute disks snapshot` sin downtime) + copia a
  `gs://rutbusiness-backups`. Documentar restore.

## Entregables al repo (lane `feature/saas-web`)

- `scripts/deploy-cloud.sh` — build (método elegido) + scp binarios + migrate + restart.
- `installer/cloud/pharma-api.service` + `installer/cloud/Caddyfile` (templates).
- `docs/product/saas-web-cloud-ops.md` — runbook: crear VM desde cero, deploy,
  backup/restore, upgrade e2-small, TODOs (dominio, billing).
- Cambios cfg-gate de GATE 0 si hubo.

## Verificación final (evidencia antes de declarar listo)

```
curl -s https://<dominio-o-ip>/api/v1/health   # o el endpoint health real — verificar path en crates/api
```
+ crear tenant demo vía CLI en la VM (`pharma tenant-create` + `user-create` +
`seed-demo --tenant demo --vertical minimarket`) y login por HTTP.

## Ship

```powershell
cd "D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\saas-web"
# GATE → commit → push -u origin feature/saas-web → gh pr create --base feature/erp-parity --fill
```

Fin → `✅ SP1 LISTO — pharma-api vivo en GCE, PR #<n> abierto · listo para /clear`.

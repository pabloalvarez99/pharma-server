# SaaS web — runbook cloud ops (pharma-api en GCE)

Estado al 2026-07-21 (SP1). VM real: `pharma-prod`, proyecto `rutbusiness-cloud`,
zona `us-west1-b`, IP estática `136.67.83.70`, e2-micro free tier (1 vCPU compartida,
1GB RAM, 30GB pd-standard). Caddy termina TLS en :443 y hace proxy a
`pharma-api` en `127.0.0.1:8080`. Data SurrealKV en `/var/lib/pharma/data`.

> **Regla de oro**: todo comando `gcloud` lleva `--project rutbusiness-cloud`
> explícito. El default del gcloud local es `tu-farmacia-prod` (farmacia real,
> PROHIBIDO tocar) y NUNCA se cambia.

## Arquitectura en la VM

```
:443/:80 (firewall GCP) → Caddy (TLS auto Let's Encrypt) → 127.0.0.1:8080 pharma-api (systemd)
                                                            └── /var/lib/pharma/data (SurrealKV)
```

- Binarios: `/opt/pharma/bin/pharma-api` + `/opt/pharma/bin/pharma` (CLI).
- Unit: `/etc/systemd/system/pharma-api.service` (template: `installer/cloud/pharma-api.service`).
- Env: `/etc/pharma/env` (template: `installer/cloud/env.template`; root:pharma 640, secrets NUNCA al repo).
- Caddy: `/etc/caddy/Caddyfile` (template: `installer/cloud/Caddyfile`).
- SSH: solo vía IAP (`gcloud compute ssh pharma-prod --project rutbusiness-cloud --zone us-west1-b --tunnel-through-iap`). Firewall público = solo 80/443.
- **SurrealKV file lock**: CLI y api NO pueden correr a la vez sobre el mismo data dir. Siempre `systemctl stop pharma-api` antes de `pharma migrate`/seed.

## Crear la VM desde cero

```bash
# 1. Proyecto + billing + API (ya hecho; idempotente)
gcloud projects create rutbusiness-cloud
gcloud billing projects link rutbusiness-cloud --billing-account 01CFAA-B6DE2D-50B576
gcloud services enable compute.googleapis.com --project rutbusiness-cloud

# 2. IP estática + firewall (80/443 público; SSH solo rango IAP)
gcloud compute addresses create pharma-api-ip-w --project rutbusiness-cloud --region us-west1
gcloud compute firewall-rules create allow-http-https --project rutbusiness-cloud \
  --network default --allow tcp:80,tcp:443 --target-tags http-server,https-server
gcloud compute firewall-rules create allow-ssh-iap --project rutbusiness-cloud \
  --network default --allow tcp:22 --source-ranges 35.235.240.0/20
gcloud compute firewall-rules delete default-allow-ssh default-allow-rdp \
  --project rutbusiness-cloud --quiet

# 3. VM (us-central1 estaba sin capacidad e2-micro el 2026-07-21; us-west1 también es free tier)
IP=$(gcloud compute addresses describe pharma-api-ip-w --project rutbusiness-cloud \
  --region us-west1 --format='value(address)')
gcloud compute instances create pharma-prod --project rutbusiness-cloud --zone us-west1-b \
  --machine-type e2-micro --image-family debian-12 --image-project debian-cloud \
  --boot-disk-size 30GB --boot-disk-type pd-standard \
  --tags http-server,https-server --address "$IP"

# 4. Bucket de backups
gsutil mb -p rutbusiness-cloud gs://rutbusiness-backups
```

### Provisión dentro de la VM (una vez)

```bash
gcloud compute ssh pharma-prod --project rutbusiness-cloud --zone us-west1-b --tunnel-through-iap
```

```bash
# Usuario de sistema + dirs
sudo useradd --system --home /var/lib/pharma --shell /usr/sbin/nologin pharma
sudo mkdir -p /opt/pharma/bin /var/lib/pharma/data /etc/pharma
sudo chown -R pharma:pharma /var/lib/pharma

# Env file (usar installer/cloud/env.template como base — set COMPLETO obligatorio:
# el CLI usa loader estricto; pharma-api con env incompleto cae SILENCIOSO a
# defaults embebidos → bind 0.0.0.0 + data dir ./data/surreal ≠ el del CLI)
sudo tee /etc/pharma/env >/dev/null <<EOF
PHARMA__BIND=127.0.0.1:8080
PHARMA__DB__PATH=/var/lib/pharma/data
PHARMA__DB__NAMESPACE=pharma
PHARMA__DB__DATABASE=main
PHARMA__JWT__SECRET=$(openssl rand -hex 32)
PHARMA__JWT__ISSUER=pharma-server
PHARMA__JWT__TTL_SECONDS=3600
PHARMA__OTLP__ENDPOINT=
PHARMA__OTLP__SERVICE_NAME=pharma-api
EOF
sudo chown root:pharma /etc/pharma/env && sudo chmod 640 /etc/pharma/env

# systemd unit (copiar installer/cloud/pharma-api.service)
sudo cp /tmp/pharma-api.service /etc/systemd/system/pharma-api.service
sudo systemctl daemon-reload && sudo systemctl enable pharma-api

# Caddy (repo oficial apt)
sudo apt-get update && sudo apt-get install -y debian-keyring debian-archive-keyring apt-transport-https curl
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt-get update && sudo apt-get install -y caddy

# Caddyfile: reemplazar <SITE> por dominio o <IP>.nip.io (hoy: 136.67.83.70.nip.io)
sudo cp /tmp/Caddyfile /etc/caddy/Caddyfile && sudo sed -i 's/<SITE>/136.67.83.70.nip.io/' /etc/caddy/Caddyfile
sudo systemctl reload caddy
```

Después correr `./scripts/deploy-cloud.sh` desde el repo (build + scp + migrate + start).

## Deploy (cada release)

```bash
./scripts/deploy-cloud.sh
```

Hace: build release en Docker `rust:1.95` → `gcloud compute scp` binarios →
stop api → start api → check `/health/ready`.

**Migraciones**: van EMBEBIDAS en `pharma-api` (`run_embedded`) y se aplican solas
al arrancar — la VM no necesita el dir `migrations/` ni correr `pharma migrate`
(ese comando es FS-based, dev-only). Log esperado en journal:
`startup migrations complete`.

## Backup / restore

**Backup preferido (sin downtime): snapshot del disco GCE**, diario, retención 7:

```bash
gcloud compute resource-policies create snapshot-schedule pharma-daily \
  --project rutbusiness-cloud --region us-west1 \
  --max-retention-days 7 --start-time 08:00 --daily-schedule \
  --storage-location us-west1
gcloud compute disks add-resource-policies pharma-prod \
  --project rutbusiness-cloud --zone us-west1-b --resource-policies pharma-daily
```

**Backup lógico adicional a GCS** (cron en la VM, `/etc/cron.d/pharma-backup`;
requiere scope de escritura GCS en la VM o service account con `storage.objectAdmin`
sobre el bucket — ver TODO):

```
0 8 * * * root systemctl stop pharma-api && tar czf /tmp/pharma-data-$(date +\%F).tar.gz -C /var/lib/pharma data && systemctl start pharma-api && gsutil cp /tmp/pharma-data-*.tar.gz gs://rutbusiness-backups/ && rm /tmp/pharma-data-*.tar.gz
```

(Downtime ~segundos con data chica; cuando moleste, migrar a solo-snapshots.)

**Restore desde snapshot**:

```bash
gcloud compute snapshots list --project rutbusiness-cloud
gcloud compute disks create pharma-restore --project rutbusiness-cloud \
  --zone us-west1-b --source-snapshot <SNAPSHOT>
# crear VM nueva con ese disco, o attachearlo y copiar /var/lib/pharma/data
```

**Restore desde tar GCS** (api detenido):

```bash
sudo systemctl stop pharma-api
gsutil cp gs://rutbusiness-backups/pharma-data-<FECHA>.tar.gz /tmp/
sudo rm -rf /var/lib/pharma/data && sudo tar xzf /tmp/pharma-data-<FECHA>.tar.gz -C /var/lib/pharma
sudo chown -R pharma:pharma /var/lib/pharma/data
sudo systemctl start pharma-api
```

## Upgrade a e2-small (cuando 1GB RAM quede chico)

```bash
gcloud compute instances stop pharma-prod --project rutbusiness-cloud --zone us-west1-b
gcloud compute instances set-machine-type pharma-prod --project rutbusiness-cloud \
  --zone us-west1-b --machine-type e2-small
gcloud compute instances start pharma-prod --project rutbusiness-cloud --zone us-west1-b
```

(IP estática se conserva. ~USD 13/mes; deja de ser free tier.)

## Operación diaria

```bash
# Logs
gcloud compute ssh pharma-prod --project rutbusiness-cloud --zone us-west1-b \
  --tunnel-through-iap --command 'sudo journalctl -u pharma-api -n 100 --no-pager'
# Estado
curl -s https://136.67.83.70.nip.io/health/ready
# CLI admin (api DETENIDO si toca la DB)
sudo systemctl stop pharma-api
sudo -u pharma bash -c 'set -a; source /etc/pharma/env; set +a; cd /var/lib/pharma && /opt/pharma/bin/pharma tenant-create --name "X" --slug x'
sudo systemctl start pharma-api
```

## TODOs

- **Dominio**: confirmar con founder si `rutbusiness.cl` existe y dónde está su DNS
  (¿Vercel?). Al tenerlo: A record `api.rutbusiness.cl → 136.67.83.70`, reemplazar
  `<SITE>` en `/etc/caddy/Caddyfile` y `systemctl reload caddy`. Mientras tanto el
  TLS provisional corre en `136.67.83.70.nip.io`.
- **Billing**: proyecto linkeado a la única billing account existente
  (`01CFAA-B6DE2D-50B576`, "Firebase Payment"). Confirmar con founder que es la
  correcta para RutBusiness cloud; e2-micro + pd-standard 30GB + IP adjunta = free
  tier, costo esperado ~$0 (egress mínimo).
- **Backup GCS**: la VM usa el service account default con scope `devstorage.read_only`;
  para que el cron `gsutil cp` funcione hay que recrear la VM con
  `--scopes storage-rw` o darle un SA dedicado. Mientras tanto los snapshots GCE
  diarios son el backup efectivo.
- **Provisioning key** (`PHARMA__PROVISIONING__KEY`): llega con SP2; agregar a
  `/etc/pharma/env` cuando exista el endpoint.
- **us-central1 sin capacidad e2-micro** (2026-07-21): si algún día conviene volver
  (latencia), recrear ahí con snapshot.

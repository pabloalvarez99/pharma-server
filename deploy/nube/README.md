# Nube de feria — Hetzner CX22

Una caja. Un `pharma-api`. Caddy en 443. SurrealKV en disco. Varios
puestos (tenants) en la misma base. El APK de feria se firma con
`-Prb.urlNube=https://<este-host>` y **nunca** pide una IP.

El capitán compra la CX22 (Ubuntu 24.04 x86_64) y el hostname.
Este pack no tiene IPs, claves, `authorized_keys` ni secretos reales.

## 1. En la caja, una vez

```bash
sudo useradd --system --home /var/lib/rutbusiness --shell /usr/sbin/nologin rutbusiness
sudo mkdir -p /opt/rutbusiness/bin /var/lib/rutbusiness/data /etc/rutbusiness /var/backups/rutbusiness /var/log/caddy
sudo chown -R rutbusiness:rutbusiness /var/lib/rutbusiness /var/backups/rutbusiness
sudo chown caddy:caddy /var/log/caddy
```

Firewall (revisar y aplicar a mano; ver `ufw.ejemplo`):

```bash
# ufw default deny incoming
# ufw default allow outgoing
# ufw allow 22/tcp && ufw allow 80/tcp && ufw allow 443/tcp
# ufw enable
```

El binario **no** escucha afuera (`PHARMA__BIND=127.0.0.1:8080`).

Journald (opcional, recomendado):

```bash
sudo mkdir -p /etc/systemd/journald.conf.d
sudo cp deploy/nube/journald-rutbusiness.conf /etc/systemd/journald.conf.d/rutbusiness.conf
sudo systemctl restart systemd-journald
```

## 2. Binario y env

Compilar en una máquina con Rust 1.85: `cargo build -p api --release --bin pharma-api`.
Copiar el binario a `/opt/rutbusiness/bin/pharma-api`.

```bash
sudo cp deploy/nube/api.env.ejemplo /etc/rutbusiness/api.env
# Generar JWT EN LA CAJA (puntero, nunca valor en el repo):
#   openssl rand -hex 32  → pegar en PHARMA__JWT__SECRET=
sudo nano /etc/rutbusiness/api.env
sudo chown root:rutbusiness /etc/rutbusiness/api.env
sudo chmod 640 /etc/rutbusiness/api.env
```

## 3. systemd (API)

```bash
sudo cp deploy/nube/rutbusiness-api.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now rutbusiness-api
# Endurecimiento: no Perfect, sí mejor que el unit mínimo
systemd-analyze security rutbusiness-api
```

## 4. Comprobar

Tras copiar binario, env y units (y rellenar `CAMBIAME`), en la VPS:

```bash
sudo bash deploy/nube/comprobar.sh
```

El script valida rutas, que `api.env` ya no tenga `CAMBIAME`, que
`PHARMA__BIND` sea loopback, y `systemctl is-active` de la API (y Caddy
si está). **No** imprime JWT ni el contenido de `api.env`.

## 5. Caddy

Instalar Caddy 2. Exportar el hostname real del capitán:

```bash
export RUTAGENT_HOST=api.ejemplo.cl   # el hostname real, no una IP
sudo cp deploy/nube/Caddyfile /etc/caddy/Caddyfile
# Si usás EnvironmentFile de systemd para caddy, poné RUTAGENT_HOST ahí
sudo mkdir -p /var/log/caddy && sudo chown caddy:caddy /var/log/caddy
sudo caddy validate --config /etc/caddy/Caddyfile
```

DNS A/AAAA del host → esta caja, después `sudo systemctl reload caddy`.

Headers de seguridad (HSTS, nosniff, frame deny, etc.) van en el
`Caddyfile`. No hay `rate_limit` experimental: la build de Caddy en
Ubuntu 24.04 no se asume con módulos extra.

**Logs Caddy:** `/var/log/caddy/rutagent.log` (JSON). No copiar al vault
ni a tickets: puede contener headers sensibles si un cliente los mandó
mal. Rotar en la caja; no reenviar Authorization/cookies/tokens.

## 6. Backup diario (timer)

SurrealKV **no comparte el file lock**: el script para la API un
momento (`systemctl stop`), hace tar, y la levanta de nuevo.

```bash
sudo cp deploy/nube/backup.sh /opt/rutbusiness/bin/backup.sh
sudo chmod 750 /opt/rutbusiness/bin/backup.sh
sudo cp deploy/nube/backup.service /etc/systemd/system/backup-rutbusiness.service
sudo cp deploy/nube/backup.timer /etc/systemd/system/backup-rutbusiness.timer
# DESTINO por defecto: /var/backups/rutbusiness (editá el unit si querés otro disco)
sudo systemctl daemon-reload
sudo systemctl enable --now backup-rutbusiness.timer
systemctl list-timers | grep backup-rutbusiness
```

Retención: tarballs `data-*.tar.gz` en `DESTINO` más viejos que 7 días
se borran al inicio de cada corrida. Un solo backup a la vez
(`flock /run/rutbusiness-backup.lock`). Cero rclone en este pack.

## 7. ¿Anduvo?

Desde la caja: `curl -sS http://127.0.0.1:8080/health/ready` tiene que
contestar JSON. Desde afuera, lo mismo por `https://<host>/health/ready`
y headers de seguridad (HSTS, `X-Content-Type-Options`, etc.).

Logs API: `journalctl -u rutbusiness-api -f`.

`POST /api/v1/setup` **solo** con DB vacía. El segundo feriante entra
por `POST /api/v1/alta`. Login: correo + clave (si el correo es de un
solo puesto; la respuesta trae `tenant_slug` y `tenant_name`). No hay
`X-Provisioning-Key` en el APK. Snapshot semanal de la CX22 en el panel
del proveedor (aparte de este tar diario).

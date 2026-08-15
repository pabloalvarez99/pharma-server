# Nube de feria — Hetzner CX22

Una caja. Un `pharma-api`. Caddy en 443. SurrealKV en disco. Varios
puestos (tenants) en la misma base. El APK de feria se firma con
`-Prb.urlNube=https://<este-host>` y **nunca** pide una IP.

El capitán compra la CX22 (Ubuntu 24.04 x86_64, Ashburn) y el hostname.
Este pack no tiene IPs, claves ni `authorized_keys`.

## 1. En la caja, una vez

```bash
sudo useradd --system --home /var/lib/rutbusiness --shell /usr/sbin/nologin rutbusiness
sudo mkdir -p /opt/rutbusiness/bin /var/lib/rutbusiness/data /etc/rutbusiness /var/backups/rutbusiness
sudo chown -R rutbusiness:rutbusiness /var/lib/rutbusiness /var/backups/rutbusiness
```

`ufw allow 22,80,443/tcp` y nada más. El binario **no** escucha afuera.

## 2. Binario y env

Compilar en una máquina con Rust 1.85: `cargo build -p api --release --bin pharma-api`.
Copiar el binario a `/opt/rutbusiness/bin/pharma-api`.

```bash
sudo cp deploy/nube/api.env.ejemplo /etc/rutbusiness/api.env
sudo nano /etc/rutbusiness/api.env   # PHARMA__JWT__SECRET=$(openssl rand -hex 32)
sudo chown root:rutbusiness /etc/rutbusiness/api.env
sudo chmod 640 /etc/rutbusiness/api.env
```

## 3. systemd + Caddy

```bash
sudo cp deploy/nube/rutbusiness-api.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now rutbusiness-api
```

Instalar Caddy. `export RUTAGENT_HOST=api.ejemplo.cl` (el hostname real).
Copiar `Caddyfile` a `/etc/caddy/Caddyfile`. DNS A del host → esta caja,
después `systemctl reload caddy`.

## 4. ¿Anduvo?

Desde la caja: `curl -sS http://127.0.0.1:8080/health/ready` tiene que
contestar JSON. Desde afuera, lo mismo por `https://<host>/health/ready`.

Logs: `journalctl -u rutbusiness-api -f`. Backup diario: `backup.sh`
(para la API un segundo; SurrealKV no comparte el lock). Snapshot
semanal de la CX22 en el panel de Hetzner.

`POST /api/v1/setup` sólo en base vacía. El segundo feriante entra por
`POST /api/v1/alta`. Login: correo + clave alcanzan si el correo es de un
solo puesto; la respuesta trae `tenant_slug` y `tenant_name` para que el
teléfono recuerde el puesto. No hay `X-Provisioning-Key` en el APK.

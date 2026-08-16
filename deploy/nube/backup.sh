#!/bin/sh
# Copia el data dir de SurrealKV a un tarball. Destino por variable.
# No sube a ningún lado: el capitán pone DESTINO (disco extra, rsync, etc.).
# Cero credenciales rclone en este pack.
#
#   DESTINO=/var/backups/rutbusiness ./backup.sh
#
# O vía timer: backup.service (Environment=DESTINO=...).
#
# IMPORTANTE: SurrealKV no comparte el file lock. Hay que parar la API
# un momento (systemctl stop) antes del tar; si no, el snapshot puede
# salir inconsistente o el tar fallar por lock.

set -eu

LOCK=/run/rutbusiness-backup.lock
DATA="${PHARMA__DB__PATH:-/var/lib/rutbusiness/data}"
DESTINO="${DESTINO:-/var/backups/rutbusiness}"
STAMP=$(date -u +%Y%m%dT%H%M%SZ)
RETENCION_DIAS=7

# flock en fd 9: una sola corrida a la vez (timer + manual).
exec 9>"$LOCK"
flock -n 9 || {
	echo "backup ya en curso (lock $LOCK)" >&2
	exit 1
}

mkdir -p "$DESTINO"

# Retención: borrar tarballs más viejos que RETENCION_DIAS en DESTINO.
find "$DESTINO" -maxdepth 1 -type f -name 'data-*.tar.gz' -mtime +"$RETENCION_DIAS" -delete

systemctl stop rutbusiness-api
# shellcheck disable=SC2064
trap 'systemctl start rutbusiness-api' EXIT
tar -C "$(dirname "$DATA")" -czf "$DESTINO/data-$STAMP.tar.gz" "$(basename "$DATA")"
systemctl start rutbusiness-api
trap - EXIT

echo "listo $DESTINO/data-$STAMP.tar.gz"

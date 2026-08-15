#!/bin/sh
# Copia el data dir de SurrealKV a un tarball. Destino por variable.
# No sube a ningún lado: el capitán pone DESTINO (disco extra, rclone, etc.).
#
#   DESTINO=/var/backups/rutbusiness ./backup.sh
#
# Parar la API un momento: SurrealKV no comparte el lock.

set -eu

DATA="${PHARMA__DB__PATH:-/var/lib/rutbusiness/data}"
DESTINO="${DESTINO:-/var/backups/rutbusiness}"
STAMP=$(date -u +%Y%m%dT%H%M%SZ)

mkdir -p "$DESTINO"
systemctl stop rutbusiness-api
tar -C "$(dirname "$DATA")" -czf "$DESTINO/data-$STAMP.tar.gz" "$(basename "$DATA")"
systemctl start rutbusiness-api
echo "listo $DESTINO/data-$STAMP.tar.gz"

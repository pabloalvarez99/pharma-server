#!/usr/bin/env bash
# deploy-cloud.sh — build Linux (Docker) + deploy a la VM GCE `pharma-prod`.
#
# Requisitos locales: docker (engine corriendo), gcloud autenticado.
# Firewall de la VM solo expone 80/443; SSH va por IAP tunnel (--tunnel-through-iap).
# Provisión inicial de la VM: ver docs/product/saas-web-cloud-ops.md (runbook).
#
# Uso: ./scripts/deploy-cloud.sh
set -euo pipefail

PROJECT=rutbusiness-cloud
ZONE=us-west1-b
VM=pharma-prod

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Git Bash (MSYS) convierte paths tipo /work a C:/Program Files/Git/work.
# cygpath da el path host nativo para el mount; MSYS_NO_PATHCONV va scoped a docker.
if command -v cygpath >/dev/null 2>&1; then
  HOST_ROOT="$(cygpath -w "$REPO_ROOT")"
else
  HOST_ROOT="$REPO_ROOT"
fi

echo "==> Build release Linux en Docker (rust:1.95)"
# CARGO_TARGET_DIR aparte para no chocar con el target/ Windows del host.
# RUSTC_WRAPPER= anula sccache del host dentro del container.
MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$HOST_ROOT:/work" \
  -v pharma-cargo-registry:/usr/local/cargo/registry \
  -w /work \
  -e CARGO_TARGET_DIR=/work/target/linux-docker \
  -e RUSTC_WRAPPER= \
  rust:1.95 cargo build --release -p api -p cli

BIN_DIR="$REPO_ROOT/target/linux-docker/release"
test -f "$BIN_DIR/pharma-api" && test -f "$BIN_DIR/pharma"

echo "==> Subiendo binarios a $VM"
gcloud compute scp --project "$PROJECT" --zone "$ZONE" --tunnel-through-iap \
  "$BIN_DIR/pharma-api" "$BIN_DIR/pharma" "$VM:/tmp/"

echo "==> Instalando + restart (migraciones van embebidas: pharma-api las aplica al arrancar)"
gcloud compute ssh "$VM" --project "$PROJECT" --zone "$ZONE" --tunnel-through-iap --command '
  set -euo pipefail
  sudo systemctl stop pharma-api 2>/dev/null || true
  sudo install -o root -g root -m 755 /tmp/pharma-api /tmp/pharma /opt/pharma/bin/
  rm -f /tmp/pharma-api /tmp/pharma
  sudo systemctl start pharma-api
  sleep 3
  curl -fsS http://127.0.0.1:8080/health/ready >/dev/null && echo "health/ready OK"
'

echo "==> Deploy listo"

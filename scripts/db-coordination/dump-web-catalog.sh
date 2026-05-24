#!/usr/bin/env bash
# dump-web-catalog.sh — dump the tu-farmacia.cl web product catalog (Cloud SQL
# Postgres) to demo-data/web_catalog.csv for web<->ERP reconciliation.
#
# Strategy: psql is NOT required. We use `gcloud sql export csv`, which runs the
# SELECT server-side and writes the result to a GCS bucket, then we download it.
# A throwaway bucket is created, granted to the instance service account, used,
# and (optionally) deleted. NO DB password is ever needed — export uses the
# Cloud SQL Admin API with your gcloud IAM credentials.
#
# Usage:
#   ./dump-web-catalog.sh
#   PROJECT=tu-farmacia-prod INSTANCE=tu-farmacia-db DB=farmacia ./dump-web-catalog.sh
#   KEEP_BUCKET=1 ./dump-web-catalog.sh     # don't delete the temp bucket
#
# Requires: gcloud authenticated with roles to manage SQL exports + create a
# GCS bucket (e.g. roles/cloudsql.admin + roles/storage.admin, or equivalent).
set -euo pipefail

PROJECT="${PROJECT:-tu-farmacia-prod}"
INSTANCE="${INSTANCE:-}"      # auto-discovered if empty
DB="${DB:-farmacia}"
REGION="${REGION:-southamerica-east1}"
OUT="${OUT:-demo-data/web_catalog.csv}"

# Columns dumped (header is added by reconcile.py — gcloud export emits no header):
#   external_id,name,price,stock,active,updated_at
QUERY="${QUERY:-SELECT external_id, name, price, stock, active, updated_at FROM products ORDER BY external_id}"

repo_root() { git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null || pwd; }
cd "$(repo_root)"

echo "==> Project: $PROJECT  DB: $DB  Region: $REGION"

if [[ -z "$INSTANCE" ]]; then
  echo "==> Discovering Cloud SQL instance..."
  INSTANCE="$(gcloud sql instances list --project "$PROJECT" \
    --filter='DATABASE_VERSION:POSTGRES*' --format='value(NAME)' | head -1)"
  [[ -n "$INSTANCE" ]] || { echo "ERROR: no Postgres instance found in $PROJECT"; exit 1; }
fi
echo "==> Instance: $INSTANCE"

SA="$(gcloud sql instances describe "$INSTANCE" --project "$PROJECT" \
  --format='value(serviceAccountEmailAddress)')"
[[ -n "$SA" ]] || { echo "ERROR: could not read instance service account"; exit 1; }
echo "==> Instance service account: $SA"

BUCKET="gs://${PROJECT}-recon-$(date +%s)"
echo "==> Creating temp bucket: $BUCKET"
gcloud storage buckets create "$BUCKET" --project "$PROJECT" --location "$REGION" >/dev/null

cleanup() {
  if [[ "${KEEP_BUCKET:-0}" == "1" ]]; then
    echo "==> KEEP_BUCKET=1, leaving $BUCKET (delete later: gcloud storage rm -r $BUCKET)"
  else
    echo "==> Cleaning up temp bucket $BUCKET"
    gcloud storage rm -r "$BUCKET" --project "$PROJECT" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

echo "==> Granting objectAdmin on $BUCKET to $SA"
gcloud storage buckets add-iam-policy-binding "$BUCKET" \
  --member="serviceAccount:$SA" --role="roles/storage.objectAdmin" \
  --project "$PROJECT" >/dev/null

URI="$BUCKET/web_catalog.csv"
echo "==> Exporting products to $URI"
gcloud sql export csv "$INSTANCE" "$URI" \
  --database="$DB" --project "$PROJECT" --query="$QUERY"

mkdir -p "$(dirname "$OUT")"
echo "==> Downloading to $OUT"
gcloud storage cp "$URI" "$OUT" --project "$PROJECT"

ROWS="$(wc -l < "$OUT" | tr -d ' ')"
echo "==> Done. $OUT has $ROWS rows (no header)."
echo "    Columns: external_id,name,price,stock,active,updated_at"
echo "    Next: python scripts/db-coordination/reconcile.py"

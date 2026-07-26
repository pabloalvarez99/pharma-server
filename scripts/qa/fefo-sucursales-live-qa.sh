#!/usr/bin/env bash
# FEFO por sucursal — QA en VIVO contra un pharma-api real (migración 0042).
#
# Lo que valida, con dos locales reales y plata/lotes reales:
#   1. FEFO NO cruza de sucursal: el local B tiene el lote que vence ANTES;
#      vender en A consume el lote de A igual y no toca el de B.
#   2. Aislamiento: no se puede vender en B lo que está en A (aunque el stock
#      global alcance de sobra).
#   3. Recepción apunta a UNA sucursal: recibir una OC de B sube sólo B, el
#      lote nace en B, y B puede vender contra él de inmediato.
#   4. Invariante 0042: Σ product_batch.stock[X] == product_branch_stock[X],
#      verificado en ambos locales después de cada movimiento.
#
# Usage:  bash scripts/qa/fefo-sucursales-live-qa.sh
# Requires: target/debug/{pharma,pharma-api}.exe, curl, jq.
# NOTE: jq.exe en Windows emite CRLF — todo valor pasa por `J` (tr -d '\r').
# Exit non-zero si algo falla. Imprime PASS/FAIL por paso.
set -uo pipefail

PORT="${PORT:-8098}"
BASE="http://127.0.0.1:${PORT}"
TENANT="fefoqa"
EMAIL="owner@fefoqa.local"
PASS="qa-secret-123"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${ROOT}/target/debug"
DATA="$(mktemp -d -t pharma-fefoqa-XXXXXX)"
LOG="${DATA}/api.log"

export PHARMA__DB__PATH="${DATA}/surreal"
export PHARMA__BIND="127.0.0.1:${PORT}"
export PHARMA__JWT__SECRET="qa-test-secret-not-prod"
export PHARMA_PASSWORD="${PASS}"

PHARMA="${BIN}/pharma.exe"
API="${BIN}/pharma-api.exe"

FAILS=0
fail() { echo "FAIL: $*"; FAILS=$((FAILS + 1)); }
ok()   { echo "PASS: $*"; }
J() { jq "$@" | tr -d '\r'; }

cleanup() {
  [[ -n "${API_PID:-}" ]] && kill "${API_PID}" 2>/dev/null
  wait "${API_PID}" 2>/dev/null
}
trap cleanup EXIT

echo "== FEFO por sucursal · QA en vivo · data=${DATA} =="

# 1. Schema + tenant + owner -------------------------------------------------
"${PHARMA}" migrate --dir "${ROOT}/migrations"           >/dev/null || { fail "migrate"; exit 1; }
"${PHARMA}" tenant-create "QA FEFO" --slug "${TENANT}"   >/dev/null || { fail "tenant-create"; exit 1; }
"${PHARMA}" user-create --tenant "${TENANT}" --email "${EMAIL}" --roles owner >/dev/null \
  || { fail "user-create"; exit 1; }
ok "setup (migrate 0042 incluida + tenant + owner)"

# 2. Boot + login ------------------------------------------------------------
"${API}" >"${LOG}" 2>&1 &
API_PID=$!
for _ in $(seq 1 40); do
  curl -fsS "${BASE}/health/live" >/dev/null 2>&1 && break
  sleep 0.25
done
curl -fsS "${BASE}/health/live" >/dev/null 2>&1 || { fail "server boot"; cat "${LOG}"; exit 1; }
TOKEN=$(curl -fsS -X POST "${BASE}/api/v1/login" -H 'content-type: application/json' \
  -d "{\"tenant\":\"${TENANT}\",\"email\":\"${EMAIL}\",\"password\":\"${PASS}\"}" | J -r .token)
[[ -n "${TOKEN}" && "${TOKEN}" != "null" ]] || { fail "login"; exit 1; }
AUTH=(-H "authorization: Bearer ${TOKEN}")
ok "server up + login"

CODEF="${DATA}/.httpcode"
gp() {
  local method="$1" path="$2" body="${3:-}" resp
  if [[ -n "${body}" ]]; then
    resp=$(curl -sS -w '\n%{http_code}' -X "${method}" "${AUTH[@]}" "${BASE}${path}" \
      -H 'content-type: application/json' -d "${body}")
  else
    resp=$(curl -sS -w '\n%{http_code}' -X "${method}" "${AUTH[@]}" "${BASE}${path}")
  fi
  echo "${resp}" | tail -1 | tr -d '\r ' > "${CODEF}"
  echo "${resp}" | sed '$d'
}
code() { cat "${CODEF}" 2>/dev/null; }
# éxito = cualquier 2xx: el POS devuelve 201 al crear la venta y 200 en el
# replay idempotente; los CREATE del resto varían igual entre 200 y 201.
is2xx() { [[ "$(code)" =~ ^2[0-9][0-9]$ ]]; }
exp_in() { date -u -d "+$1 days" +%Y-%m-%dT%H:%M:%SZ; }

# on-hand del bucket de una sucursal ("none" = casa matriz)
qty_at() {
  gp GET "/api/v1/stock/sucursales?product=$1&branch=$2" | J -r '.[0].stock // 0'
}
# Σ de los lotes de un producto en una sucursal
batch_sum_at() {
  gp GET "/api/v1/batches?product=$1&branch=$2&limit=500" | J '[.[].stock] | add // 0'
}
# stock de UN lote puntual
batch_stock() {
  gp GET "/api/v1/batches?product=$1&branch=$2&limit=500" \
    | J -r --arg c "$3" '.[] | select(.batch_code==$c) | .stock'
}
# invariante 0042 en una sucursal
assert_cuadra() {
  local pid="$1" br="$2" label="$3" b q
  b=$(batch_sum_at "${pid}" "${br}")
  q=$(qty_at "${pid}" "${br}")
  if [[ "${b}" == "${q}" ]]; then
    ok "invariante 0042 en ${label}: Σ lotes == on-hand == ${q}"
  else
    fail "invariante 0042 roto en ${label}: Σ lotes=${b} vs on-hand=${q}"
  fi
}

# 3. Dos sucursales reales ---------------------------------------------------
# Casa matriz = bucket NONE (el negocio siempre lo tiene). Creamos el 2º local.
BR_B=$(gp POST /api/v1/sucursales '{"name":"Local Centro"}' | J -r '.id')
[[ -n "${BR_B}" && "${BR_B}" != "null" ]] || { fail "crear sucursal ($(code))"; exit 1; }
ok "sucursal B = ${BR_B} (A = casa matriz)"

PID=$(gp POST /api/v1/products '{"name":"QA FEFO Item","price":"2000","stock":0}' | J -r '.id')
[[ -n "${PID}" && "${PID}" != "null" ]] || { fail "crear producto ($(code))"; exit 1; }
ok "producto ${PID} (stock 0 — todo entra por lotes)"

# 4. Lotes: A vence TARDE (90d), B vence PRONTO (10d) ------------------------
gp POST /api/v1/batches \
  "{\"product\":\"${PID}\",\"batch_code\":\"L-CASA\",\"expiry_date\":\"$(exp_in 90)\",\"stock\":20,\"cost\":\"400\"}" >/dev/null
is2xx || { fail "crear lote casa matriz ($(code))"; exit 1; }
gp POST /api/v1/batches \
  "{\"product\":\"${PID}\",\"branch\":\"${BR_B}\",\"batch_code\":\"L-CENTRO\",\"expiry_date\":\"$(exp_in 10)\",\"stock\":20,\"cost\":\"400\"}" >/dev/null
is2xx || { fail "crear lote sucursal B ($(code))"; exit 1; }
ok "lotes: L-CASA vence 90d (casa matriz) · L-CENTRO vence 10d (B)"

[[ "$(qty_at "${PID}" none)" == 20 ]] || fail "casa matriz debería tener 20"
[[ "$(qty_at "${PID}" "${BR_B}")" == 20 ]] || fail "sucursal B debería tener 20"

# 5. LA REGLA: vender en casa matriz NO puede tocar el lote de B ------------
gp POST /api/v1/pos/sale \
  "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"QA FEFO Item\",\"quantity\":5,\"unit_price\":\"2000\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"100000\"}" >/dev/null
is2xx || { fail "venta en casa matriz ($(code))"; exit 1; }

A_CASA=$(batch_stock "${PID}" none L-CASA)
A_CENTRO=$(batch_stock "${PID}" "${BR_B}" L-CENTRO)
if [[ "${A_CASA}" == 15 && "${A_CENTRO}" == 20 ]]; then
  ok "FEFO no cruzó de sucursal: L-CASA 20→15, L-CENTRO intacto en 20 (vence antes y NO se tocó)"
else
  fail "FEFO cruzó de sucursal: L-CASA=${A_CASA} (esperado 15), L-CENTRO=${A_CENTRO} (esperado 20)"
fi
assert_cuadra "${PID}" none "casa matriz"
assert_cuadra "${PID}" "${BR_B}" "sucursal B"

# 6. Aislamiento: B no vende lo que está en casa matriz ---------------------
# B tiene 20; pedimos 25 → el stock global (35) alcanza, el de B no.
OUT=$(gp POST /api/v1/pos/sale \
  "{\"branch\":\"${BR_B}\",\"items\":[{\"product\":\"${PID}\",\"product_name\":\"QA FEFO Item\",\"quantity\":25,\"unit_price\":\"2000\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"100000\"}")
if ! is2xx; then
  ok "B no puede vender el stock de casa matriz (rechazo $(code)): $(echo "${OUT}" | J -r '.error.message // .message // "sin mensaje"')"
else
  fail "B vendió 25 teniendo 20 — el aislamiento por sucursal no se aplicó"
fi

# 7. Recepción de mercadería apunta a UNA sucursal --------------------------
SUP_OUT=$(gp POST /api/v1/suppliers '{"name":"Drogueria QA","rut":"76.123.456-7"}')
SID=$(echo "${SUP_OUT}" | J -r '.id' 2>/dev/null)
[[ -n "${SID}" && "${SID}" != "null" ]] || { fail "crear proveedor ($(code)): ${SUP_OUT}"; exit 1; }

PO=$(gp POST /api/v1/purchase-orders \
  "{\"supplier\":\"${SID}\",\"branch\":\"${BR_B}\",\"items\":[{\"product\":\"${PID}\",\"product_name\":\"QA FEFO Item\",\"quantity\":30,\"unit_cost\":\"400\"}]}")
PO_ID=$(echo "${PO}" | J -r '.id')
PO_LINE=$(echo "${PO}" | J -r '.items[0].id')
PO_BR=$(echo "${PO}" | J -r '.branch')
[[ "${PO_BR}" == "${BR_B}" ]] || fail "la OC no guardó su sucursal (got ${PO_BR})"

gp POST "/api/v1/purchase-orders/${PO_ID}/send" '' >/dev/null
is2xx || { fail "enviar OC ($(code))"; exit 1; }

CASA_ANTES=$(qty_at "${PID}" none)
gp POST "/api/v1/purchase-orders/${PO_ID}/receive" \
  "{\"lines\":[{\"po_line_id\":\"${PO_LINE}\",\"qty_received\":30,\"lot\":\"L-RECIBIDO\",\"expiry_date\":\"$(exp_in 200)\"}]}" >/dev/null
is2xx || { fail "recibir mercadería ($(code))"; exit 1; }

B_DESPUES=$(qty_at "${PID}" "${BR_B}")
CASA_DESPUES=$(qty_at "${PID}" none)
if [[ "${B_DESPUES}" == 50 && "${CASA_DESPUES}" == "${CASA_ANTES}" ]]; then
  ok "recepción en B subió sólo B: 20→50; casa matriz sin cambios en ${CASA_DESPUES}"
else
  fail "recepción se desparramó: B=${B_DESPUES} (esperado 50), casa matriz ${CASA_ANTES}→${CASA_DESPUES}"
fi
REC_EN_B=$(batch_stock "${PID}" "${BR_B}" L-RECIBIDO)
REC_EN_CASA=$(batch_stock "${PID}" none L-RECIBIDO)
if [[ "${REC_EN_B}" == 30 && -z "${REC_EN_CASA}" ]]; then
  ok "el lote L-RECIBIDO nació en B (30u) y NO existe en casa matriz"
else
  fail "lote mal domiciliado: en B=${REC_EN_B} (esperado 30), en casa matriz=${REC_EN_CASA:-<ninguno>} (esperado ninguno)"
fi
assert_cuadra "${PID}" "${BR_B}" "sucursal B"

# 8. B vende contra lo que acaba de recibir (FEFO local: L-CENTRO vence antes)
gp POST /api/v1/pos/sale \
  "{\"branch\":\"${BR_B}\",\"items\":[{\"product\":\"${PID}\",\"product_name\":\"QA FEFO Item\",\"quantity\":20,\"unit_price\":\"2000\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"100000\"}" >/dev/null
is2xx || { fail "venta en B tras recepción ($(code))"; exit 1; }

F_CENTRO=$(batch_stock "${PID}" "${BR_B}" L-CENTRO)
F_RECIBIDO=$(batch_stock "${PID}" "${BR_B}" L-RECIBIDO)
if [[ "${F_CENTRO}" == 0 && "${F_RECIBIDO}" == 30 ]]; then
  ok "FEFO dentro de B: agotó L-CENTRO (vence 10d) antes de tocar L-RECIBIDO (vence 200d)"
else
  fail "FEFO local mal ordenado: L-CENTRO=${F_CENTRO} (esperado 0), L-RECIBIDO=${F_RECIBIDO} (esperado 30)"
fi
assert_cuadra "${PID}" "${BR_B}" "sucursal B"
assert_cuadra "${PID}" none "casa matriz"

# --- veredicto --------------------------------------------------------------
echo
if [[ "${FAILS}" -eq 0 ]]; then
  echo "== FEFO por sucursal: TODO VERDE =="
  exit 0
else
  echo "== FEFO por sucursal: ${FAILS} FALLA(S) =="
  exit 1
fi

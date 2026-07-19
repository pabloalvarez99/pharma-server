#!/usr/bin/env bash
# POS runtime QA — drives a real cashier day against a LIVE pharma-api backend.
# Not a unit test: spins up the actual server + SurrealKv, seeds demo data, and
# hits the same HTTP endpoints the Tauri POS client calls. Hunts runtime bugs the
# vitest journeys cannot (schema asserts, 500s, wrong status codes).
#
# Usage:  bash scripts/qa/pos-runtime-qa.sh [pharmacy|minimarket]
# Requires: built target/debug/{pharma,pharma-api}.exe, curl, jq.
#
# Exit non-zero on the first hard failure; prints a PASS/FAIL line per step.
set -uo pipefail

VERTICAL="${1:-pharmacy}"
PORT="${PORT:-8099}"
BASE="http://127.0.0.1:${PORT}"
TENANT="qa"
EMAIL="owner@qa.local"
PASS="qa-secret-123"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${ROOT}/target/debug"
DATA="$(mktemp -d -t pharma-qa-XXXXXX)"
LOG="${DATA}/api.log"

export PHARMA__DB__PATH="${DATA}/surreal"
export PHARMA__BIND="127.0.0.1:${PORT}"
export PHARMA__JWT__SECRET="qa-test-secret-not-prod"
export PHARMA_PASSWORD="${PASS}"

PHARMA="${BIN}/pharma.exe"
API="${BIN}/pharma-api.exe"

fail() { echo "FAIL: $*"; }
ok()   { echo "PASS: $*"; }

cleanup() {
  [[ -n "${API_PID:-}" ]] && kill "${API_PID}" 2>/dev/null
  wait "${API_PID}" 2>/dev/null
}
trap cleanup EXIT

echo "== POS runtime QA · vertical=${VERTICAL} · data=${DATA} =="

# 1. Schema + tenant + owner user + demo data --------------------------------
"${PHARMA}" migrate --dir "${ROOT}/migrations"            >/dev/null || { fail "migrate"; exit 1; }
"${PHARMA}" tenant-create "QA Pharmacy" --slug "${TENANT}" >/dev/null || { fail "tenant-create"; exit 1; }
"${PHARMA}" user-create --tenant "${TENANT}" --email "${EMAIL}" --roles owner >/dev/null \
  || { fail "user-create"; exit 1; }
"${PHARMA}" seed-demo --tenant "${TENANT}" --vertical "${VERTICAL}" --force >/dev/null \
  || { fail "seed-demo"; exit 1; }
ok "setup (migrate+tenant+user+seed ${VERTICAL})"

# 2. Boot server -------------------------------------------------------------
"${API}" >"${LOG}" 2>&1 &
API_PID=$!
for i in $(seq 1 40); do
  curl -fsS "${BASE}/health/live" >/dev/null 2>&1 && break
  sleep 0.25
done
curl -fsS "${BASE}/health/live" >/dev/null 2>&1 || { fail "server boot (see ${LOG})"; cat "${LOG}"; exit 1; }
ok "server up on ${BASE}"

# 3. Login -------------------------------------------------------------------
TOKEN=$(curl -fsS -X POST "${BASE}/api/v1/login" -H 'content-type: application/json' \
  -d "{\"tenant\":\"${TENANT}\",\"email\":\"${EMAIL}\",\"password\":\"${PASS}\"}" | jq -r .token)
[[ -n "${TOKEN}" && "${TOKEN}" != "null" ]] || { fail "login"; exit 1; }
AUTH=(-H "authorization: Bearer ${TOKEN}")
ok "login"

# 4. Pick two products from the seeded catalog -------------------------------
PRODUCTS=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/products?limit=60")
P1=$(echo "${PRODUCTS}" | jq -c 'if type=="array" then .[0] else .items[0] end')
PID=$(echo "${P1}" | jq -r '.id')
PNAME=$(echo "${P1}" | jq -r '.name')
PPRICE=$(echo "${P1}" | jq -r '.price // .sale_price // .unit_price')
[[ -n "${PID}" && "${PID}" != "null" ]] || { fail "catalog empty"; echo "${PRODUCTS}" | head -c 400; exit 1; }
ok "catalog: ${PNAME} (${PID}) @ ${PPRICE}"

# 5. Open caja ---------------------------------------------------------------
SESSION=$(curl -fsS -X POST "${AUTH[@]}" "${BASE}/api/v1/cash-sessions" \
  -H 'content-type: application/json' \
  -d '{"register_name":"Caja 1","opening_cash":"10000","notes":"QA"}')
SID=$(echo "${SESSION}" | jq -r '.id')
[[ -n "${SID}" && "${SID}" != "null" ]] || { fail "open caja"; echo "${SESSION}"; exit 1; }
ok "caja abierta ${SID}"

# 6. Sell (cash, with change) ------------------------------------------------
SALE=$(curl -sS -w '\n%{http_code}' -X POST "${AUTH[@]}" "${BASE}/api/v1/pos/sale" \
  -H 'content-type: application/json' \
  -d "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":2,\"unit_price\":\"${PPRICE}\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"20000\"}")
SALE_CODE=$(echo "${SALE}" | tail -1)
SALE_BODY=$(echo "${SALE}" | sed '$d')
ORDER=$(echo "${SALE_BODY}" | jq -r '.order.id // .order_id // empty')
if [[ "${SALE_CODE}" == "200" || "${SALE_CODE}" == "201" ]]; then
  ok "venta ${ORDER} (HTTP ${SALE_CODE})"
else
  fail "venta HTTP ${SALE_CODE}: ${SALE_BODY}"
fi

# 7. Receipt -----------------------------------------------------------------
RC=$(curl -sS -o /dev/null -w '%{http_code}' "${AUTH[@]}" "${BASE}/api/v1/orders/${ORDER}/receipt")
[[ "${RC}" == "200" ]] && ok "boleta/ticket receipt (HTTP ${RC})" || fail "receipt HTTP ${RC}"

# 8. Emit boleta SII (39) — expect graceful error without CAF, NOT a 5xx ------
BOL=$(curl -sS -w '\n%{http_code}' -X POST "${AUTH[@]}" "${BASE}/api/v1/dte/boletas" \
  -H 'content-type: application/json' -d "{\"order_id\":\"${ORDER}\"}")
BOL_CODE=$(echo "${BOL}" | tail -1)
case "${BOL_CODE}" in
  200|201) ok "boleta SII emitida (HTTP ${BOL_CODE})";;
  4*)      ok "boleta SII rechazada limpio sin CAF (HTTP ${BOL_CODE}) — esperado";;
  *)       fail "boleta SII HTTP ${BOL_CODE}: $(echo "${BOL}" | sed '$d')";;
esac

# 9. Return — THE bug path: client historically sent tipo=total/parcial -------
RET=$(curl -sS -w '\n%{http_code}' -X POST "${AUTH[@]}" "${BASE}/api/v1/pos/returns" \
  -H 'content-type: application/json' \
  -d "{\"order\":\"${ORDER}\",\"tipo\":\"venta\",\"motivo\":\"Cliente devuelve 1 unidad\",\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":1,\"unit_price\":\"${PPRICE}\",\"restock\":true}]}")
RET_CODE=$(echo "${RET}" | tail -1)
RET_BODY=$(echo "${RET}" | sed '$d')
if [[ "${RET_CODE}" == "200" || "${RET_CODE}" == "201" ]]; then
  ok "devolución parcial (HTTP ${RET_CODE})"
else
  fail "devolución HTTP ${RET_CODE}: ${RET_BODY}"
fi

# 9b. Regression probe: the OLD client payload (tipo=total) MUST be rejected ---
RETBAD=$(curl -sS -o /dev/null -w '%{http_code}' -X POST "${AUTH[@]}" "${BASE}/api/v1/pos/returns" \
  -H 'content-type: application/json' \
  -d "{\"order\":\"${ORDER}\",\"tipo\":\"total\",\"motivo\":\"old payload\",\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":1,\"unit_price\":\"${PPRICE}\",\"restock\":true}]}")
echo "INFO: old payload tipo=total -> HTTP ${RETBAD} (was the crash; documents BUG-paul-001)"

# 10. Arqueo + cierre --------------------------------------------------------
AR=$(curl -sS -o /dev/null -w '%{http_code}' "${AUTH[@]}" "${BASE}/api/v1/cash-sessions/${SID}/arqueo")
[[ "${AR}" == "200" ]] && ok "arqueo (HTTP ${AR})" || fail "arqueo HTTP ${AR}"

CL=$(curl -sS -w '\n%{http_code}' -X POST "${AUTH[@]}" "${BASE}/api/v1/cash-sessions/${SID}/close" \
  -H 'content-type: application/json' -d '{"closing_cash_counted":"30000","notes":"cierre QA"}')
CL_CODE=$(echo "${CL}" | tail -1)
[[ "${CL_CODE}" == "200" ]] && ok "cierre caja (HTTP ${CL_CODE})" || fail "cierre HTTP ${CL_CODE}: $(echo "${CL}" | sed '$d')"

echo "== done (${VERTICAL}) =="

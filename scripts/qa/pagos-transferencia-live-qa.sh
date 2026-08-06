#!/usr/bin/env bash
# Pagos V4 slice 1 — QA en VIVO contra un pharma-api real (migración 0043).
#
# Lo que valida, con plata real en un server real:
#   1. El tender `pos_transferencia` PERSISTE (guardián del whitelist: sin el
#      `DEFINE FIELD OVERWRITE` de 0043 la venta ni se guarda).
#   2. INVARIANTE DEL VECTOR: la venta por transferencia NO entra al efectivo
#      esperado del arqueo — el cajón sólo espera lo que se cobró en efectivo.
#   3. El cierre cuadra contando SÓLO el efectivo real (discrepancia 0).
#   4. El reporte por método de pago reparte cada peso a su bucket (efectivo /
#      tarjeta / transferencia / fiado) y no pierde plata en el camino.
#
# Usage:  bash scripts/qa/pagos-transferencia-live-qa.sh
# Requires: target/debug/{pharma,pharma-api}.exe, curl, jq.
# NOTE: jq.exe en Windows emite CRLF — todo valor pasa por `J` (tr -d '\r').
# Exit non-zero si algo falla. Imprime PASS/FAIL por paso.
set -uo pipefail

PORT="${PORT:-8104}"
BASE="http://127.0.0.1:${PORT}"
TENANT="pagosqa"
EMAIL="owner@pagosqa.local"
PASS="qa-secret-123"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${ROOT}/target/debug"
DATA="$(mktemp -d -t pharma-pagosqa-XXXXXX)"
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
cleanup() { [[ -n "${API_PID:-}" ]] && kill "${API_PID}" 2>/dev/null; wait "${API_PID}" 2>/dev/null; }
trap cleanup EXIT

echo "== pagos: transferencia · QA en vivo · data=${DATA} =="

# 1. Esquema + tenant + owner -----------------------------------------------
"${PHARMA}" migrate --dir "${ROOT}/migrations" >/dev/null || { fail "migrate"; exit 1; }
"${PHARMA}" tenant-create "Pagos QA" --slug "${TENANT}" >/dev/null || { fail "tenant-create"; exit 1; }
"${PHARMA}" user-create --tenant "${TENANT}" --email "${EMAIL}" --roles owner >/dev/null \
  || { fail "user-create"; exit 1; }
ok "setup (migraciones incluida 0043 + tenant + owner)"

# 2. Server + login ----------------------------------------------------------
"${API}" >"${LOG}" 2>&1 &
API_PID=$!
for _ in $(seq 1 60); do curl -fsS "${BASE}/health/live" >/dev/null 2>&1 && break; sleep 0.25; done
curl -fsS "${BASE}/health/live" >/dev/null 2>&1 || { fail "server boot"; tail -20 "${LOG}"; exit 1; }
TOKEN=$(curl -fsS -X POST "${BASE}/api/v1/login" -H 'content-type: application/json' \
  -d "{\"tenant\":\"${TENANT}\",\"email\":\"${EMAIL}\",\"password\":\"${PASS}\"}" | J -r .token)
[[ -n "${TOKEN}" && "${TOKEN}" != "null" ]] || { fail "login"; exit 1; }
AUTH=(-H "authorization: Bearer ${TOKEN}")
ok "server arriba + login"

CODEF="${DATA}/.httpcode"
gp() {
  local method="$1" path="$2" body="${3:-}" resp
  if [[ -n "${body}" ]]; then
    resp=$(curl -sS -w '\n%{http_code}' -X "${method}" "${AUTH[@]}" "${BASE}${path}" \
      -H 'content-type: application/json' \
      -H "Idempotency-Key: $(uuidgen 2>/dev/null || echo "k-${RANDOM}${RANDOM}")" -d "${body}")
  else
    resp=$(curl -sS -w '\n%{http_code}' -X "${method}" "${AUTH[@]}" "${BASE}${path}")
  fi
  echo "${resp}" | tail -1 | tr -d '\r ' > "${CODEF}"
  echo "${resp}" | sed '$d'
}
code() { cat "${CODEF}" 2>/dev/null; }
is2xx() { [[ "$(code)" == 2* ]]; }

# 3. Caja abierta con $10.000 + producto ------------------------------------
SES=$(gp POST /api/v1/cash-sessions '{"register_name":"caja-1","opening_cash":"10000"}' | J -r '.id')
[[ -n "${SES}" && "${SES}" != "null" ]] || { fail "abrir caja ($(code))"; exit 1; }
PID=$(gp POST /api/v1/products '{"name":"QA Pan","price":"2000","stock":100}' | J -r '.id')
[[ -n "${PID}" && "${PID}" != "null" ]] || { fail "crear producto ($(code))"; exit 1; }
ok "caja ${SES} abierta con 10000 · producto ${PID}"

venta() { # venta <metodo> <qty> [json extra]
  local metodo="$1" qty="$2" extra="${3:-}"
  gp POST /api/v1/pos/sale \
    "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"QA Pan\",\"quantity\":${qty},\"unit_price\":\"2000\"}],\"payment_method\":\"${metodo}\"${extra}}"
}

# 4. Venta EN EFECTIVO de 2000 ----------------------------------------------
venta pos_cash 1 ',"cash_amount":"2000"' >/dev/null
is2xx || { fail "venta en efectivo ($(code))"; exit 1; }
ok "venta en efectivo 2000"

# 5. Venta POR TRANSFERENCIA de 6000 (el tender nuevo) -----------------------
TR=$(venta pos_transferencia 3)
if is2xx; then
  MET=$(echo "${TR}" | J -r '.order.payment_method')
  [[ "${MET}" == "pos_transferencia" ]] \
    && ok "el tender persiste tal cual se cobró (${MET}) — whitelist 0043 vivo" \
    || fail "el tender no persistió: '${MET}'"
else
  fail "venta por transferencia rechazada ($(code)): ${TR}"
  exit 1
fi

# 6. INVARIANTE: el arqueo NO cuenta la transferencia ------------------------
ARQ=$(gp GET "/api/v1/cash-sessions/${SES}/arqueo")
CASH_SALES=$(echo "${ARQ}" | J -r '.cash_sales')
ESPERADO=$(echo "${ARQ}" | J -r '.session.closing_cash_expected')
if [[ "${CASH_SALES}" == "2000" && "${ESPERADO}" == "12000" ]]; then
  ok "arqueo: ventas en efectivo=2000 · esperado en el cajón=12000 (10000 apertura + 2000) — la transferencia NO infla la caja"
else
  fail "arqueo contaminado: cash_sales=${CASH_SALES} (esperado 2000), esperado=${ESPERADO} (esperado 12000)"
fi

# 7. El cierre cuadra contando sólo el efectivo real -------------------------
CLOSE=$(gp POST "/api/v1/cash-sessions/${SES}/close" '{"closing_cash_counted":"12000"}')
DISC=$(echo "${CLOSE}" | J -r '.session.discrepancia')
if [[ "${DISC}" == "0" ]]; then
  ok "cierre con 12000 contados → discrepancia 0 (cero faltante fantasma)"
else
  fail "cierre descuadrado: discrepancia=${DISC} (esperado 0)"
fi

# 8. Reporte por método de pago ---------------------------------------------
REP=$(gp GET "/api/v1/reports/sales-by-method")
R_EFE=$(echo "${REP}" | J -r '.[] | select(.method=="efectivo") | .amount')
R_TRA=$(echo "${REP}" | J -r '.[] | select(.method=="transferencia") | .amount')
R_TOT=$(echo "${REP}" | J -r '[.[].amount | tonumber] | add')
if [[ "${R_EFE}" == "2000" && "${R_TRA}" == "6000" && "${R_TOT}" == "8000" ]]; then
  ok "reporte por método: efectivo 2000 · transferencia 6000 · total 8000 (no se pierde un peso)"
else
  fail "reporte descuadrado: efectivo=${R_EFE} (2000), transferencia=${R_TRA} (6000), total=${R_TOT} (8000)"
fi

# --- veredicto --------------------------------------------------------------
echo
if [[ "${FAILS}" -eq 0 ]]; then
  echo "== Pagos / transferencia: TODO VERDE =="
  exit 0
else
  echo "== Pagos / transferencia: ${FAILS} FALLA(S) =="
  exit 1
fi

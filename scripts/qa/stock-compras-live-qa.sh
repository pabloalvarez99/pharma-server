#!/usr/bin/env bash
# Stock & compras runtime QA — drives the goods-in / money-out half of the ERP
# against a LIVE pharma-api backend (real server + SurrealKv + seed-demo), the
# same HTTP endpoints the Tauri inventory/compras/gastos views call.
#
# Not a unit test: it validates the NUMBERS in vivo end-to-end —
#   1. Purchase-order lifecycle: create(draft) → send → receive (the
#      draft→sent transition is BUG-bob-002; receiving a draft must 409).
#   2. WAC (weighted-average cost) recomputed byte-for-byte vs hand math:
#      10u@$100 then 5u@$160 → cost_price = $120. Partial + over-receipt.
#   3. Stock reconciles: product.stock == Σ product_batch.stock ==
#      Σ stock_movement.delta — before and after a FEFO sale.
#   4. Gasto en efectivo con caja → retiro reflejado en arqueo/cierre.
#   5. near-expiry / reorder feeds return sane numbers on realistic seed.
#
# Usage:  bash scripts/qa/stock-compras-live-qa.sh [pharmacy|minimarket]
# Requires: built target/debug/{pharma,pharma-api}.exe, curl, jq, awk.
# NOTE: jq.exe on Windows emits CRLF — every jq value is piped through
#       `tr -d '\r'` (the `J` helper) so record-ids carry no control chars.
# Exit non-zero if any check fails. Prints PASS/FAIL per step.
set -uo pipefail

VERTICAL="${1:-pharmacy}"
PORT="${PORT:-8097}"
BASE="http://127.0.0.1:${PORT}"
TENANT="qa"
EMAIL="owner@qa.local"
PASS="qa-secret-123"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${ROOT}/target/debug"
DATA="$(mktemp -d -t pharma-scqa-XXXXXX)"
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
# jq that strips the CRLF jq.exe appends (keeps record-ids clean for re-use).
J() { jq "$@" | tr -d '\r'; }
# numeric (decimal) equality via awk, tolerant of "120" vs "120.00".
deceq() { awk -v a="$1" -v b="$2" 'BEGIN{exit !(a+0==b+0)}'; }

cleanup() {
  [[ -n "${API_PID:-}" ]] && kill "${API_PID}" 2>/dev/null
  wait "${API_PID}" 2>/dev/null
}
trap cleanup EXIT

echo "== stock/compras runtime QA · vertical=${VERTICAL} · data=${DATA} =="

# 1. Schema + tenant + owner + demo data ------------------------------------
"${PHARMA}" migrate --dir "${ROOT}/migrations"             >/dev/null || { fail "migrate"; exit 1; }
"${PHARMA}" tenant-create "QA Stock" --slug "${TENANT}"     >/dev/null || { fail "tenant-create"; exit 1; }
"${PHARMA}" user-create --tenant "${TENANT}" --email "${EMAIL}" --roles owner >/dev/null \
  || { fail "user-create"; exit 1; }
"${PHARMA}" seed-demo --tenant "${TENANT}" --vertical "${VERTICAL}" --force >/dev/null \
  || { fail "seed-demo"; exit 1; }
ok "setup (migrate+tenant+user+seed ${VERTICAL})"

# 2. Boot server + login -----------------------------------------------------
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

# curl helpers --------------------------------------------------------------
# gp <method> <path> [json] -> echoes body; writes HTTP status to ${CODEF}.
# A file (not a var) because gp is usually called inside $() — a subshell
# can't export a status back to the parent, but a file write survives.
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

# 3. Fresh supplier + product (controlled WAC numbers) -----------------------
SUP=$(gp POST /api/v1/suppliers '{"name":"Proveedor QA","rut":"76.123.456-7"}')
SID=$(echo "${SUP}" | J -r '.id')
[[ -n "${SID}" && "${SID}" != "null" ]] || { fail "create supplier ($(code)): ${SUP}"; exit 1; }
ok "proveedor ${SID}"

PROD=$(gp POST /api/v1/products '{"name":"QA WAC Item","price":"2000","stock":0}')
PID=$(echo "${PROD}" | J -r '.id')
[[ -n "${PID}" && "${PID}" != "null" ]] || { fail "create product ($(code)): ${PROD}"; exit 1; }
ok "producto ${PID} (stock 0, sin costo)"

exp_in() { date -u -d "+$1 days" +%Y-%m-%dT%H:%M:%SZ; }

# new_po <qty> <unit_cost> -> echoes "po_id line_id"
new_po() {
  local qty="$1" cost="$2" po
  po=$(gp POST /api/v1/purchase-orders \
    "{\"supplier\":\"${SID}\",\"items\":[{\"product\":\"${PID}\",\"product_name\":\"QA WAC Item\",\"quantity\":${qty},\"unit_cost\":\"${cost}\"}]}")
  echo "${po}" | J -r '"\(.id) \(.items[0].id)"'
}

# 4. Lifecycle: draft receive must 409, then send unblocks ------------------
read -r PO_A LINE_A < <(new_po 10 100)
ST=$(gp GET "/api/v1/purchase-orders/${PO_A}" | J -r '.status')
[[ "${ST}" == "draft" ]] && ok "OC creada en draft" || fail "OC status=${ST} (esperaba draft)"

gp POST "/api/v1/purchase-orders/${PO_A}/receive" \
  "{\"lines\":[{\"po_line_id\":\"${LINE_A}\",\"qty_received\":10}]}" >/dev/null
[[ "$(code)" == "409" ]] && ok "recibir draft → 409 (BUG-bob-002: hay que emitir primero)" \
  || fail "recibir draft HTTP $(code) (esperaba 409)"

SENT=$(gp POST "/api/v1/purchase-orders/${PO_A}/send")
{ [[ "$(code)" == "200" ]] && [[ "$(echo "${SENT}" | J -r .status)" == "sent" ]]; } \
  && ok "POST /send → draft→sent" || fail "send HTTP $(code): ${SENT}"

# 5. Receive full @100 with lot → WAC seeds at 100 --------------------------
EXP_A=$(exp_in 180)
RA=$(gp POST "/api/v1/purchase-orders/${PO_A}/receive" \
  "{\"lines\":[{\"po_line_id\":\"${LINE_A}\",\"qty_received\":10,\"lot\":\"LOTE-A\",\"expiry_date\":\"${EXP_A}\"}]}")
{ [[ "$(code)" == "200" ]] && [[ "$(echo "${RA}" | J -r .status)" == "received" ]]; } \
  && ok "recepción completa PO_A → received" || fail "receive PO_A HTTP $(code): ${RA}"

P=$(gp GET "/api/v1/products/${PID}")
STK=$(echo "${P}" | J -r '.stock'); COST=$(echo "${P}" | J -r '.cost_price')
{ [[ "${STK}" == "10" ]] && deceq "${COST}" 100; } \
  && ok "tras PO_A: stock=10 WAC=${COST} (=100)" || fail "PO_A stock=${STK} WAC=${COST} (esperaba 10/100)"

# 6. Receive 5 @160 → WAC = (10*100 + 5*160)/15 = 120 (byte-a-byte) ---------
read -r PO_B LINE_B < <(new_po 5 160)
gp POST "/api/v1/purchase-orders/${PO_B}/send" >/dev/null
EXP_B=$(exp_in 90)
gp POST "/api/v1/purchase-orders/${PO_B}/receive" \
  "{\"lines\":[{\"po_line_id\":\"${LINE_B}\",\"qty_received\":5,\"lot\":\"LOTE-B\",\"expiry_date\":\"${EXP_B}\"}]}" >/dev/null
P=$(gp GET "/api/v1/products/${PID}")
STK=$(echo "${P}" | J -r '.stock'); COST=$(echo "${P}" | J -r '.cost_price')
{ [[ "${STK}" == "15" ]] && deceq "${COST}" 120; } \
  && ok "WAC byte-a-byte: 10@100 + 5@160 → stock=15 WAC=${COST} (=120)" \
  || fail "WAC stock=${STK} WAC=${COST} (esperaba 15/120)"

# 7. Reconcile: product.stock == Σbatch == Σmovement ------------------------
reconcile() {
  local label="$1" p b m
  p=$(gp GET "/api/v1/products/${PID}" | J -r '.stock')
  b=$(gp GET "/api/v1/batches?product=${PID}" | J '[.[].stock]|add // 0')
  m=$(gp GET "/api/v1/stock-movements?product_id=${PID}&limit=200" | J '[.data[].delta]|add // 0')
  { [[ "${p}" == "${b}" && "${b}" == "${m}" ]]; } \
    && ok "reconcilia ${label}: stock=${p} Σbatch=${b} Σmov=${m}" \
    || fail "reconcilia ${label}: stock=${p} Σbatch=${b} Σmov=${m} (deben coincidir)"
}
reconcile "post-recepción"

# 8. FEFO sale of 3 → consumes earliest-expiry lot (LOTE-B @90d) first.
#    Invariant product.stock == Σbatch == Σmovement must still hold.
gp POST /api/v1/cash-sessions \
  '{"register_name":"Caja 1","opening_cash":"10000","notes":"QA"}' >/dev/null
SESSION=$(gp GET "/api/v1/cash-sessions" | J -r '.[0].id // .items[0].id')
SALE=$(gp POST /api/v1/pos/sale \
  "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"QA WAC Item\",\"quantity\":3,\"unit_price\":\"2000\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"6000\"}")
{ [[ "$(code)" == "200" || "$(code)" == "201" ]]; } && ok "venta FEFO 3u (HTTP $(code))" \
  || fail "venta HTTP $(code): ${SALE}"
P=$(gp GET "/api/v1/products/${PID}" | J -r '.stock')
[[ "${P}" == "12" ]] && ok "stock tras venta = 12" || fail "stock tras venta = ${P} (esperaba 12)"
reconcile "post-venta"

# 9. Over-receipt rejected ---------------------------------------------------
read -r PO_C LINE_C < <(new_po 5 200)
gp POST "/api/v1/purchase-orders/${PO_C}/send" >/dev/null
gp POST "/api/v1/purchase-orders/${PO_C}/receive" \
  "{\"lines\":[{\"po_line_id\":\"${LINE_C}\",\"qty_received\":6}]}" >/dev/null
[[ "$(code)" == "409" ]] && ok "sobre-recepción (6 de 5) → 409" \
  || fail "sobre-recepción HTTP $(code) (esperaba 409)"

# 10. Partial then complete --------------------------------------------------
read -r PO_D LINE_D < <(new_po 10 100)
gp POST "/api/v1/purchase-orders/${PO_D}/send" >/dev/null
RD=$(gp POST "/api/v1/purchase-orders/${PO_D}/receive" \
  "{\"lines\":[{\"po_line_id\":\"${LINE_D}\",\"qty_received\":4}]}")
[[ "$(echo "${RD}" | J -r .status)" == "partially_received" ]] \
  && ok "recepción parcial (4 de 10) → partially_received" || fail "parcial: ${RD}"
RD2=$(gp POST "/api/v1/purchase-orders/${PO_D}/receive" \
  "{\"lines\":[{\"po_line_id\":\"${LINE_D}\",\"qty_received\":6}]}")
[[ "$(echo "${RD2}" | J -r .status)" == "received" ]] \
  && ok "recepción restante (6) → received" || fail "completar: ${RD2}"

# 11. Gasto en efectivo con caja → retiro reflejado en arqueo ---------------
EXP0=$(gp GET "/api/v1/cash-sessions/${SESSION}/arqueo" | J -r '.session.closing_cash_expected')
GE=$(gp POST /api/v1/expenses \
  "{\"category\":\"varios\",\"description\":\"cafe para el local\",\"amount\":\"2500\",\"payment_method\":\"cash\",\"cash_session\":\"${SESSION}\"}")
{ [[ "$(code)" == "201" || "$(code)" == "200" ]]; } || fail "crear gasto HTTP $(code): ${GE}"
A1=$(gp GET "/api/v1/cash-sessions/${SESSION}/arqueo")
EXP1=$(echo "${A1}" | J -r '.session.closing_cash_expected')
OUT1=$(echo "${A1}" | J -r '.movements_out')
DELTA=$(awk -v a="${EXP0}" -v b="${EXP1}" 'BEGIN{print a-b}')
{ deceq "${DELTA}" 2500 && deceq "${OUT1}" 2500; } \
  && ok "gasto efectivo \$2500 → arqueo baja 2500 (exp ${EXP0}→${EXP1}, retiros=${OUT1})" \
  || fail "gasto no impacta caja: exp ${EXP0}→${EXP1} (Δ${DELTA}), retiros=${OUT1} (esperaba Δ2500)"

# 12. near-expiry + reorder feeds sane on seed ------------------------------
NE=$(gp GET "/api/v1/reports/near-expiry?days=60")
NEOK=$(echo "${NE}" | J 'if type=="array" then "y" else "n" end')
{ [[ "$(code)" == "200" && "${NEOK}" == "\"y\"" ]]; } \
  && ok "near-expiry → array (${VERTICAL}, ${VERTICAL}: $(echo "${NE}" | J 'length') lotes)" \
  || fail "near-expiry HTTP $(code): ${NE}"
RO=$(gp GET "/api/v1/inventory/reorder-suggestions")
ROOK=$(echo "${RO}" | J 'if (type=="array") or (.rows|type=="array") or (.data|type=="array") then "y" else "n" end')
{ [[ "$(code)" == "200" && "${ROOK}" == "\"y\"" ]]; } \
  && ok "reorder-suggestions → array" || fail "reorder HTTP $(code): ${RO}"

echo "== done (${VERTICAL}) · FAILS=${FAILS} =="
[[ "${FAILS}" -eq 0 ]] || exit 1

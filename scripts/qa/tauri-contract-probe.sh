#!/usr/bin/env bash
# Tauri IPC contract probe — the cashier-owned surfaces (devolución · caja · clientes).
#
# The Tauri `invoke` commands in client/src-tauri/src/lib.rs deserialize each API
# response into a fixed serde struct. Any REQUIRED (non-Option) field the server
# stops sending makes `invoke()` throw in the real window — a failure the vitest
# journeys cannot see because they mock `invoke`. This probe boots a live
# pharma-api, drives the cashier surfaces, and asserts every required key is
# present (and non-null) in the raw JSON, against the struct field sets in lib.rs.
#
# It is the §3c step of scripts/qa/tauri-smoke.md: run it after a build, before
# driving the desktop window, to fail fast on serde drift.
#
# Usage:  bash scripts/qa/tauri-contract-probe.sh [pharmacy|minimarket]
# Requires: built target/debug/{pharma,pharma-api}.exe, curl, jq.
#
# Runs ALL checks (does not stop on first failure) to surface multiple drifts;
# exits non-zero if any required key is missing. PASS/FAIL/INFO per check.
set -uo pipefail

VERTICAL="${1:-pharmacy}"
PORT="${PORT:-8098}"
BASE="http://127.0.0.1:${PORT}"
TENANT="ctqa"
EMAIL="owner@ctqa.local"
PASS="ctqa-secret-123"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${ROOT}/target/debug"
DATA="$(mktemp -d -t pharma-ctqa-XXXXXX)"
LOG="${DATA}/api.log"

export PHARMA__DB__PATH="${DATA}/surreal"
export PHARMA__BIND="127.0.0.1:${PORT}"
export PHARMA__JWT__SECRET="ctqa-test-secret-not-prod"
export PHARMA_PASSWORD="${PASS}"

PHARMA="${BIN}/pharma.exe"
API="${BIN}/pharma-api.exe"

FAILS=0
fail() { echo "FAIL: $*"; FAILS=$((FAILS + 1)); }
ok()   { echo "PASS: $*"; }
info() { echo "INFO: $*"; }

# require_keys <json> <label> <key...> — every key must be present AND non-null.
# This mirrors a serde struct with non-Option fields: a missing/null required key
# is exactly what makes the Tauri command's deserialize step throw.
require_keys() {
  local json="$1" label="$2"; shift 2
  local missing=""
  for k in "$@"; do
    local present
    present=$(echo "${json}" | jq -r --arg k "$k" 'has($k) and (.[$k] != null)' 2>/dev/null)
    [[ "${present}" == "true" ]] || missing="${missing} ${k}"
  done
  if [[ -z "${missing}" ]]; then
    ok "${label} — todas las claves requeridas presentes"
  else
    fail "${label} — claves requeridas FALTANTES/null:${missing}"
    echo "${json}" | jq -c . 2>/dev/null | head -c 400; echo
  fi
}

cleanup() {
  [[ -n "${API_PID:-}" ]] && kill "${API_PID}" 2>/dev/null
  wait "${API_PID}" 2>/dev/null
}
trap cleanup EXIT

echo "== Tauri IPC contract probe · vertical=${VERTICAL} · data=${DATA} =="

# --- 1. setup ----------------------------------------------------------------
"${PHARMA}" migrate --dir "${ROOT}/migrations"             >/dev/null || { fail "migrate"; exit 1; }
"${PHARMA}" tenant-create "Contract QA" --slug "${TENANT}" >/dev/null || { fail "tenant-create"; exit 1; }
"${PHARMA}" user-create --tenant "${TENANT}" --email "${EMAIL}" --roles owner >/dev/null \
  || { fail "user-create"; exit 1; }
"${PHARMA}" seed-demo --tenant "${TENANT}" --vertical "${VERTICAL}" --force >/dev/null \
  || { fail "seed-demo"; exit 1; }
ok "setup (migrate+tenant+user+seed ${VERTICAL})"

# --- 2. boot + login ---------------------------------------------------------
"${API}" >"${LOG}" 2>&1 &
API_PID=$!
for i in $(seq 1 40); do curl -fsS "${BASE}/health/live" >/dev/null 2>&1 && break; sleep 0.25; done
curl -fsS "${BASE}/health/live" >/dev/null 2>&1 || { fail "server boot"; cat "${LOG}"; exit 1; }
TOKEN=$(curl -fsS -X POST "${BASE}/api/v1/login" -H 'content-type: application/json' \
  -d "{\"tenant\":\"${TENANT}\",\"email\":\"${EMAIL}\",\"password\":\"${PASS}\"}" | jq -r .token)
[[ -n "${TOKEN}" && "${TOKEN}" != "null" ]] || { fail "login"; exit 1; }
AUTH=(-H "authorization: Bearer ${TOKEN}")
ok "server up + login"

# pick a sellable product
P1=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/products?limit=80" \
  | jq -c '(if type=="array" then . else .items end)
    | map(select((.stock // .stock_qty // 0) >= 4 and ((.price // .sale_price // .unit_price)|tonumber) > 500)) | .[0]')
PID=$(echo "${P1}" | jq -r '.id')
PNAME=$(echo "${P1}" | jq -r '.name')
PPRICE=$(printf '%.0f' "$(echo "${P1}" | jq -r '.price // .sale_price // .unit_price')")
[[ -n "${PID}" && "${PID}" != "null" ]] || { fail "no sellable product"; exit 1; }
ok "producto ${PNAME} (${PID}) @ ${PPRICE}"

# === A. CashSession (open) — cash_sessions / open_cash_session struct =========
echo "-- A. CashSession (open) --"
SESS=$(curl -fsS -X POST "${AUTH[@]}" "${BASE}/api/v1/cash-sessions" -H 'content-type: application/json' \
  -d '{"register_name":"Caja Probe","opening_cash":"50000","notes":"probe"}')
SID=$(echo "${SESS}" | jq -r '.id')
require_keys "${SESS}" "CashSession" id user register_name opening_cash opened_at status
[[ -n "${SID}" && "${SID}" != "null" ]] || { fail "open caja"; exit 1; }

# list shape too (cash_sessions returns Vec<CashSession>)
LIST=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/cash-sessions?status=open&limit=50")
require_keys "$(echo "${LIST}" | jq -c '(if type=="array" then . else .items end)[0]')" \
  "CashSession[] (lista)" id user register_name opening_cash opened_at status

# === B. Receipt — get_receipt struct (need an order first) ====================
echo "-- B. Receipt --"
SALE=$(curl -fsS -X POST "${AUTH[@]}" "${BASE}/api/v1/pos/sale" -H 'content-type: application/json' \
  -d "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":2,\"unit_price\":\"${PPRICE}\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"$((PPRICE*2+1000))\"}")
OID=$(echo "${SALE}" | jq -r '.order.id // .order_id // empty')
[[ -n "${OID}" ]] || { fail "venta para boleta"; echo "${SALE}" | head -c 300; }
if [[ -n "${OID}" ]]; then
  RCPT=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/orders/${OID}/receipt")
  require_keys "${RCPT}" "Receipt" order_id folio_or_number datetime tenant_name \
    items subtotal discount total payment_method loyalty_points_awarded footer_note
  require_keys "$(echo "${RCPT}" | jq -c '.items[0]')" "ReceiptItem" name qty unit_price line_total
fi

# === C. CashCloseSummary — cash_arqueo struct (session still open) ============
echo "-- C. CashCloseSummary (arqueo) --"
ARQ=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/cash-sessions/${SID}/arqueo")
require_keys "${ARQ}" "CashCloseSummary" session cash_sales movements_in movements_out
require_keys "$(echo "${ARQ}" | jq -c '.session')" "CashCloseSummary.session" \
  id user register_name opening_cash opened_at status

# === D. Devolucion — create_refund + list_refunds struct ======================
# create_refund returns the RAW RefundResponse wrapper ({devolucion, items,
# stock_movements, order_marked_refunded}) as serde_json::Value — the view ignores
# the body. The flat `Devolucion` struct is what list_refunds (GET /returns)
# deserializes, so assert the wrapper's `.devolucion` sub-object here + the flat
# list below.
echo "-- D. Devolucion --"
if [[ -n "${OID}" ]]; then
  REF=$(curl -fsS -X POST "${AUTH[@]}" "${BASE}/api/v1/pos/returns" -H 'content-type: application/json' \
    -d "{\"order\":\"${OID}\",\"tipo\":\"venta\",\"motivo\":\"probe parcial\",\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":1,\"unit_price\":\"${PPRICE}\",\"restock\":false}]}")
  require_keys "$(echo "${REF}" | jq -c '.devolucion')" "RefundResponse.devolucion (create)" \
    id tipo motivo total_devuelto created_at
fi
DLIST=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/returns?limit=60")
require_keys "$(echo "${DLIST}" | jq -c '(if type=="array" then . else .items end)[0]')" \
  "Devolucion[] (lista)" id tipo motivo total_devuelto created_at

# === E. CashSession (close) — close_cash_session struct =======================
echo "-- E. CashSession (close) --"
CLOSE=$(curl -fsS -X POST "${AUTH[@]}" "${BASE}/api/v1/cash-sessions/${SID}/close" \
  -H 'content-type: application/json' -d '{"closing_cash_counted":"60000","notes":"probe cierre"}')
require_keys "${CLOSE}" "CashCloseSummary (close)" session cash_sales movements_in movements_out
require_keys "$(echo "${CLOSE}" | jq -c '.session')" "CashSession (cerrada)" \
  id user register_name opening_cash opened_at status closing_cash_counted closing_cash_expected

# === F. Customer / CustomerDetail / CustomerOrder — clientes surface ==========
# Degrades like the view: if customers-loyalty isn't merged, search/detail/history
# 404 → the Tauri commands raise CUSTOMERS_MODULE_MISSING. Probe notes it, not fails.
echo "-- F. Clientes + fidelidad --"
CCODE=$(curl -sS -o "${DATA}/cust.json" -w '%{http_code}' -X POST "${AUTH[@]}" "${BASE}/api/v1/clientes" \
  -H 'content-type: application/json' -d '{"name":"Cliente Probe","rut":"11111111-1","phone":"+56900000000"}')
if [[ "${CCODE}" == "404" ]]; then
  info "Clientes surface ausente (404) → vista degrada a CUSTOMERS_MODULE_MISSING (no es bug)"
else
  CUST=$(cat "${DATA}/cust.json")
  CID=$(echo "${CUST}" | jq -r '.id')
  require_keys "${CUST}" "Customer (create)" id name loyalty_points active created_at updated_at
  # sale attributed to the customer → loyalty accrual reflected in detail
  curl -fsS -X POST "${AUTH[@]}" "${BASE}/api/v1/pos/sale" -H 'content-type: application/json' \
    -d "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":1,\"unit_price\":\"${PPRICE}\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"${PPRICE}\",\"customer\":\"${CID}\"}" >/dev/null
  SCODE=$(curl -sS -o "${DATA}/search.json" -w '%{http_code}' "${AUTH[@]}" "${BASE}/api/v1/customers/search?q=Probe")
  if [[ "${SCODE}" == "404" ]]; then
    info "customers/search ausente (404) → vista degrada (no es bug)"
  else
    require_keys "$(jq -c '.[0] // (.items[0])' "${DATA}/search.json")" \
      "Customer[] (search)" id name loyalty_points active created_at updated_at
    DET=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/customers/${CID}")
    require_keys "${DET}" "CustomerDetail" id name loyalty_points total_spent visit_count active created_at updated_at
    HIST=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/customers/${CID}/history?limit=20")
    HROW=$(echo "${HIST}" | jq -c '(if type=="array" then . else .items end)[0] // empty')
    if [[ -n "${HROW}" ]]; then
      require_keys "${HROW}" "CustomerOrder (history)" id status payment_method total items_count created_at
    else
      info "history vacío para el cliente recién creado (acepta — sin fila que validar)"
    fi
  fi
fi

echo "== done (${VERTICAL}) · FAILS=${FAILS} =="
[[ "${FAILS}" -eq 0 ]] || exit 1

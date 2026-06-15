#!/usr/bin/env bash
# POS payments + fidelidad QA — deepens pos-runtime-qa.sh (ola 5).
# Drives the cashier-loop edges #199 did NOT cover, against a LIVE pharma-api:
#   - multi-tender (pos_mixed: efectivo + tarjeta) incl. underpay reject + overpay vuelto
#   - descuento global (clamp anti-negativo) + descuento por línea (unit_price ajustado)
#   - cliente + fidelidad (puntos acumulados == floor(total/regla); ledger; bump exacto)
#   - devolución parcial con restock + RE-VENTA del ítem devuelto (caza FEFO/stock desync)
# Money is CLP integer end-to-end; both verticals.
#
# Usage:  bash scripts/qa/pos-payments-fidelidad-qa.sh [pharmacy|minimarket]
# Requires: built target/debug/{pharma,pharma-api}.exe (cargo build -p api -p cli), curl, jq.
#
# Runs ALL scenarios (does not exit on first failure) to surface multiple bugs;
# exits non-zero if any hard assertion failed. PASS/FAIL/INFO per check.
set -uo pipefail

VERTICAL="${1:-pharmacy}"
PORT="${PORT:-8097}"
BASE="http://127.0.0.1:${PORT}"
TENANT="payqa"
EMAIL="owner@payqa.local"
PASS="payqa-secret-123"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BIN="${ROOT}/target/debug"
DATA="$(mktemp -d -t pharma-payqa-XXXXXX)"
LOG="${DATA}/api.log"

export PHARMA__DB__PATH="${DATA}/surreal"
export PHARMA__BIND="127.0.0.1:${PORT}"
export PHARMA__JWT__SECRET="payqa-test-secret-not-prod"
export PHARMA_PASSWORD="${PASS}"

PHARMA="${BIN}/pharma.exe"
API="${BIN}/pharma-api.exe"

FAILS=0
fail() { echo "FAIL: $*"; FAILS=$((FAILS + 1)); }
ok()   { echo "PASS: $*"; }
info() { echo "INFO: $*"; }
# assert_eq <actual> <expected> <label>
assert_eq() { if [[ "$1" == "$2" ]]; then ok "$3 ($1)"; else fail "$3: esperaba '$2', obtuve '$1'"; fi; }
# assert_ok <httpcode> <label> — sale/return success is 200 OR 201 (Created)
assert_ok() { if [[ "$1" == "200" || "$1" == "201" ]]; then ok "$2 ($1)"; else fail "$2: esperaba 2xx, obtuve '$1'"; fi; }
is_ok() { [[ "$1" == "200" || "$1" == "201" ]]; }

cleanup() {
  [[ -n "${API_PID:-}" ]] && kill "${API_PID}" 2>/dev/null
  wait "${API_PID}" 2>/dev/null
}
trap cleanup EXIT

echo "== POS payments+fidelidad QA · vertical=${VERTICAL} · data=${DATA} =="

# --- 1. setup ----------------------------------------------------------------
"${PHARMA}" migrate --dir "${ROOT}/migrations"             >/dev/null || { fail "migrate"; exit 1; }
"${PHARMA}" tenant-create "Pay QA" --slug "${TENANT}"      >/dev/null || { fail "tenant-create"; exit 1; }
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

# helper: fetch a product row by id from the catalog (fresh stock each call)
prod_field() { # <pid> <jqfield>
  curl -fsS "${AUTH[@]}" "${BASE}/api/v1/products?limit=80" \
    | jq -r --arg id "$1" '(if type=="array" then . else .items end)[]|select(.id==$id)|'"$2"
}

# pick a product with stock >= 6 and a usable price
PRODUCTS=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/products?limit=80")
P1=$(echo "${PRODUCTS}" | jq -c '(if type=="array" then . else .items end)
  | map(select((.stock // .stock_qty // 0) >= 6 and ((.price // .sale_price // .unit_price)|tonumber) > 500))
  | .[0]')
PID=$(echo "${P1}" | jq -r '.id')
PNAME=$(echo "${P1}" | jq -r '.name')
PPRICE=$(printf '%.0f' "$(echo "${P1}" | jq -r '.price // .sale_price // .unit_price')")
[[ -n "${PID}" && "${PID}" != "null" ]] || { fail "no product with stock>=6"; echo "${PRODUCTS}" | head -c 400; exit 1; }
ok "producto ${PNAME} (${PID}) @ ${PPRICE}"

# open caja
SID=$(curl -fsS -X POST "${AUTH[@]}" "${BASE}/api/v1/cash-sessions" -H 'content-type: application/json' \
  -d '{"register_name":"Caja Pay","opening_cash":"50000","notes":"payqa"}' | jq -r '.id')
[[ -n "${SID}" && "${SID}" != "null" ]] || { fail "open caja"; exit 1; }
ok "caja abierta ${SID}"

# convenience: POST a sale, echo "HTTP\nbody"
post_sale() { curl -sS -w '\n%{http_code}' -X POST "${AUTH[@]}" "${BASE}/api/v1/pos/sale" \
  -H 'content-type: application/json' -d "$1"; }

# === SCENARIO 1 — multi-tender pos_mixed (cash + card == total) ==============
echo "-- S1 multi-tender exacto --"
TOTAL1=$((PPRICE * 2))
CASH1=$((TOTAL1 / 2)); CARD1=$((TOTAL1 - CASH1))
R=$(post_sale "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":2,\"unit_price\":\"${PPRICE}\"}],\"payment_method\":\"pos_mixed\",\"cash_amount\":\"${CASH1}\",\"card_amount\":\"${CARD1}\"}")
CODE=$(echo "$R"|tail -1); BODY=$(echo "$R"|sed '$d')
assert_ok "$CODE" "S1 venta mixta HTTP"
if is_ok "$CODE"; then
  assert_eq "$(echo "$BODY"|jq -r '.order.total')"       "$TOTAL1" "S1 total"
  assert_eq "$(echo "$BODY"|jq -r '.order.cash_amount')" "$CASH1"  "S1 cash_amount persistido"
  assert_eq "$(echo "$BODY"|jq -r '.order.card_amount')" "$CARD1"  "S1 card_amount persistido"
  assert_eq "$(echo "$BODY"|jq -r '.order.payment_method')" "pos_mixed" "S1 método"
fi

# === SCENARIO 2 — mixed underpay (cash + card < total) MUST reject ===========
echo "-- S2 multi-tender insuficiente --"
R=$(post_sale "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":2,\"unit_price\":\"${PPRICE}\"}],\"payment_method\":\"pos_mixed\",\"cash_amount\":\"1\",\"card_amount\":\"1\"}")
CODE=$(echo "$R"|tail -1)
if [[ "$CODE" =~ ^4 ]]; then ok "S2 mixto insuficiente rechazado (HTTP ${CODE})"; else fail "S2 mixto insuficiente debió 4xx, fue ${CODE}"; fi

# === SCENARIO 3 — mixed OVERPAY: vuelto en boleta? (finding probe) ===========
echo "-- S3 multi-tender sobrepago / vuelto --"
TOTAL3=$PPRICE
OVER=2000; CASH3=$((TOTAL3 + OVER)); CARD3=0   # all cash, overpaid, declared as mixed
R=$(post_sale "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":1,\"unit_price\":\"${PPRICE}\"}],\"payment_method\":\"pos_mixed\",\"cash_amount\":\"${CASH3}\",\"card_amount\":\"0\"}")
CODE=$(echo "$R"|tail -1); BODY=$(echo "$R"|sed '$d')
if is_ok "$CODE"; then
  OID=$(echo "$BODY"|jq -r '.order.id')
  CHG=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/orders/${OID}/receipt" | jq -r '.change')
  info "S3 sobrepago ${OVER} en pos_mixed → receipt.change=${CHG} (esperado vuelto ${OVER}; null = gap cara-cajero, ver BUG LOG)"
else
  info "S3 pos_mixed all-cash overpay HTTP ${CODE}"
fi

# === SCENARIO 4 — descuento global ==========================================
echo "-- S4 descuento global --"
SUB4=$((PPRICE * 3)); DISC4=$((PPRICE)); EXP4=$((SUB4 - DISC4))
R=$(post_sale "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":3,\"unit_price\":\"${PPRICE}\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"${SUB4}\",\"discount\":\"${DISC4}\"}")
CODE=$(echo "$R"|tail -1); BODY=$(echo "$R"|sed '$d')
assert_ok "$CODE" "S4 venta c/descuento HTTP"
if is_ok "$CODE"; then
  assert_eq "$(echo "$BODY"|jq -r '.order.subtotal')" "$SUB4" "S4 subtotal"
  assert_eq "$(echo "$BODY"|jq -r '.order.discount')" "$DISC4" "S4 descuento"
  assert_eq "$(echo "$BODY"|jq -r '.order.total')"    "$EXP4" "S4 total=subtotal-descuento"
fi

# === SCENARIO 5 — descuento > subtotal: clamp, NUNCA total negativo =========
echo "-- S5 over-descuento clamp --"
SUB5=$PPRICE; HUGE=$((PPRICE * 10))
R=$(post_sale "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":1,\"unit_price\":\"${PPRICE}\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"0\",\"discount\":\"${HUGE}\"}")
CODE=$(echo "$R"|tail -1); BODY=$(echo "$R"|sed '$d')
if is_ok "$CODE"; then
  TOT5=$(echo "$BODY"|jq -r '.order.total')
  if [[ "$TOT5" =~ ^- ]]; then fail "S5 total NEGATIVO ${TOT5} (descuento no clampeado)"; else ok "S5 total clampeado >=0 (${TOT5}, descuento=$(echo "$BODY"|jq -r '.order.discount'))"; fi
else
  info "S5 over-descuento HTTP ${CODE} (rechazo limpio también es válido)"
fi

# === SCENARIO 6 — descuento por línea (unit_price ajustado por el cajero) ====
echo "-- S6 descuento por línea --"
HALF=$((PPRICE / 2))
R=$(post_sale "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":2,\"unit_price\":\"${HALF}\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"${PPRICE}\"}")
CODE=$(echo "$R"|tail -1); BODY=$(echo "$R"|sed '$d')
assert_ok "$CODE" "S6 venta línea-descuento HTTP"
if is_ok "$CODE"; then
  assert_eq "$(echo "$BODY"|jq -r '.order.subtotal')" "$((HALF * 2))" "S6 subtotal usa unit_price ajustado"
fi

# === SCENARIO 7 — cliente + fidelidad =======================================
echo "-- S7 cliente + fidelidad --"
CID=$(curl -fsS -X POST "${AUTH[@]}" "${BASE}/api/v1/clientes" -H 'content-type: application/json' \
  -d '{"name":"Cliente Fiel","rut":"11111111-1","phone":"+56900000000"}' | jq -r '.id')
if [[ -z "${CID}" || "${CID}" == "null" ]]; then
  fail "S7 crear cliente"
else
  PTS0=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/customers/${CID}" | jq -r '.loyalty_points')
  QTY7=5; TOTAL7=$((PPRICE * QTY7)); EXP_PTS=$((TOTAL7 / 1000))
  R=$(post_sale "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":${QTY7},\"unit_price\":\"${PPRICE}\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"${TOTAL7}\",\"customer\":\"${CID}\"}")
  CODE=$(echo "$R"|tail -1); BODY=$(echo "$R"|sed '$d')
  assert_ok "$CODE" "S7 venta c/cliente HTTP"
  if is_ok "$CODE"; then
    AW=$(echo "$BODY"|jq -r '.loyalty_points_awarded')
    assert_eq "$AW" "$EXP_PTS" "S7 puntos otorgados = floor(total/1000)"
    PTS1=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/customers/${CID}" | jq -r '.loyalty_points')
    assert_eq "$PTS1" "$((PTS0 + EXP_PTS))" "S7 customer.loyalty_points bump exacto"
    LEDGER=$(curl -fsS "${AUTH[@]}" "${BASE}/api/v1/loyalty?customer=${CID}" | jq -r 'length')
    if [[ "${LEDGER}" -ge 1 ]]; then ok "S7 ledger fidelidad con ${LEDGER} tx"; else fail "S7 ledger fidelidad vacío"; fi
  fi
fi

# === SCENARIO 8 — devolución parcial + restock + RE-VENTA (FEFO/stock) ======
echo "-- S8 devolución parcial → restock → re-venta --"
ST0=$(prod_field "$PID" '.stock // .stock_qty')
info "S8 stock inicial=${ST0}"
# vender 3
R=$(post_sale "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":3,\"unit_price\":\"${PPRICE}\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"$((PPRICE*3))\"}")
CODE=$(echo "$R"|tail -1); BODY=$(echo "$R"|sed '$d')
OID8=$(echo "$BODY"|jq -r '.order.id // empty')
assert_ok "$CODE" "S8 venta qty3 HTTP"
ST1=$(prod_field "$PID" '.stock // .stock_qty')
assert_eq "$ST1" "$((ST0 - 3))" "S8 stock tras venta -3"
# devolver 1 con restock
R=$(curl -sS -w '\n%{http_code}' -X POST "${AUTH[@]}" "${BASE}/api/v1/pos/returns" -H 'content-type: application/json' \
  -d "{\"order\":\"${OID8}\",\"tipo\":\"venta\",\"motivo\":\"devuelve 1\",\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":1,\"unit_price\":\"${PPRICE}\",\"restock\":true}]}")
CODE=$(echo "$R"|tail -1)
assert_ok "$CODE" "S8 devolución parcial HTTP"
ST2=$(prod_field "$PID" '.stock // .stock_qty')
assert_eq "$ST2" "$((ST0 - 2))" "S8 stock tras restock +1 (=-2)"
# re-vender el ítem devuelto (1)
R=$(post_sale "{\"items\":[{\"product\":\"${PID}\",\"product_name\":\"${PNAME}\",\"quantity\":1,\"unit_price\":\"${PPRICE}\"}],\"payment_method\":\"pos_cash\",\"cash_amount\":\"${PPRICE}\"}")
CODE=$(echo "$R"|tail -1)
assert_ok "$CODE" "S8 re-venta del devuelto HTTP"
ST3=$(prod_field "$PID" '.stock // .stock_qty')
assert_eq "$ST3" "$((ST0 - 3))" "S8 stock tras re-venta (=-3, sin desync FEFO)"

# === cierre ==================================================================
CL=$(curl -sS -o /dev/null -w '%{http_code}' -X POST "${AUTH[@]}" "${BASE}/api/v1/cash-sessions/${SID}/close" \
  -H 'content-type: application/json' -d '{"closing_cash_counted":"60000","notes":"cierre payqa"}')
assert_eq "$CL" "200" "cierre caja"

echo "== done (${VERTICAL}) · FAILS=${FAILS} =="
[[ "${FAILS}" -eq 0 ]] || exit 1

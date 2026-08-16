#!/usr/bin/env bash
# Comprobar post-install de la nube de feria (correr EN la VPS).
# No imprime secretos ni el contenido de api.env.
#
#   sudo bash deploy/nube/comprobar.sh
#   # o, ya copiado: sudo bash /opt/rutbusiness/bin/comprobar.sh

set -euo pipefail

ok=0
warn=0
fail=0

pass() { echo "OK   $*"; ok=$((ok + 1)); }
soft() { echo "WARN $*"; warn=$((warn + 1)); }
bad()  { echo "FAIL $*"; fail=$((fail + 1)); }

BIN=/opt/rutbusiness/bin/pharma-api
ENV_FILE=/etc/rutbusiness/api.env
UNIT_API=/etc/systemd/system/rutbusiness-api.service
UNIT_BACKUP_SVC=/etc/systemd/system/backup-rutbusiness.service
UNIT_BACKUP_TMR=/etc/systemd/system/backup-rutbusiness.timer
BACKUP_SH=/opt/rutbusiness/bin/backup.sh

echo "== rutas =="

if [[ -x "$BIN" ]]; then
	pass "binario $BIN"
elif [[ -e "$BIN" ]]; then
	bad "existe $BIN pero no es ejecutable"
else
	bad "falta $BIN"
fi

if [[ -f "$ENV_FILE" ]]; then
	pass "env $ENV_FILE"
else
	bad "falta $ENV_FILE (copiar de api.env.ejemplo y rellenar)"
fi

for u in "$UNIT_API" "$UNIT_BACKUP_SVC" "$UNIT_BACKUP_TMR"; do
	if [[ -f "$u" ]]; then
		pass "unit $u"
	else
		bad "falta unit $u"
	fi
done

if [[ -x "$BACKUP_SH" ]]; then
	pass "backup $BACKUP_SH"
elif [[ -e "$BACKUP_SH" ]]; then
	soft "existe $BACKUP_SH pero no es ejecutable"
else
	soft "falta $BACKUP_SH (opcional hasta activar timer)"
fi

echo "== api.env (sin volcar valores) =="

if [[ -f "$ENV_FILE" ]]; then
	# Sentinel de plantilla: no mostrar líneas ni secretos.
	if grep -q 'CAMBIAME' "$ENV_FILE" 2>/dev/null; then
		bad "CAMBIAME still present — reemplazar placeholders en $ENV_FILE"
	else
		pass "CAMBIAME no presente"
	fi

	# Bind: solo loopback. No echo del valor completo si es basura; solo veredicto.
	if ! grep -qE '^[[:space:]]*PHARMA__BIND=' "$ENV_FILE" 2>/dev/null; then
		bad "falta clave PHARMA__BIND (debe ser 127.0.0.1:…)"
	elif grep -qE '^[[:space:]]*PHARMA__BIND=127\.0\.0\.1(:[0-9]+)?[[:space:]]*$' "$ENV_FILE" 2>/dev/null; then
		pass "PHARMA__BIND es loopback (127.0.0.1)"
	elif grep -qE '^[[:space:]]*PHARMA__BIND=localhost(:[0-9]+)?[[:space:]]*$' "$ENV_FILE" 2>/dev/null; then
		pass "PHARMA__BIND es loopback (localhost)"
	elif grep -qE '^[[:space:]]*PHARMA__BIND=0\.0\.0\.0' "$ENV_FILE" 2>/dev/null; then
		bad "PHARMA__BIND expone 0.0.0.0 — debe ser 127.0.0.1 (Caddy es el front)"
	else
		bad "PHARMA__BIND no es loopback (esperaba 127.0.0.1 o localhost)"
	fi
fi

echo "== systemd =="

if ! command -v systemctl >/dev/null 2>&1; then
	soft "systemctl no disponible (¿no es systemd?)"
elif [[ "$(id -u)" -ne 0 ]] && ! systemctl show-environment >/dev/null 2>&1; then
	soft "sin privilegios para consultar units — reintentar con sudo"
else
	if systemctl cat rutbusiness-api.service >/dev/null 2>&1; then
		if systemctl is-active --quiet rutbusiness-api.service 2>/dev/null; then
			pass "rutbusiness-api is-active"
		else
			st=$(systemctl is-active rutbusiness-api.service 2>/dev/null || true)
			soft "rutbusiness-api no active (estado: ${st:-unknown}) — ¿enable --now?"
		fi
	else
		soft "unit rutbusiness-api no instalada aún en systemd"
	fi

	if systemctl cat caddy.service >/dev/null 2>&1; then
		if systemctl is-active --quiet caddy.service 2>/dev/null; then
			pass "caddy is-active"
		else
			st=$(systemctl is-active caddy.service 2>/dev/null || true)
			soft "caddy no active (estado: ${st:-unknown})"
		fi
	else
		soft "caddy no instalado o unit ausente (ok si aún no se configuró)"
	fi

	if systemctl cat backup-rutbusiness.timer >/dev/null 2>&1; then
		if systemctl is-enabled --quiet backup-rutbusiness.timer 2>/dev/null \
			|| systemctl is-active --quiet backup-rutbusiness.timer 2>/dev/null; then
			pass "backup-rutbusiness.timer presente/enabled"
		else
			soft "backup-rutbusiness.timer instalado pero no enabled"
		fi
	else
		soft "backup-rutbusiness.timer no instalado aún"
	fi
fi

echo "== resumen: ok=$ok warn=$warn fail=$fail =="

if [[ "$fail" -gt 0 ]]; then
	exit 1
fi
exit 0

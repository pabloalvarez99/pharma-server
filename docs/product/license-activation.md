---
title: License activation — operator guide
status: draft
date: 2026-05-20
audience: operadores (instaladores, soporte, ops)
related:
  - ../strategy/license-architecture.md
  - ../adr/0002-license-ed25519-offline.md
  - ../adr/0005-core-gratis-no-locked-in.md
---

# License activation — operator guide

> Cómo activar, verificar, renovar y resetear una license en un nodo
> pharma-server. Todo offline excepto la descarga inicial del `.lic`.

## TL;DR

```powershell
# 1. Importar el .lic (verifica firma offline, persiste a disk)
pharma license import C:\Users\Administrator\Downloads\farmacia-coquimbo.lic

# 2. Hot-reload en el service corriendo (sin restart)
$token = "<admin-bearer-token>"
curl.exe -X POST `
  -H "Authorization: Bearer $token" `
  http://localhost:8080/api/v1/admin/license/reload

# 3. Verificar
pharma license status
```

---

## 1. Estados posibles

| Estado | Cuándo | Comportamiento |
|---|---|---|
| **active** | `now <= expires_at` (o `tier=free` perpetuo) | Features pagadas OK. Core OK. |
| **grace** | `expires_at < now <= expires_at + 30d` | Features pagadas OK + toast 1×/día. Core OK. |
| **expired** | `now > expires_at + 30d` | Features pagadas → 402. Core sigue OK siempre. |
| **(no license)** | `data/license.json` no existe | Tier Free (default). Core OK. |
| **(invalid)** | firma rota / schema futuro / key_id desconocido | Tier Free (fallback). Logs warning. Core OK. |

Invariante absoluto (ADR-0005): el core gratis **nunca** se bloquea, da
igual el estado del license.

---

## 2. Importar un .lic

El archivo `.lic` llega por email tras el checkout en `pharma-server.cl`
(o en USB para deploys air-gapped).

```powershell
pharma license import <ruta-al-.lic>
```

Qué hace:
1. Lee el archivo.
2. Verifica la firma Ed25519 **offline** contra la pubkey del licenser
   embebida en el binario.
3. Valida `schema_version <= soportada`, regla `expires_at=null⇒Free`,
   `issuer_did` coincide con `key_id`.
4. Persiste pretty-JSON en `<data_dir>/license.json`.

Exit 0 si OK, 1 si firma inválida.

**Importante**: el service en ejecución NO recarga automáticamente la
license. Hay que pedirle un reload (sección 3) o reiniciar el service.

---

## 3. Hot-reload sin reiniciar el service

```powershell
# Obtener un admin/owner token (login normal, o issue dev)
$token = "<bearer>"

curl.exe -X POST `
  -H "Authorization: Bearer $token" `
  http://localhost:8080/api/v1/admin/license/reload
```

Respuesta:

```jsonc
{
  "tier": "pro",
  "status": "active",
  "license_id": "lic_01HX...",
  "features": ["reports.margins_daily", "integrations.sii_dte_auto"],
  "expires_at": "2027-05-20T14:00:00Z",
  "key_id": "lk-2026-01",
  "seat_count": 3
}
```

El swap es atómico (lock-free via `ArcSwap`). Llamadas a endpoints
gated en curso siguen viendo la license vieja hasta que terminen; las
nuevas ven la nueva. Cero downtime.

Si el archivo está roto o ausente: la respuesta es 200 con tier=free
(fallback ADR-0005), NO 5xx. Status del logs lo deja claro.

---

## 4. Verificar estado

CLI (no requiere service corriendo):

```powershell
pharma license status
```

HTTP (consulta el ArcSwap actual, sin tocar disco):

```powershell
curl.exe -H "Authorization: Bearer $token" `
  http://localhost:8080/api/v1/admin/license/status
```

---

## 5. Ver qué features están entitled

```powershell
pharma license features          # uno por línea
pharma license features --json   # array JSON pretty
```

UI/integraciones: usar el array del endpoint `/api/v1/admin/license/status`.

---

## 6. Verificar un .lic sin importarlo (diagnóstico)

```powershell
pharma license verify C:\path\to\file.lic
```

Útil cuando soporte pide "¿este archivo es válido?" antes de aplicar.
No toca disk del nodo.

---

## 7. Exportar la license activa (para bug reports / soporte)

```powershell
pharma license export > active-license.json
```

Soporte pide esto cuando un usuario reporta "el botón X no aparece".
Permite comparar `features` del nodo vs. lo que se le vendió.

---

## 8. Borrar la license (volver a Free)

```powershell
pharma license clear --force
# luego reload o restart
curl.exe -X POST -H "Authorization: Bearer $token" `
  http://localhost:8080/api/v1/admin/license/reload
```

Útil para tests E2E, devoluciones de venta cancelada, o resetear un nodo
antes de revenderlo. NUNCA borra datos del ERP — sólo la license file.

---

## 9. Troubleshooting

### "license file invalid: firma inválida"

Causas posibles:
- El archivo se transfirió en modo texto (CRLF) — re-bajar en binario.
- El `.lic` fue editado a mano (¡no se hace!).
- La pubkey embebida en el binario no es la que firmó este .lic →
  binario muy viejo (release < fecha de rotación). Actualizar binario.
- Tu archivo es para otro `tenant_id` — pedir al licenser uno nuevo.

### "key_id desconocido: lk-XXX"

El `.lic` fue firmado con una key que no está en el binario actual.
Significa: instalar la última release de pharma-server (incluye nuevas
pubkeys agregadas por rotación). Ver ADR-0007.

### "schema_version 2 no soportada"

License fue emitida bajo schema v2 (futuro) pero binario soporta v1.
Actualizar binario. La compat backward está garantizada (ADR-0002 §2.2);
forward NO.

### El endpoint `/api/v1/admin/license/reload` devuelve 403

El token no tiene rol `admin` ni `owner`. Issue un token con
`pharma user-create --roles admin,owner ...` o pedir al admin del tenant.

### El endpoint devuelve 503 con `license_path` ausente

Service arrancó sin `data_dir` configurado (caso raro — sólo tests en
memoria). Setear `db.path` en `config/local.toml`.

---

## 10. Flujo de renovación anual (Pro/Business)

1. Webpay/Stripe cobra el ciclo anual (ver `docs/strategy/payments-cl.md`).
2. `pharma-license-server` emite un nuevo `.lic` con `expires_at` extendido.
3. Operador descarga + `pharma license import` + reload.
4. Antes de `expires_at` no es necesario reimportar; el grace period de
   30 días absorbe demoras de hasta 30 días post-vencimiento sin afectar
   features (sólo aparece warning).

---

## 11. Flujo de microtransacción (one-time)

1. Operador compra (ej. "Branding pack").
2. License-server **re-emite** el `.lic` activo con el addon agregado en
   `bought_addons[]` + las `feature_keys` agregadas en `features[]`.
3. Mismo `pharma license import` + reload.
4. Las microtx son **append-only**: una compra nunca se quita, sólo se
   agrega. Si el usuario hace downgrade del tier, los addons sobreviven.

---

## 12. Referencias

- Spec técnica: [`license-architecture.md`](../strategy/license-architecture.md).
- Por qué Ed25519 offline: [ADR-0002](../adr/0002-license-ed25519-offline.md).
- Por qué core nunca se bloquea: [ADR-0005](../adr/0005-core-gratis-no-locked-in.md).
- Revocation/CRL: [ADR-0006](../adr/0006-revocation-strategy-signed-crl.md).
- Rotación de claves: [ADR-0007](../adr/0007-key-rotation-licenser.md).

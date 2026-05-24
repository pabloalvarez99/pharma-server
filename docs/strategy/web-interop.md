---
title: Interop web ↔ pharma-server — Guía del operador
status: Draft — 2026-05-24
owners: pabloalvarez99
adr: ADR-0012
---

# Conectar tu web/storefront a pharma-server

Esta guía es para el dueño / IT de una farmacia que **ya tiene una página web
funcionando** (storefront Next.js, Astro, WordPress, Shopify headless, lo que sea) y
quiere que esa web muestre el catálogo y stock que vive en su pharma-server local,
**sin migrar nada al cloud y sin perder el control de sus datos**.

> Decisión arquitectónica detrás de esta guía: [ADR-0012](../adr/0012-web-onprem-interop.md).
> Si querés entender *por qué* hacemos esto vía HTTP en vez de DB compartida, leelo
> primero.

## Resumen en 30 segundos

Hay tres formas de conectar tu web con el pharma-server. Elegí la primera salvo que
necesités otra cosa:

| Patrón | Dirección | Cuándo usarlo | Requiere puerto entrante |
|---|---|---|---|
| **A. Pull** (default) | web ← pharma-server | Mostrar catálogo/precios/stock con freshness de minutos/horas. | No (con VPN/tunnel) o Sí (IP pública) |
| **B. Push stock**     | pharma-server → web | Storefront necesita freshness <5 min en tiempo real. | No (saliente desde la farmacia) |
| **C. Push pedidos**   | web → pharma-server | Recibir pedidos online en el ERP del local. | **Sí** (pharma-server alcanzable desde internet) |

Patrón **A** se puede activar hoy con el catálogo público. **B** y **C** son
roadmap (endpoints aún no existen — esta guía los documenta para fijar el contrato).

---

## Patrón A — Pull desde la web (recomendado)

### 1. Abrí el endpoint público en pharma-server

Por default pharma-server escucha sólo en LAN (puerto TCP `8080`). Para Patrón A
tenés dos caminos:

**Opción A.1 — VPN / tunnel saliente** (sin abrir puerto entrante):
- Instalá Cloudflare Tunnel, Tailscale Funnel o ngrok en la máquina que corre
  pharma-server.
- El tunnel expone `https://farmacia-acme.trycloudflare.com` → `localhost:8080`.
- Esa URL la usás como `PHARMA_SERVER_URL` desde la web.

**Opción A.2 — IP pública + firewall** (requiere router con NAT abierta):
- Asigná IP estática al server.
- Abrí TCP/443 en el router → reenvía a `<ip-server>:8080`.
- Configurá un reverse proxy con TLS (Caddy / nginx).

> En ambos casos: NO publiques el endpoint sin habilitar la API key (paso 3).
> Sin API key, pharma-server por default rechaza llamadas al catálogo público.

### 2. Activá `public_catalog` en la config

Editá `config/local.toml` en la máquina del server (creá el archivo si no existe):

```toml
[public_catalog]
enabled = true
# Tenant slug que la web puede consultar. Si tenés varias sucursales, listá los slugs.
allowed_tenants = ["coquimbo-centro"]
# Rate limit por IP (requests/minuto). Por default 60.
rate_limit_per_min = 60
```

Reiniciá el servicio:

```powershell
sc stop PharmaServer
sc start PharmaServer
```

### 3. Generá una API key read-only

```powershell
pharma public-key create --tenant coquimbo-centro --scope catalog:read --name "web-vercel"
# Output:
#   key_id: pk_a1b2c3
#   secret: pk_live_xxxxxxxxxxxxxxxxxxxxxxxx
#   created_at: 2026-05-24T15:30:00Z
```

> El `secret` se imprime **una sola vez**. Guardalo en el secrets manager del web
> (Vercel env vars, GitHub Secrets, Doppler, etc.) como `PHARMA_PUBLIC_READ_KEY`.

### 4. Apuntá el build del web al pharma-server

En el repo del web (e.g. `build-and-deploy-webdev-asap`):

```bash
# Vercel project env vars (Production + Preview):
PHARMA_SERVER_URL=https://farmacia-acme.trycloudflare.com
PHARMA_TENANT_SLUG=coquimbo-centro
PHARMA_PUBLIC_READ_KEY=pk_live_xxxxxxxxxxxxxxxxxxxxxxxx
OUTPUT_SQL_FILE=./out/catalog_upsert.sql
```

Agregá el script `scripts/web-sync/pull-catalog.mjs` (ver
[`../../scripts/web-sync/README.md`](../../scripts/web-sync/README.md)) al repo del
web, y corré:

```bash
node scripts/web-sync/pull-catalog.mjs
psql "$DATABASE_URL" -f out/catalog_upsert.sql
```

Esto puede correr en build (`vercel-build`), en cron (Vercel Cron / GitHub Action),
o on-demand.

### 5. Verificá con curl

```bash
curl -H "Authorization: Bearer $PHARMA_PUBLIC_READ_KEY" \
     "$PHARMA_SERVER_URL/api/v1/public/catalog?tenant=$PHARMA_TENANT_SLUG&limit=5"
```

Respuesta esperada (extracto):

```json
{
  "schema_version": "1.0",
  "tenant": "coquimbo-centro",
  "generated_at": "2026-05-24T15:32:00Z",
  "items": [
    {
      "sku": "PARA-500-20",
      "name": "Paracetamol 500mg x20",
      "price_clp": 1990,
      "category": "Analgésicos",
      "image_url": null,
      "stock_status": "in_stock"
    }
  ],
  "pagination": { "next_cursor": "..." }
}
```

Si recibís `401` → revisá la API key. `403` → el tenant no está en
`allowed_tenants`. `429` → bajaste el rate limit; subí `rate_limit_per_min`.

---

## Patrón B — Push stock (pharma-server → web)

> Endpoint del lado pharma-server: **roadmap**. Esta sección fija el contrato para
> que el web pueda implementar el receptor en paralelo.

### Contrato

Pharma-server emite un POST cada vez que cambia stock o precio:

```http
POST https://tu-farmacia-coquimbo.cl/api/webhooks/pharma-stock
Content-Type: application/json
X-Pharma-Signature: sha256=<HMAC-SHA256(body, SHARED_SECRET)>
X-Pharma-Event-Id: 8f3e1d2c-4a5b-6c7d-8e9f-0a1b2c3d4e5f
X-Pharma-Tenant: coquimbo-centro
X-Pharma-Timestamp: 2026-05-24T15:34:12Z

{
  "schema_version": "1.0",
  "event_id": "8f3e1d2c-4a5b-6c7d-8e9f-0a1b2c3d4e5f",
  "event_type": "stock_changed",
  "tenant": "coquimbo-centro",
  "sku": "PARA-500-20",
  "stock_after": 42,
  "price_clp": 1990
}
```

Responsabilidades del **web** (no de pharma-server):
- Verificar `X-Pharma-Signature` (HMAC-SHA256 con `PHARMA_WEBHOOK_SHARED_SECRET`).
- Rechazar `X-Pharma-Timestamp` con drift > 5 min (replay defense).
- Idempotencia por `X-Pharma-Event-Id` (skip si ya procesado).
- Responder `2xx` rápido (<2s). Procesamiento pesado → en background.

Si el web responde `5xx` o timeout, pharma-server reintenta con backoff exponencial
hasta 6 veces.

### Activación (futura)

```toml
[webhooks.stock]
enabled = true
url = "https://tu-farmacia-coquimbo.cl/api/webhooks/pharma-stock"
secret_env = "PHARMA_WEBHOOK_SHARED_SECRET"   # Lee de env var, no del toml.
tenants = ["coquimbo-centro"]
```

---

## Patrón C — Push pedidos (web → pharma-server)

> Endpoint del lado pharma-server: **roadmap**. Contrato congelado acá.

### Contrato

Cuando el cliente confirma compra online, el web POSTea al pharma-server:

```http
POST https://farmacia-acme.trycloudflare.com/api/v1/public/orders/web
Content-Type: application/json
X-Pharma-Signature: sha256=<HMAC-SHA256(body, SHARED_SECRET)>
Idempotency-Key: order-web-2026-05-24-0001
X-Pharma-Tenant: coquimbo-centro

{
  "schema_version": "1.0",
  "external_order_id": "order-web-2026-05-24-0001",
  "customer": {
    "name": "Juan Pérez",
    "rut": "12.345.678-9",
    "phone": "+56912345678",
    "email": "juan@example.cl"
  },
  "items": [
    { "sku": "PARA-500-20", "qty": 2, "unit_price_clp": 1990 }
  ],
  "delivery": {
    "mode": "pickup",
    "address": null
  },
  "payment": {
    "provider": "webpay",
    "status": "paid",
    "external_ref": "wp_abc123"
  }
}
```

Pharma-server responde:

```json
{
  "order_id": "order:abc123",
  "status": "received",
  "estimated_ready_at": "2026-05-24T16:00:00Z"
}
```

Requiere que pharma-server sea alcanzable desde internet (a diferencia de A y B).

---

## Seguridad — checklist mínimo

- [ ] `PHARMA_PUBLIC_READ_KEY` y `PHARMA_WEBHOOK_SHARED_SECRET` viven en el secrets
      manager del web, **nunca en el repo**.
- [ ] El secret de HMAC tiene ≥ 32 bytes random (`openssl rand -hex 32`).
- [ ] TLS obligatorio en cualquier URL pública (sin TLS = sin API key válida).
- [ ] Rate limit activo (`rate_limit_per_min` en config).
- [ ] El endpoint público sirve **subset publicable** del catálogo. Datos sensibles
      (costo, margen, proveedor) **no se exponen** vía `public_catalog`.
- [ ] Rotá las keys cada 6 meses (`pharma public-key rotate <key_id>`).

---

## FAQ del operador

**P: ¿Tengo que abrir un puerto en mi router para usar Patrón A?**
R: No, si usás Cloudflare Tunnel / Tailscale Funnel. Sí, si querés IP pública directa.

**P: ¿Qué pasa si pharma-server se cae?**
R: La web sigue sirviendo desde el mirror de Cloud SQL (último pull exitoso). El POS
local también sigue funcionando: pharma-server es offline-first, no depende del web.

**P: ¿Y si la web se cae?**
R: Pharma-server sigue operando normalmente. Los webhooks del Patrón B se reintentan
con backoff exponencial; si la web vuelve, recibe el catch-up.

**P: ¿Puedo usar esto con WordPress / Shopify / Astro?**
R: Sí. El script `pull-catalog.mjs` es Node puro sin dependencias; podés portarlo a
PHP/Python/Ruby en <1 hora. El contrato es HTTP+JSON, no atado a un stack.

**P: ¿Esto cuenta como "sync online" del Tier Business?**
R: No directamente. El Tier Business incluye sync **entre nodos pharma-server**
(Fase 12). Patrón A/B/C con un web tuyo es interop con tu propio storefront, no con
otro ERP. Es gratis en el Tier Free siempre que `public_catalog` esté soportado por
el server.

---

## Referencias

- [ADR-0012](../adr/0012-web-onprem-interop.md) — decisión arquitectónica completa.
- [ADR-0005](../adr/0005-core-gratis-no-locked-in.md) — invariantes que esta guía respeta.
- [`../../scripts/web-sync/`](../../scripts/web-sync/) — script de referencia para Patrón A.
- [CLAUDE.md § "Scope de este repo"](../../CLAUDE.md) — separación de repos.

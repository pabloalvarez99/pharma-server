---
title: Interop web ↔ pharma-server — Guía del operador
status: Draft — 2026-05-24 · § Free Web agregada 2026-07-20
owners: pabloalvarez99
adr: ADR-0012, ADR-0013, ADR-0020
---

# Conectar tu web/storefront a pharma-server

Esta guía es para el dueño / IT de una farmacia que **ya tiene una página web
funcionando** (storefront Next.js, Astro, WordPress, Shopify headless, lo que sea) y
quiere que esa web muestre el catálogo y stock que vive en su pharma-server local,
**sin migrar nada al cloud y sin perder el control de sus datos**.

> Decisiones arquitectónicas detrás de esta guía:
> [ADR-0012](../adr/0012-web-onprem-interop.md) (3 patrones HTTP),
> [ADR-0013](../adr/0013-sync-bidireccional-stock.md) (stock sync + matriz canónica).
> Si querés entender *por qué* hacemos esto vía HTTP en vez de DB compartida, leelos
> primero.

## Resumen en 30 segundos

Hay tres formas de conectar tu web con el pharma-server. Elegí la primera salvo que
necesités otra cosa:

| Patrón | Dirección | Cuándo usarlo | Requiere puerto entrante |
|---|---|---|---|
| **A. Pull** (default) | web ← pharma-server | Mostrar catálogo/precios/stock con freshness de minutos/horas. | No (con VPN/tunnel) o Sí (IP pública) |
| **B. Push stock**     | pharma-server → web | Storefront necesita freshness <10s en tiempo real. | No (saliente desde la farmacia) |
| **C. Push pedidos**   | web → pharma-server | Recibir pedidos online en el ERP del local. | **Sí** (pharma-server alcanzable desde internet) |

Patrón **A** se puede activar hoy con el catálogo público. **B** y **C** son
roadmap (endpoints aún no existen — esta guía los documenta para fijar el contrato).

> **¿No tenés web propia?** El pharma-server trae un storefront integrado
> ([ADR-0020](../adr/0020-free-web-as-core.md)): catálogo público + pedidos de
> retiro bajo `/api/v1/public/{slug}/…`. Para exponerlo a internet ver la
> sección siguiente — no necesitás nada de los patrones A/B/C.

---

## Free Web (ADR-0020) — exponer el storefront con Cloudflare Tunnel

El seam Free Web vive en `/api/v1/public/{slug}/…`: lectura de catálogo sin
key (gated por el setting `web.published`) y `POST …/orders/web` con clave
`rb_live_…` + firma HMAC. Para que un cliente lo vea desde internet alcanza un
Cloudflare Tunnel **restringido al prefijo `/api/v1/public/`** — cinco pasos:

### 1. Instalá cloudflared

En la máquina que corre pharma-server (Windows):

```powershell
winget install Cloudflare.cloudflared
cloudflared tunnel login   # abre el browser; elegí tu zona DNS
```

### 2. Creá el tunnel

```powershell
cloudflared tunnel create rb-<slug>       # ej: rb-demo
# → guarda credenciales en %USERPROFILE%\.cloudflared\<TUNNEL_UUID>.json
```

### 3. Ruteá el DNS

```powershell
cloudflared tunnel route dns rb-<slug> tienda.<tu-dominio>.cl
```

### 4. Ingress SOLO hacia `/api/v1/public/`

`%USERPROFILE%\.cloudflared\config.yml`:

```yaml
tunnel: <TUNNEL_UUID>
credentials-file: C:\Users\<usuario>\.cloudflared\<TUNNEL_UUID>.json

ingress:
  # Sólo el seam público del storefront pasa por el tunnel.
  - hostname: tienda.<tu-dominio>.cl
    path: ^/api/v1/public/.*
    service: http://127.0.0.1:8080
  # Todo lo demás (admin, POS, login, settings, swagger) muere acá.
  - service: http_status:404
```

### 5. Corrélo como servicio

```powershell
cloudflared service install
sc start cloudflared
# smoke test:
curl https://tienda.<tu-dominio>.cl/api/v1/public/<slug>/store
```

### Troubleshooting

| Síntoma | Causa | Fix |
|---|---|---|
| `404` en todas las rutas públicas | Web no publicada (o slug mal escrito) — 404-oscuridad es intencional. | `PUT /api/v1/settings/web.published {"value":"true"}` con JWT admin (desde LAN, no por el tunnel). |
| `401 INVALID_API_KEY` | Clave `rb_live_…` mala/revocada/rotada. | Mint o rotate en `/api/v1/admin/web/keys`; actualizá `RB_API_KEY`. |
| `401 SIGNATURE_INVALID` | Secreto HMAC equivocado o canonical mal armado. | El secreto es POR CLAVE (`whsec_…` de la respuesta del mint). Canonical: `{ts}.POST.{path}.{sha256_hex(body)}` con el path completo `/api/v1/public/{slug}/orders/web`. |
| `401 TIMESTAMP_SKEW` | Reloj del cliente/server corrido > ±300s. | Sincronizá NTP (`w32tm /resync`) o revisá timezone del ts (debe ser unix **UTC** en segundos). |
| `403 SCOPE_DENIED` | Clave sin scope `orders:write` o de otro tenant. | Mint con scopes default (`catalog:read`, `orders:write`). |

### Nota de amenaza

- **Nunca** apuntes el ingress a `/` ni agregues rutas admin/POS: el único
  prefijo expuesto es `/api/v1/public/`, que sirve una proyección sin
  costos/márgenes/stock numérico y exige firma para escribir. Login, settings,
  keys y swagger quedan sólo en LAN.
- La regla catch-all `http_status:404` es obligatoria: sin ella cloudflared
  enruta cualquier path al service anterior.
- Despublicar (`web.published = "false"`) apaga el seam completo al instante
  (404 uniforme), aún con el tunnel arriba — kill-switch del operador.

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

> Endpoint del lado pharma-server: **roadmap** (rama futura `feat/api-stock-webhook`,
> ver [ADR-0013](../adr/0013-sync-bidireccional-stock.md) § Next steps). Esta sección
> fija el contrato para que el web pueda implementar el receptor en paralelo.

### Cuándo se dispara un webhook

El ERP emite **un POST por cada `stock_movement` con `delta != 0`** de un producto
con flag `publish_to_web = true`. Tipos de movimiento que disparan:

- `pos.sale` — venta normal POS (delta negativo).
- `pos.refund` — devolución (delta positivo).
- `po.receive` — recepción de orden de compra (delta positivo).
- `manual.adjust` — ajuste manual desde admin (delta cualquiera).
- `expiry.write_off` — write-off por vencimiento (delta negativo).

**NO disparan** (ruido innecesario):
- `inventory.recount` con `delta == 0`.
- SKUs con `publish_to_web = false` (productos internos no comercializados online).
- Tenants sin `[webhooks.stock]` configurado en `config/local.toml`.

**Coalescing**: si llegan varios movimientos del mismo SKU dentro de una ventana de
**2 segundos**, el ERP emite **un solo webhook** con el stock final. Esto absorbe
ráfagas (e.g. ajuste masivo, recarga de stock) sin saturar al web.

### Contrato del payload

```http
POST https://<web>/api/webhooks/pharma-stock
Content-Type: application/json
X-Pharma-Signature: sha256=<HMAC-SHA256(body, PHARMA_STOCK_WEBHOOK_SECRET)>
X-Pharma-Timestamp: 2026-05-24T15:34:12Z
X-Pharma-Tenant: coquimbo-centro
Idempotency-Key: 01J0K5R8X2HTZN3M5VC4P9KQ7T

{
  "schema_version": "1.0",
  "tenant_slug": "coquimbo-centro",
  "external_id": "PARA-500-20",
  "new_stock": 42,
  "in_stock": true,
  "ts": "2026-05-24T15:34:12Z",
  "idempotency_key": "01J0K5R8X2HTZN3M5VC4P9KQ7T"
}
```

Campos clave:
- `new_stock`: **stock final** después del movimiento (no el delta — el delta
  rompería idempotencia).
- `in_stock`: `new_stock > umbral_minimo`. Conveniencia para el web.
- `ts`: timestamp del movimiento (no del envío). Web descarta si llega out-of-order
  (`ts < last_applied_ts` para ese SKU).
- `idempotency_key`: UUID v7 (incluye timestamp) o hash determinístico del
  movement_id interno. El web persiste claves recibidas y skipea duplicados.

### Responsabilidades del **web** (no del ERP)

- Verificar `X-Pharma-Signature` con `crypto.timingSafeEqual` (Node) — timing-safe
  compare obligatorio.
- Rechazar `X-Pharma-Timestamp` con drift > 5 min (replay defense).
- Idempotencia por `Idempotency-Key`: tabla `webhook_received(idempotency_key PK,
  applied_at)`, skipear duplicados con 200 OK.
- Out-of-order: comparar `ts` payload vs `last_applied_ts(sku)` en Cloud SQL;
  si payload es más viejo, ack con 200 pero **no aplicar**.
- Responder `2xx` rápido (<2s). Procesamiento pesado → en background.

### Política de retry y fallo

Si el web responde 5xx, timeout o conexión rechazada, el ERP reintenta:

| Intento | Espera previa |
|---|---|
| 1 (inmediato) | 0s |
| 2 | 1s |
| 3 | 5s |
| 4 (último) | 30s |

Tras 4 intentos fallidos, el ERP:
- **Drop** el webhook (no se persiste para reintento futuro).
- **Log WARN** con `event_id`, `sku`, último status.
- **Métrica** `pharma_stock_webhook_dropped_total{tenant,reason}` para alerting.
- **NUNCA bloquea** la venta POS — el webhook es side-effect best-effort.

Status `4xx` (excepto 408/429) se considera **error de contrato** (firma inválida,
payload malformado): NO se reintenta, log ERROR, drop inmediato.

### Reconcile nightly (garantía de consistencia)

El push best-effort puede dropear eventos en outages prolongados. Para garantizar
**convergencia eventual**, el web debe correr el script `pull-catalog.mjs`
(Patrón A) **al menos una vez al día**, recomendado cron nocturno 03:00 hora local.

El pull trae el estado completo del catálogo (incluyendo `in_stock`) y reconcilia
cualquier drift acumulado por webhooks dropeados. **Cuando pull y webhook
discrepan, gana el más reciente en wall-clock** — el pull a las 03:00 se asume
implícitamente más reciente que cualquier webhook anterior.

### Activación (futura)

```toml
[webhooks.stock]
enabled = true
url = "https://tu-farmacia-coquimbo.cl/api/webhooks/pharma-stock"
hmac_secret_env = "PHARMA_STOCK_WEBHOOK_SECRET"   # Lee de env var, no del toml.
tenants = ["coquimbo-centro"]
coalesce_window_ms = 2000
publish_to_web_filter = true   # Sólo SKUs con flag publish_to_web=true.
```

Generá el secret:

```bash
openssl rand -hex 32
# → ponelo en env var del server (PHARMA_STOCK_WEBHOOK_SECRET) y en el secrets
#   manager del web. NUNCA en config/local.toml commiteado.
```

---

## Patrón C — Push pedidos (web → pharma-server)

> Endpoint del lado pharma-server: **roadmap**. Contrato congelado acá.

### Contrato

Cuando el cliente confirma compra online, el web POSTea al pharma-server:

```http
POST https://farmacia-acme.trycloudflare.com/api/v1/public/orders/web
Content-Type: application/json
X-Pharma-Signature: sha256=<HMAC-SHA256(body, PHARMA_ORDERS_WEBHOOK_SECRET)>
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

> Si el stock cayó entre que el web lo mostró disponible y el POST, el ERP
> responde **409 Conflict** con un payload de qué SKUs se quedaron sin stock.
> El web debe manejar este caso (refund, oferta de sustituto, etc.) — el ERP
> es canónico para stock, no acepta overselling.

Requiere que pharma-server sea alcanzable desde internet (a diferencia de A y B).

---

## Stock sync — modelo canónico (ADR-0013)

> Esta sección formaliza **quién manda en cada campo** cuando el ERP y el web
> tienen el mismo dato. Es la fuente única de resolución de conflictos. Referenciada
> en [ADR-0013](../adr/0013-sync-bidireccional-stock.md).

### Matriz de verdad

| Dominio / Campo | Canon | Razón | Replicado a |
|---|---|---|---|
| **Catálogo: name, laboratory, category, active_ingredient, image_url** | **Web** | El operador edita en admin del web (marketing-friendly, multi-canal). | ERP via import manual / CSV (no auto). |
| **Catálogo: external_id (SKU)** | **ERP** | Llave operativa interna; el web la copia pero no la inventa. | Web via Patrón A. |
| **Precio de venta online** | **Web** | Operador maneja precios online (campañas, descuentos por canal). Patrón A trae un "precio sugerido" del ERP, el web decide. | ERP no recibe. |
| **Precio de venta POS** | **ERP** | El POS físico tiene su propia tabla (convenios isapre, descuentos cash). | Web no recibe. |
| **Costo, margen, proveedor** | **ERP** | Nunca se publica. No sale del LAN. | Nada. |
| **Stock (cantidad)** | **ERP** | El POS decrementa; sólo el ERP escribe stock real. | Web via Patrón B push + Patrón A reconcile. |
| **Stock (booleano `in_stock`)** | **ERP** | Derivado de `stock > umbral_minimo`. | Web via Patrón B + Patrón A. |
| **Pedido online (al crearse)** | **Web (origen)** → **ERP (autoritativo tras aceptar)** | El cliente compra online, el web crea el pedido y lo POSTea via Patrón C. Aceptado en el ERP, el ERP pasa a ser la verdad (estado preparación, stock descontado). | Web recibe updates de estado via webhook futuro. |
| **Cliente / RUT / dirección** | **Web** (pedidos online) o **ERP** (POS físico) | Cada canal crea su propia ficha. Reconciliar es problema separado (Fase 13 CRM). | No replicado en v1. |
| **Boleta electrónica DTE** | **ERP** | Sólo el ERP habla con SII. | Nada. |

**Regla de oro**: cuando un campo aparece en ambos sistemas con valores distintos:
1. Si está en la matriz: gana el sistema canónico (sin merge).
2. Si NO está en la matriz: documentar acá antes de implementar — no inventar
   resolución ad-hoc.

### Garantía de consistencia

- **Patrón B push** = latencia típica <10s.
- **Patrón A pull nightly** = convergencia garantizada en <24h (cubre webhooks
  dropeados durante outages).
- **Stock real-time** (cliente online compra justo cuando POS también vende) =
  resuelto por **Patrón C 409 Conflict** — el ERP es la única autoridad para
  decrementar, no acepta overselling.

---

## Seguridad — checklist mínimo

- [ ] `PHARMA_PUBLIC_READ_KEY`, `PHARMA_STOCK_WEBHOOK_SECRET` y
      `PHARMA_ORDERS_WEBHOOK_SECRET` viven en el secrets manager del web,
      **nunca en el repo**.
- [ ] Cada secret HMAC tiene ≥ 32 bytes random (`openssl rand -hex 32`).
- [ ] TLS obligatorio en cualquier URL pública (sin TLS = sin API key válida).
- [ ] Rate limit activo (`rate_limit_per_min` en config).
- [ ] El endpoint público sirve **subset publicable** del catálogo. Datos sensibles
      (costo, margen, proveedor) **no se exponen** vía `public_catalog`.
- [ ] Rotá las keys cada 6 meses (`pharma public-key rotate <key_id>`).
- [ ] Webhook secrets se rotan **independiente** del read-key (blast radius
      separado).

---

## FAQ del operador

**P: ¿Tengo que abrir un puerto en mi router para usar Patrón A?**
R: No, si usás Cloudflare Tunnel / Tailscale Funnel. Sí, si querés IP pública directa.

**P: ¿Qué pasa si pharma-server se cae?**
R: La web sigue sirviendo desde el mirror de Cloud SQL (último pull exitoso). El POS
local también sigue funcionando: pharma-server es offline-first, no depende del web.

**P: ¿Y si la web se cae?**
R: Pharma-server sigue operando normalmente. Los webhooks del Patrón B se reintentan
con backoff (1s/5s/30s, 4 intentos). Si no responden, se dropean y el reconcile
nightly del Patrón A los cubre.

**P: ¿Y si el web aplica stock viejo (out-of-order)?**
R: El payload del webhook trae `ts` del movimiento. Si el web ya aplicó un evento
con `ts` posterior para el mismo SKU, descarta el viejo. Es responsabilidad del
receptor — el ERP no garantiza orden de entrega (los reintentos pueden re-ordenar).

**P: ¿Quién manda si edito un precio en el web y otro en el ERP?**
R: Ver la matriz canónica arriba. **Precio online = web**. **Precio POS = ERP**. Son
campos distintos y conviven sin merge.

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

- [ADR-0012](../adr/0012-web-onprem-interop.md) — decisión arquitectónica de los 3
  patrones HTTP.
- [ADR-0013](../adr/0013-sync-bidireccional-stock.md) — diseño concreto del Patrón B
  (stock sync) + matriz canónica.
- [ADR-0005](../adr/0005-core-gratis-no-locked-in.md) — invariantes offline-first
  que estas guías respetan.
- [`../../scripts/web-sync/`](../../scripts/web-sync/) — script de referencia para
  Patrón A.
- [CLAUDE.md § "Scope de este repo"](../../CLAUDE.md) — separación de repos.

# scripts/web-sync

Clientes Node **de referencia** (zero npm deps, Node ≥ 20) para los dos seams
web del `pharma-server`:

| Archivo | Seam | Qué hace |
|---|---|---|
| `pull-catalog.mjs` | **Free Web** (ADR-0020) | Pull del catálogo público `/api/v1/public/{slug}/…` → tabla + `catalog.json`. |
| `push-order.mjs` | **Free Web** (ADR-0020) | Crea pedido pickup firmado (Bearer + HMAC + Idempotency-Key). |
| `pull-catalog-sql.mjs` | Tu Farmacia (ADR-0012, Patrón A) | Pull `/api/v1/public/catalog?tenant=` → SQL UPSERTs para Cloud SQL. |
| `test.mjs` | — | Tests del generador SQL legacy (`node scripts/web-sync/test.mjs`). |

> Estos scripts NO corren en pharma-server: corren en el entorno del web
> (Vercel build, GitHub Action, cron, o tu terminal para probar el seam).
> Viven acá como referencia copy-paste.

---

## Seam Free Web (ADR-0020)

Storefront integrado del tenant: `GET /api/v1/public/{slug}/store|catalog|catalog/{pslug}`
(lectura sin key, gated por `web.published`) + `POST /api/v1/public/{slug}/orders/web`
(escritura con clave `rb_live_…` + firma HMAC por clave).

### `pull-catalog.mjs`

Baja la ficha de la tienda + catálogo completo (pagina `offset` hasta
`next_offset` null), imprime tabla `name · price_clp · availability` y escribe
`catalog.json`.

| Env | Requerida | Descripción |
|---|---|---|
| `ERP_ORIGIN` | no | Origen del server. Default `http://127.0.0.1:8080`. Sin trailing slash. |
| `RB_SLUG` | sí | Slug del tenant (segmento `{slug}` de la URL). |
| `OUTPUT_JSON` | no | Path del JSON. Default `./catalog.json`. |

```bash
ERP_ORIGIN=http://127.0.0.1:8080 RB_SLUG=demo node scripts/web-sync/pull-catalog.mjs
```

404 en todo = tenant desconocido **o** web no publicada (404-oscuridad,
indistinguibles a propósito).

### `push-order.mjs`

POST firmado de un pedido de retiro. Contrato de firma (PR3, verificado por el
server):

```text
canonical      = `${ts}.POST.${path}.${sha256_hex(rawBody)}`   // ts unix secs
X-Rb-Signature = hex(HMAC_SHA256(RB_HMAC_SECRET, canonical))
X-Rb-Timestamp = ts                                            // skew ±300s
```

| Env | Requerida | Descripción |
|---|---|---|
| `ERP_ORIGIN` | no | Default `http://127.0.0.1:8080`. |
| `RB_SLUG` | sí | Slug del tenant. |
| `RB_API_KEY` | sí | Clave `rb_live_…` con scope `orders:write` (mint: `POST /api/v1/admin/web/keys`). |
| `RB_HMAC_SECRET` | sí | Secreto `whsec_…` de ESA clave (se muestra una sola vez al crearla). |

| Flag | Descripción |
|---|---|
| `--product <id>` | Record id del producto (`product:xxx`, campo `id` del catálogo público). Requerido. |
| `--qty <n>` | Cantidad (entero ≥ 1). Requerido. |
| `--name`, `--phone` | Datos del cliente. Defaults de demo. |
| `--replay` | Re-envía el mismo body con la MISMA `Idempotency-Key` → espera 200 con payload cacheado (mismo `order_id`). |

```bash
RB_SLUG=demo RB_API_KEY=rb_live_xxx RB_HMAC_SECRET=whsec_xxx \
node scripts/web-sync/push-order.mjs --product product:abc123 --qty 2 --replay
```

Imprime `pickup_code / status / total / order_id / expires_at`.

### Demo end-to-end (server local sembrado)

Requiere: server corriendo (`cargo run -p api`), tenant `demo` con productos
`online_visible = true`, y un usuario admin. Bash (Git Bash / WSL):

```bash
ORIGIN=http://127.0.0.1:8080

# 0. JWT admin
TOKEN=$(curl -s $ORIGIN/api/v1/login -H 'Content-Type: application/json' \
  -d '{"tenant":"demo","email":"admin@demo.cl","password":"secret123"}' | \
  node -p "JSON.parse(require('fs').readFileSync(0)).token")

# 1. Publicar la web
curl -s -X PUT $ORIGIN/api/v1/settings/web.published \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"value":"true"}'

# 2. Mint de clave storefront (key + hmac_secret salen UNA sola vez)
CRED=$(curl -s -X POST $ORIGIN/api/v1/admin/web/keys \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"name":"storefront-demo"}')
export RB_API_KEY=$(echo "$CRED" | node -p "JSON.parse(require('fs').readFileSync(0)).key")
export RB_HMAC_SECRET=$(echo "$CRED" | node -p "JSON.parse(require('fs').readFileSync(0)).hmac_secret")

# 3. Pull del catálogo (elige un product id de la tabla)
export ERP_ORIGIN=$ORIGIN RB_SLUG=demo
node scripts/web-sync/pull-catalog.mjs

# 4. Pedido + replay de idempotencia
node scripts/web-sync/push-order.mjs --product product:XXXX --qty 2 --replay

# 5. El pedido está en el ERP (channel=web)
curl -s "$ORIGIN/api/v1/orders?channel=web" -H "Authorization: Bearer $TOKEN"

# 6. Despublicar → catálogo vuelve a 404-oscuridad
curl -s -X PUT $ORIGIN/api/v1/settings/web.published \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"value":"false"}'
curl -s -o /dev/null -w '%{http_code}\n' $ORIGIN/api/v1/public/demo/catalog   # 404
```

Túnel a internet (Cloudflare Tunnel, sólo `/api/v1/public/`): ver
[`docs/strategy/web-interop.md` § Free Web](../../docs/strategy/web-interop.md#free-web-adr-0020--exponer-el-storefront-con-cloudflare-tunnel).

---

## Seam Tu Farmacia (ADR-0012) — `pull-catalog-sql.mjs`

Cliente del Patrón A de ADR-0012 (storefront EXTERNO con su propio Cloud SQL).
Hace fetch paginado a `GET /api/v1/public/catalog?tenant=` y emite SQL con
UPSERTs idempotentes para la tabla `products` del web.

| Env | Requerida | Descripción |
|---|---|---|
| `PHARMA_SERVER_URL` | sí | URL base del pharma-server. |
| `PHARMA_TENANT_SLUG` | sí | Slug del tenant a sincronizar. |
| `PHARMA_PUBLIC_READ_KEY` | sí | API key read-only scope `catalog:read`. |
| `OUTPUT_SQL_FILE` | no | Default `./out/catalog_upsert.sql`. |

```bash
PHARMA_SERVER_URL=https://farmacia-acme.trycloudflare.com \
PHARMA_TENANT_SLUG=coquimbo-centro \
PHARMA_PUBLIC_READ_KEY=pk_live_xxx \
node scripts/web-sync/pull-catalog-sql.mjs        # --dry-run imprime sin escribir
```

Aplicar: `psql "$DATABASE_URL" -f out/catalog_upsert.sql`. Emite
`INSERT … ON CONFLICT (sku) DO UPDATE` + tombstone soft
(`stock_status='out_of_stock'`) para SKUs desaparecidos. Exit codes: 0 OK ·
1 config · 2 red/HTTP · 3 shape. Tests: `node scripts/web-sync/test.mjs`.

Guía completa del operador (patrones A/B/C, matriz canónica de stock):
[`docs/strategy/web-interop.md`](../../docs/strategy/web-interop.md).

---

## Referencias

- [ADR-0020](../../docs/adr/0020-free-web-as-core.md) — Free Web como core.
- [ADR-0012](../../docs/adr/0012-web-onprem-interop.md) — interop web externa (3 patrones).
- [`docs/strategy/web-interop.md`](../../docs/strategy/web-interop.md) — guía operador (tunnel, seguridad).

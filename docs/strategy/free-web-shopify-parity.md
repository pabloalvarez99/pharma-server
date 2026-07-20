# Free Web — paridad Shopify para el tier Free

**Misión**: que cualquier negocio con RutBusiness publique gratis un storefront
(catálogo + pedidos pickup) servido por su propio ERP offline, en menos de 15 minutos.
Doctrina: [ADR-0020](../adr/0020-free-web-as-core.md).

## Persona

**Sandra**: 1 local, atiende WhatsApp como CRM, sus clientes retiran en tienda
(pickup > courier). Si publicar su web tarda más de 15 minutos o pide tarjeta,
abandona. Ella es el bar de éxito de todo lo que se construya acá.

## Gap vs Shopify (tier Free)

| Capacidad | Shopify | RutBusiness Free |
|---|---|---|
| Catálogo público | ✅ | ✅ (pull desde el ERP local, seam HTTP) |
| Carrito | ✅ | ✅ (client-side, sin cuenta) |
| Checkout | Envío + pago online | **Pickup** (retiro en tienda) |
| Pagos | Online (Shopify Payments) | **POS / al mesón** (online = pago, `web.payments_online`) |
| Dominio propio | Pago | Pago (`web.custom_domain`) |
| Temas | Muchos | **1 tema** (extras = `web.branding_advanced`) |
| Backoffice | Cloud SaaS | ERP offline-first local (verdad = ERP) |

## Free vs Pago

| Free (ungated, sin 402) | Pago (feature key) |
|---|---|
| 1 storefront público | Multi-sitio — `web.multi_site` |
| Catálogo + pedidos pickup | Pago online tarjeta — `web.payments_online` |
| Subdominio / URL provista | Dominio propio — `web.custom_domain` |
| 1 tema estándar | Branding avanzado — `web.branding_advanced` |
| — | Marketing automation — `web.marketing_automation` |

**Invariantes** (ADR-0020): `web.published` default off → 404 público; POS/ERP offline
siempre; stock+dinero se resuelven en el ERP; precios strings decimales; `cost_price`
jamás sale del server.

## Build queue

Prompts autónomos por PR: [`docs/product/free-web-prompts/README.md`](../product/free-web-prompts/README.md)
— PR1 catálogo público → PR2 API keys → PR3 pedidos web → PR4 tooling → PR5 storefront.

## Métricas

- Time-to-publish < 15 min (instalar → web pública).
- Primer pedido web el mismo día de publicar.
- Cero oversell (stock web nunca vende lo que el POS ya vendió).

## Demo verificada (2026-07-20)

Corrida real del demo end-to-end de [`scripts/web-sync/README.md`](../../scripts/web-sync/README.md)
contra server local (`pharma-api` debug, `127.0.0.1:8090`, DB fresca migrada
0001→0038, tenant `demo` sembrado `seed-demo --vertical pharmacy`, 6 productos
`online_visible`). Outputs reales (secretos truncados):

**Mint de clave + publicar** (`POST /api/v1/admin/web/keys` + `PUT settings/web.published`):

```json
{"id":"api_key:v639r84fdom5ffela8m0","name":"storefront-demo","key":"rb_live_6ea3…","hmac_secret":"whsec_662f…","key_prefix":"rb_live_6ea3","scopes":["catalog:read","orders:write"]}
{"key":"web.published","value":"true","updated_at":"2026-07-20T22:31:56.013336300Z"}
```

**Pull catálogo** (`node scripts/web-sync/pull-catalog.mjs`):

```text
[pull-catalog] Source: http://127.0.0.1:8090/api/v1/public/demo
[pull-catalog] Store: Farmacia Demo (currency=CLP)
[pull-catalog] Page 1: +6 items (total=6)

name                   price_clp  availability
---------------------  ---------  ------------
Alcohol Gel 250ml      1990       low
Amoxicilina 500mg x12  3990       in_stock
Aspirina 100mg x30     1690       in_stock
Clorfenamina 4mg x20   990        in_stock
Ibuprofeno 400mg x20   2490       in_stock
Loratadina 10mg x10    1890       in_stock

[pull-catalog] Wrote 6 items to …\pr8-catalog.json
```

**Pedido pickup + replay de idempotencia** (`node scripts/web-sync/push-order.mjs --product product:ja21olhevmnvng1tqvbp --qty 2 --replay`):

```text
[push-order] POST http://127.0.0.1:8090/api/v1/public/demo/orders/web
[push-order] Idempotency-Key: 36d8956c-6f1a-4269-bdfa-40b368942687
[push-order] HTTP 201 — pickup_code=RET-FPEV status=reserved total=4980 CLP order_id=order:41456384c6ed4e07a5ab2039ffa8d164 expires_at=2026-07-21T22:32:13.185723900Z
[push-order] Replaying same Idempotency-Key (espera HTTP 200 + payload cacheado)…
[push-order] HTTP 200 — pickup_code=RET-FPEV status=reserved total=4980
[push-order] Idempotencia OK: mismo order_id, sin pedido duplicado.
```

**Pedido en el ERP** (`GET /api/v1/orders?channel=web`):

```json
{"id":"order:41456384c6ed4e07a5ab2039ffa8d164","status":"reserved","total":"4980","customer_name":"Cliente Web","channel":"web","pickup_code":"RET-FPEV","fulfillment_type":"pickup","expires_at":"2026-07-21T22:32:13.185723900Z"}
```

**Transición admin + despublicar → 404-oscuridad**:

```text
POST /api/v1/admin/orders/order:41456384…/transition {"to":"preparing"} -> status=preparing
PUT  /api/v1/settings/web.published {"value":"false"}                   -> despublicada
GET  /api/v1/public/demo/catalog                                        -> HTTP 404
```

Ciclo completo verificado: publish → catálogo público → pedido `RET-FPEV`
(HMAC + Idempotency-Key, replay seguro) → visible en ERP `channel=web` →
transición `reserved→preparing` → despublicar vuelve todo `/public/{slug}` a 404.

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

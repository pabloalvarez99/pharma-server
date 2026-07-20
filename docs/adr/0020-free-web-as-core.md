# ADR-0020: Web gratis como core (1 storefront público en el tier Free)

- **Status**: Accepted
- **Date**: 2026-07-20
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: producto, freemium, storefront, estrategia
- **Supersede parcialmente**: [ADR-0014](./0014-dss-storefront-integration.md) (cláusulas freemium) · [ADR-0019](./0019-product-surface-taxonomy.md) (principio 5, cláusula de cobro)

## Context and Problem Statement

Hasta hoy la doctrina escrita era "la web es tier pago":

- [ADR-0014 § Decision Drivers](./0014-dss-storefront-integration.md): *"**Freemium**: la
  web/storefront es **valor pago** (encaja con 'agentes = tier pago', tesis SaaS→Agentic)"*,
  reforzado en sus guardrails: *"**Freemium**: storefront/web conectada = capacidad de
  **tier pago**"* y en el ladder L3 (provisioning "Tier pago").
- [ADR-0019 § principio 5](./0019-product-surface-taxonomy.md): *"Se cobra **alcance** …
  y **salir a la web** (storefront), nunca el derecho a operar"*.

Shopify demostró que la presencia web básica es la puerta de entrada, no el premium. Para
el beachhead (comercio chico chileno, pickup + WhatsApp) exigir pago por un catálogo
público mata la adopción antes de generar el hábito. El Free que solo opera puertas
adentro no compite contra "hazte una tienda gratis en 15 minutos".

## Decision Outcome

**El tier Free incluye 1 storefront web público**: catálogo + pedidos con retiro en
tienda (pickup), servido desde el ERP local a través del seam HTTP público existente
(ADR-0012/0013/0014). La web gratis es **ungated**: cero 402 en catálogo y pedidos
básicos.

### Qué supersede (y qué NO)

- **Supersede**: toda cláusula previa que haga de la presencia web básica una capacidad
  de pago — ADR-0014 § Decision Drivers ("la web/storefront es valor pago"), ADR-0014
  § guardrails ("storefront/web conectada = capacidad de tier pago") y ADR-0019
  § principio 5 en su parte "se cobra salir a la web". La presencia web básica pasa a ser
  additive al Free (coherente con [ADR-0005](./0005-core-gratis-no-locked-in.md):
  al Free solo se AGREGA, nunca se quita).
- **Mantiene**: toda la arquitectura del seam — repos separados, 3 verbos
  (pull catálogo · push stock · push pedidos), API keys server-side, exposición WAN vía
  tunnel, dirección de verdad = el núcleo (ADR-0012/0013/0014/0019 intactos en lo técnico).

### Qué sigue siendo pago

| Capacidad | Feature key |
|---|---|
| Dominio propio (custom domain) | `web.custom_domain` |
| Branding avanzado (temas extra, CSS custom) | `web.branding_advanced` |
| Pago online con tarjeta | `web.payments_online` |
| Multi-sitio (más de 1 storefront) | `web.multi_site` |
| Automatización de marketing | `web.marketing_automation` |

### Invariantes

- **Opt-in**: `web.published` default **off** → los endpoints públicos responden 404
  uniforme. Nadie sale a la web sin decirlo.
- **Offline-first intacto**: POS/ERP operan sin internet para siempre (ADR-0005).
  La web caída jamás bloquea vender en el mesón.
- **Verdad = ERP**: stock y dinero se resuelven en el núcleo; el storefront nunca es
  fuente de verdad.
- **Precios**: strings decimales (contrato de dinero existente).
- **`cost_price` jamás sale del server**: el catálogo público solo expone precio de venta.

## Consequences

**Positivas**
- Funnel Shopify-grade: instala gratis → opera → publica su web gratis → paga por
  dominio/branding/pagos online cuando el canal ya le vende.
- Coherencia con ADR-0005 (invariante additive) y con el pitch "ERP que respeta a su dueño".

**Costos / riesgos**
- Se renuncia a cobrar la puerta de entrada web; la monetización se corre a las
  feature keys de la tabla.
- Costo de soporte del canal web en usuarios Free (mitigado: opt-in + tunnel del dueño).

## More Information

- Estrategia y gap vs Shopify: [`docs/strategy/free-web-shopify-parity.md`](../strategy/free-web-shopify-parity.md)
- Seam técnico: [ADR-0012](./0012-web-onprem-interop.md) · [ADR-0013](./0013-sync-bidireccional-stock.md) · [ADR-0014](./0014-dss-storefront-integration.md)
- Invariantes Free: [ADR-0005](./0005-core-gratis-no-locked-in.md)
- Taxonomía de superficies: [ADR-0019](./0019-product-surface-taxonomy.md)

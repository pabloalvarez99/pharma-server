# ADR-0014: DSS como capa storefront de RutAgentIA (integración por seam HTTP, no merge)

- **Status**: Partially superseded by [ADR-0020](./0020-free-web-as-core.md) (cláusulas freemium de storefront); Accepted for seam HTTP / arquitectura DSS
- **Date**: 2026-06-14
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: producto, estrategia, interop, multi-rubro, agéntico
- **Superseded (partial)**: las cláusulas que trataban la web/storefront como solo tier pago quedan superadas por [ADR-0020](./0020-free-web-as-core.md) (1 storefront público en Free). El seam HTTP y el no-merge de repos se mantienen. Cuerpo histórico sin reescribir.

## Context and Problem Statement

El fundador tiene **DSS** ([`https://dss-spa.vercel.app`](https://dss-spa.vercel.app),
Vercel + Cloudflare): una agencia/generador de sitios web para comercios chilenos,
con (a) una **taxonomía de rubros** (Restaurant/Comida, Café/Pastelería, Tienda/Retail,
Belleza/Estética, Servicios/Oficios, Otro) y (b) un **portafolio de páginas estáticas
por rubro** (flagship `tu-farmacia.cl`). Quiere integrar DSS "de la mejor forma" a
pharma-server / RutAgentIA.

`pharma-server` es el ERP on-prem (back-office). DSS es presencia web (front-office).
En la visión RutAgentIA ([`docs/strategy/rutagentia-vision.md`](../strategy/rutagentia-vision.md))
un negocio = `1 RUT → 1 agente` que opera **ambos lados**. DSS es, literalmente, el
front-office que faltaba nombrar.

El riesgo es integrarlo mal: fusionar repos, romper offline-first, meter dependencia
cloud en el core, o construir un "storefront-as-a-service" gigante antes de tener
revenue. Esta ADR fija la forma correcta y el orden.

## Decision Drivers

- **Cerrar el lazo** back-office ↔ front-office sin violar la separación de repos
  ([CLAUDE.md § Scope](../../CLAUDE.md): no cross-imports, no shared CI/deploy).
- **Offline-first intacto** ([ADR-0005](./0005-core-gratis-no-locked-in.md)): el core
  ERP nunca depende de DSS ni de la nube para operar.
- **Reusar lo ya construido**: el seam HTTP ya existe ([ADR-0012](./0012-web-onprem-interop.md)
  patrones A/B/C + [ADR-0013](./0013-sync-bidireccional-stock.md) push de stock;
  endpoints `GET /api/v1/public/catalog`, `POST /api/v1/public/orders/web`,
  `scripts/web-sync/`, order_channel mig 0019). No reinventar.
- **Freemium**: la web/storefront es **valor pago** (encaja con "agentes = tier pago",
  tesis SaaS→Agentic). El core gratis no la necesita.
- **Self-funded / disciplina**: empezar por lo barato que valida el beachhead (Tu
  Farmacia Coquimbo conecta su sitio DSS a su pharma-server HOY). No construir Fase 14
  antes de tiempo.

## Considered Options

1. **Seam HTTP en capas (DSS consume la API pública de pharma-server)** — DSS = cliente
   del contrato ADR-0012/0013. Cero shared code. Capas L0→L4 incrementales.
2. **Monorepo / fusionar DSS dentro de pharma-server** — un solo repo full-stack.
3. **DB compartida** (DSS escribe/lee la DB del ERP).
4. **Generador de storefront DENTRO de pharma-server** (el server sirve el sitio).

## Decision Outcome

**Elegida: Opción 1 — seam HTTP en capas.** DSS es la **capa storefront de RutAgentIA**,
acoplada por el contrato HTTP ya existente. Repos separados, sin cross-import. Las
opciones 2/3/4 rompen offline-first, scope, o ambos (la 4 obligaría al MSI on-prem a
servir web pública desde la LAN — exactamente lo que ADR-0012 rechazó).

### Arquitectura objetivo (constelación)

```
RUT (identidad: did:rut: / Ed25519, reusa crates/agent)
  → Agente RutAgentIA (orquestador)
     ├─ back-office: pharma-server (ERP on-prem, offline-first, LAN)   [este repo]
     └─ front-office: storefront DSS (cloud Vercel/CF, por rubro)      [repo DSS]
        ↕ seam HTTP (opt-in, API key + HMAC, idempotente):
           · Patrón A  GET  /api/v1/public/catalog        (web pull catálogo)
           · Patrón B  push /stock-movements (webhook)    (ERP push stock, ADR-0013)
           · Patrón C  POST /api/v1/public/orders/web     (web push pedido → ERP)
        ↕ puente LAN→internet: Cloudflare Tunnel (el ERP NO se expone directo)
```

Verdad canónica (de ADR-0013): **stock = ERP**, **precio/catálogo publicable = web**.
El rubro es el **vocabulario común** (mismo valor en ambos lados → join key).

### Capas de integración (incrementales, cada una se paga sola)

| Capa | Qué | Estado | Fase |
|---|---|---|---|
| **L0** Vocabulario | Catálogo de rubros compartido (DSS taxonomy = RutAgentIA verticals) | ✅ hecho ([`rubro-catalog.md`](../strategy/rubro-catalog.md)) | hoy |
| **L1** Seam vivo | Un storefront DSS conecta a un pharma-server real (pull catálogo + push pedido + recibe stock) vía Cloudflare Tunnel | ~80% (endpoints existen; falta endurecer + cliente DSS + doc tunnel) | hoy (beachhead) |
| **L2** Plantillas×rubro | Mapear cada plantilla estática DSS a un rubro del catálogo; "qué sitio para qué rubro" | ⬜ doc | corto |
| **L3** Provisioning | RutAgentIA ofrece "publica tu web": provisiona un storefront DSS cableado al ERP del tenant. Tier pago. | ⬜ | Fase 14 (cloud companion) |
| **L4** Agéntico | El agente del RUT opera ambos: actualiza precio/catálogo en la web, lee pedidos, gestiona stock. La web es superficie del agente. | ⬜ | Fase 15 |

### Plan de ejecución (orden estricto, disciplina self-funded)

**Ahora (L1 — desbloquea revenue real, Tu Farmacia Coquimbo):**
1. **Endurecer el seam** (server): confirmar/cerrar API key con scopes (`catalog:read`,
   `orders:write`) + HMAC en orders + Patrón B (push stock) completo (ADR-0013). Es
   backend → lane de **marvin**.
2. **Cliente DSS de referencia** en `scripts/web-sync/` (Node, zero-dep): `push-order`
   y `pull-catalog` con la forma que un storefront DSS usa. Reference code (corre en el
   web, no en el server) → lane de **bob** (tooling).
3. **Doc puente Cloudflare Tunnel** (LAN→internet opt-in): receta para exponer el
   pharma-server del local sólo por los endpoints públicos → `docs/strategy/web-interop.md`
   (ampliar) → lane docs.

**Corto (L2):** catalogar las plantillas DSS y mapearlas a rubros (extiende
`rubro-catalog.md` §plantillas). Sin código.

**Post-revenue (L3/L4, Fase 14/15):** provisioning 1-click + agente operando la web.
NO antes. Reevaluar plantillas DSS como base del storefront por tenant.

### Invariantes (no se rompen)

- **Sin cross-import**: DSS consume HTTP/JSON, jamás importa código del server (ni
  viceversa). Repos, CI y deploy separados.
- **Offline-first**: endpoints públicos son **opt-in** (toggle + API key); apagados =
  404 uniforme. El POS y el core operan sin internet pase lo que pase.
- **Freemium**: storefront/web conectada = capacidad de **tier pago**; el core gratis
  no la requiere. Coherente con ADR-0005 (sólo se AGREGA al Free, nunca se quita).
- **Datos del cliente**: JSON versionado sobre HTTP, exportable. El dueño es dueño.
- **RUT/identidad**: pedidos web entran con `order_channel = web` (mig 0019); a futuro
  el storefront se ata al RUT del tenant (did:rut:, Fase 15).

## Consequences

### Positivas
- Cierra el lazo comercial (catálogo online → pedido → ERP → stock → web) **sin** tocar
  la arquitectura on-prem ni el offline-first.
- Reusa 100% el seam ya construido (ADR-0012/0013) → time-to-value bajo.
- Convierte DSS de "agencia aparte" en **canal de distribución + upsell** de
  RutAgentIA (cada sitio DSS es un gancho al ERP, y cada ERP un gancho a un sitio).
- Da a Tu Farmacia Coquimbo (cliente real) un beneficio inmediato → valida el beachhead.

### Negativas / riesgos
- El puente LAN→internet (Cloudflare Tunnel) es responsabilidad operativa del dueño;
  mitigación: doc guiado + opt-in + scopes mínimos.
- Tentación de saltar a L3 (provisioning) antes de revenue; mitigación: este ADR fija
  el orden y marca L3/L4 como Fase 14/15 explícitamente.

### Neutras
- DSS puede seguir vendiendo sitios a comercios SIN pharma-server; la integración es
  un superpoder opcional, no un requisito de ninguno de los dos.

## More Information
- [ADR-0012](./0012-web-onprem-interop.md) (seam HTTP), [ADR-0013](./0013-sync-bidireccional-stock.md)
  (push stock), [ADR-0005](./0005-core-gratis-no-locked-in.md) (invariantes), [ADR-0001](./0001-freemium-pivot.md).
- [`docs/strategy/rutagentia-vision.md`](../strategy/rutagentia-vision.md),
  [`docs/strategy/rubro-catalog.md`](../strategy/rubro-catalog.md),
  [`docs/strategy/ecosystem-roadmap.md`](../strategy/ecosystem-roadmap.md).
- DSS: https://dss-spa.vercel.app · memoria [[dss-rubro-catalog]].
- Repo web real relacionado: `build-and-deploy-webdev-asap` (Tu Farmacia Coquimbo).

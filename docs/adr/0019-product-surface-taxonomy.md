# ADR-0019: Taxonomía de superficies de producto (núcleo + operador + storefront), genérica para todo cliente

- **Status**: Accepted
- **Date**: 2026-06-26
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: producto, arquitectura, naming, distribución, multi-rubro, storefront
- **Plan**: [`docs/strategy/product-surfaces-master-plan.md`](../strategy/product-surfaces-master-plan.md)
- **Extiende / unifica**:
  [ADR‑0005 core gratis / no lock‑in](./0005-core-gratis-no-locked-in.md) ·
  [ADR‑0012 web‑onprem‑interop](./0012-web-onprem-interop.md) ·
  [ADR‑0013 sync bidireccional stock](./0013-sync-bidireccional-stock.md) ·
  [ADR‑0014 DSS storefront](./0014-dss-storefront-integration.md) ·
  [ADR‑0015 cliente universal](./0015-universal-cross-platform-client.md) ·
  [ADR‑0017 BYO‑AI](./0017-byo-ai-provider.md) ·
  [ADR‑0018 cloud multi-tenant](./0018-cloud-multitenant-saas.md)

## Context and Problem Statement

Existen cuatro nombres que el fundador usa — **RutAgent Windows (MSI)**,
**RutAgent Business**, **RutAgent Web** y **tu-farmacia.cl** — y no estaba escrito
**cómo encajan entre sí** ni **cómo se generalizan a cualquier cliente** (la farmacia es
sólo el piloto). Riesgo: tratarlos como cuatro productos rivales, branding inconsistente,
o construir el storefront acoplado al núcleo. Hace falta una **taxonomía canónica** que
dé sentido al conjunto para *todo* negocio futuro (cualquier rubro, identificado por su RUT).

## Decision

Adoptar una taxonomía de **2 audiencias × 2 lugares**, con **un núcleo por RUT** y
**N superficies** que se asoman a él. Las cuatro cosas se clasifican así:

| Clave | Nombre | Categoría | Audiencia | Lugar |
|-------|--------|-----------|-----------|-------|
| **N** | RutAgent Windows (MSI) | **Núcleo** (servidor `pharma-server`) | — (infra) | On-prem / tenant cloud |
| **O1** | RutAgent Business | Superficie de **operador** (app nativa Tauri) | Dueño/cajero | Desktop/móvil |
| **O2** | RutAgent Web | Superficie de **operador** (web/PWA, `/app`) | Dueño/cajero | Navegador |
| **O3** | Agente | Capa **conversacional** dentro de O1/O2 | Operador | Donde corra O1/O2 |
| **C1** | `tu-negocio.cl` (`tu-farmacia.cl` = piloto) | Superficie de **cliente final** (storefront) | El comprador | Nube/repo separado |
| **+** | `rutagent.cl` | Núcleo **hospedado** (tier cloud) | — (infra) | Nube multi-tenant |

Principios canónicos (vinculantes para todo trabajo nuevo):

1. **1 RUT = 1 núcleo = 1 agente = N superficies.** El núcleo es la verdad; las
   superficies son vistas. Ninguna superficie es dueña del dato.
2. **Dos seams, cero merges.**
   - Operador ↔ núcleo = **`/api/v1`** (JWT por tenant, tool-first, idempotente).
   - Núcleo ↔ storefront = **3 verbos** (pull catálogo · push stock · push pedidos),
     dirección de verdad = el núcleo. Sin cross-import, sin DB/CI compartidos.
3. **`RutAgent` es la marca.** Las superficies se nombran *RutAgent Business* (operador
   nativo), *RutAgent Web* (operador web), *RutAgent (núcleo/Windows)*; el storefront es
   **`tu-negocio.cl`**. `RutBusiness`/`RutAgentIA` = sinónimos históricos de la familia.
   El **rename físico** (repo/crates/binarios) queda **diferido** hasta go explícito.
4. **Genérico por configuración, no por fork.** Un binario, un agente, una web sirven a
   todo rubro; `business.vertical` decide qué se muestra. `tu-farmacia.cl` es la instancia
   pharma de `tu-negocio.cl`; el storefront cambia de **plantilla** por rubro (DSS), no de motor.
5. **El operar es gratis y offline-first.** Se cobra **alcance** (cajas/sucursales/sync) y
   **salir a la web** (storefront), nunca el derecho a operar ([ADR‑0005](./0005-core-gratis-no-locked-in.md)).

## Consequences

**Positivas**
- Una sola historia para vender a cualquier cliente: instala/abre el núcleo (gratis) →
  opera con Business/Web/agente → activa su storefront `tu-negocio.cl` (pago) → crece.
- Naming consistente; el equipo sabe en qué celda cae cada cosa nueva.
- El storefront se generaliza sin tocar el núcleo (seam estable).

**Costos / riesgos**
- El rename físico diferido mantiene "pharma-server"/"RutBusiness" en código y docs
  (deuda de branding consciente, no bloqueante).
- Generalizar `tu-farmacia.cl` → `tu-negocio.cl` por rubro + activación 1‑click es trabajo
  real (storefront, repo separado) — ver [ADR‑0014](./0014-dss-storefront-integration.md).

**No-objetivos**
- No define el código del storefront ni del cloud (los cubren ADR‑0014 y ADR‑0018).
- No cambia el roadmap de fases; ordena lo que ya existe.

## Links

- Master plan operativo: [`product-surfaces-master-plan.md`](../strategy/product-surfaces-master-plan.md)
- Visión: [`agentic-business-platform.md`](../strategy/agentic-business-platform.md) · [`rutagentia-vision.md`](../strategy/rutagentia-vision.md)
- Web/cliente: [ADR‑0015](./0015-universal-cross-platform-client.md) · [`rutagent-web-platform-master-plan.md`](../strategy/rutagent-web-platform-master-plan.md)
- Storefront: [ADR‑0012](./0012-web-onprem-interop.md) · [ADR‑0013](./0013-sync-bidireccional-stock.md) · [ADR‑0014](./0014-dss-storefront-integration.md)
- Cloud: [ADR‑0018](./0018-cloud-multitenant-saas.md) · Negocio: [`freemium-master-plan.md`](../strategy/freemium-master-plan.md)

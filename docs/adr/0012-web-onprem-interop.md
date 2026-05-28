# ADR-0012: Interop web (Tu Farmacia) ↔ pharma-server vía HTTP only

- **Status**: Accepted
- **Date**: 2026-05-24
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: producto, infra, protocolo, interop

## Context and Problem Statement

`pharma-server` es el ERP on-prem (Rust + SurrealKv embedded, MSI Windows) que vive en
la LAN de cada farmacia. `build-and-deploy-webdev-asap` ("Tu Farmacia") es la web pública
del local real en Coquimbo: Next.js 14 + Cloud SQL Postgres 15 + Vercel. Son
**dos productos separados, dos repos separados, dos stacks separados**
(ver [CLAUDE.md § "Scope de este repo"](../../CLAUDE.md), regla "no cross-imports,
no shared CI, no shared deploy").

A medida que más farmacias adopten pharma-server (post-pivote freemium MSI,
[ADR-0001](./0001-freemium-pivot.md)), va a surgir la pregunta operativa:
*"tengo mi web/storefront ya andando; ¿cómo la conecto al ERP local sin romper la
separación?"*. Esta ADR fija la respuesta antes de que algún operador (o nosotros mismos
en Coquimbo) reinvente el wiring de manera frágil.

Restricciones duras:
- Cero cross-imports de código entre repos.
- Cero shared CI / shared deploy.
- Pharma-server permanece offline-first y LAN-only por default
  ([ADR-0005](./0005-core-gratis-no-locked-in.md), invariantes 1, 2, 4).
- La web pública no puede asumir que pharma-server es alcanzable desde internet —
  típicamente NO lo es (NAT residencial, firewall del comercio).
- Cliente es dueño de sus datos → el formato del payload debe ser estable,
  versionado y exportable (JSON sobre HTTP).

## Decision Drivers

- **Acoplamiento mínimo**: el día que cualquiera de los dos repos cambie de stack,
  el otro no debe enterarse.
- **Operabilidad por el dueño de la farmacia**, no por nosotros: configuración
  declarativa (env vars + un toggle), no ingeniería bespoke.
- **Seguridad por defecto**: la superficie expuesta a internet es opt-in, autenticada
  (API key + HMAC), y mínima en endpoints.
- **Idempotente y reintentable**: pull/push pueden fallar (red residencial); ningún
  side-effect debe duplicarse al reintentar.
- **Compatibilidad con Fase 12** (sync online opt-in entre nodos): el mismo patrón
  HTTP debe servir tanto para web↔on-prem como para on-prem↔on-prem futuro.

## Considered Options

1. **HTTP only entre repos** — pharma-server expone endpoints públicos opt-in;
   la web los consume (pull) o los recibe vía webhook (push). Sin shared code.
2. **DB compartida** — pharma-server escribe directo a Cloud SQL del web (o viceversa).
3. **Replicación directa Postgres ↔ SurrealKv** — replicación lógica entre los dos
   motores de DB.
4. **GraphQL Federation** — gateway federado que une los schemas de ambos.
5. **gRPC bidi streaming** — canal persistente entre web y on-prem con protobuf.

## Decision Outcome

**Elegida: Opción 1 (HTTP only entre repos, sin shared code)**, con tres patrones
de integración soportados oficialmente:

### Patrón A — Pull (web ← pharma-server) · DEFAULT RECOMMENDED

La web actúa como cliente HTTP del pharma-server. En build-time o cron (Vercel cron /
Cloud Scheduler / GitHub Action), llama:

```
GET https://<pharma-host>/api/v1/public/catalog?tenant=<slug>
Authorization: Bearer <PHARMA_PUBLIC_READ_KEY>
```

La respuesta es un JSON estable, paginado, con productos publicables (subset del
catálogo interno: `sku`, `name`, `price`, `category`, `image_url`, `stock_status`).
La web mirror-ea ese subset en Cloud SQL (tabla `products`). El POS del local sigue
trabajando local; la web lee del mirror.

Por qué es el default:
- Pharma-server queda detrás de NAT sin tener que abrir puerto entrante
  *si* el operador hostea pharma-server en VPN/tunnel (Cloudflare Tunnel, Tailscale
  Funnel) o si el server tiene IP pública estática.
- Read-only desde web → cero riesgo de corrupción del estado autoritativo (que vive
  en pharma-server).
- Latencia desacoplada: storefront sirve desde el mirror, no depende de la red del
  local.
- Modelo operativo simple: 1 endpoint público, 1 API key read-only.

Endpoint requerido (futuro, no implementado en este ADR): `GET /api/v1/public/catalog`
auth-gated por API key específica (NO el JWT regular del POS), tenant-scoped, sólo
lectura, opt-in en `config/local.toml` (`[public_catalog] enabled = false` por default).

### Patrón B — Push in-store → web (webhook saliente desde pharma-server)

Cuando cambia stock o precio en pharma-server, el server emite un webhook saliente:

```
POST https://<web>/api/webhooks/pharma-stock
X-Pharma-Signature: <HMAC-SHA256(body, SHARED_SECRET)>
Content-Type: application/json
```

El webhook lo procesa un endpoint del repo web (responsabilidad del web: validar
HMAC, idempotencia por `event_id`, aplicar mutación a Cloud SQL). Pharma-server
no espera respuesta más allá de `2xx`; si falla, reintenta con backoff
exponencial limitado.

Cuándo usarlo: storefront necesita freshness <5 min (vs el batch del Patrón A).
Trade-off: más infra (queue de retries en pharma-server, endpoint en el web).

### Patrón C — Push web → on-prem (online orders desde el storefront)

Cuando un cliente compra online, el web envía la orden al pharma-server:

```
POST https://<pharma-host>/api/v1/public/orders/web
X-Pharma-Signature: <HMAC-SHA256(body, SHARED_SECRET)>
Idempotency-Key: <uuid>
Content-Type: application/json
```

Pharma-server valida HMAC + Idempotency-Key, crea la orden en el tenant correcto,
descuenta stock, y responde 201 con el `order_id` interno. La web la mostrará como
"recibida en farmacia".

Endpoint futuro (no implementado en este ADR): `POST /api/v1/public/orders/web`.
Requiere que pharma-server sea alcanzable desde internet (Patrón C es el caso que
fuerza ese requisito; A y B no).

### Consequences

#### Positivas
- Cero acoplamiento de código → cada repo evoluciona su stack libremente.
- Cero acoplamiento de CI/CD → un break del pipeline de uno no rompe al otro.
- Cumple invariantes de [ADR-0005](./0005-core-gratis-no-locked-in.md): el server sigue
  funcionando aunque la web esté caída, y viceversa.
- Patrón A no requiere abrir puerto entrante en la farmacia → adopción incremental.
- El mismo protocolo HMAC (Patrones B/C) se reutiliza para sync online entre nodos
  (Fase 12), reduciendo superficie nueva.
- Vendor-agnostic: cualquier storefront (Next.js, Astro, Shopify headless, WordPress)
  puede implementar el cliente del Patrón A en <100 líneas.

#### Negativas
- Drift de schema entre payload publicado y schema interno de pharma-server: mitigado
  versionando el endpoint (`/api/v1/...`) y publicando un JSON Schema en
  `docs/strategy/web-interop.md`.
- Latencia del Patrón A (eventual consistency en el storefront). Mitigado vía
  Patrón B opt-in.
- Operador debe gestionar 2 secrets distintos (`PHARMA_PUBLIC_READ_KEY` para A,
  `PHARMA_WEBHOOK_SHARED_SECRET` para B/C). Mitigado por docs paso-a-paso en
  `docs/strategy/web-interop.md`.

#### Neutras
- El web sigue siendo libre de no integrarse en absoluto (Tu Farmacia hoy no lo hace).
- Ningún patrón es obligatorio para vender la licencia MSI; son aditivos.

## Pros and Cons of the Options

### Opción 1: HTTP only (elegida)
- **Pros**: ver consequences positivas. Simple, observable (logs HTTP standard),
  reintentable, debuggeable con curl.
- **Cons**: ver consequences negativas. Eventual consistency en Patrón A.

### Opción 2: DB compartida
- **Pros**: latencia cero, sin duplicar datos.
- **Cons**: **viola el scope del repo** (forzaría a pharma-server a hablar Postgres,
  contradiciendo SurrealKv embedded como tesis). Acopla el deploy del web al schema
  interno. Imposible de operar para una farmacia que no tiene Cloud SQL — sólo
  serviría a Tu Farmacia, no al producto vendible. Rechazado.

### Opción 3: Replicación Postgres ↔ SurrealKv
- **Pros**: bidireccional automático.
- **Cons**: no existe replication driver entre los dos motores; habría que
  construirlo. Complejidad operativa enorme (resolución de conflictos, schema
  evolution doble). Acoplamiento extremo. Rechazado.

### Opción 4: GraphQL Federation
- **Pros**: schema único cara al consumidor.
- **Cons**: requiere un gateway extra (otro servicio que operar). Pharma-server
  hoy expone OpenAPI vía utoipa, no GraphQL — habría que duplicar capa. Overhead
  desproporcionado a un mirror de catálogo. Rechazado.

### Opción 5: gRPC bidi streaming
- **Pros**: tipado fuerte, streaming push gratis.
- **Cons**: requiere conexión persistente (mal fit con NAT residencial), protobuf
  schema sync entre repos (vuelve el problema de Opción 2 vestido distinto),
  debug menos accesible para un operador. Rechazado para v1; reevaluable en Fase 12
  si hace falta canal persistente entre nodos.

## More Information

- [`../strategy/web-interop.md`](../strategy/web-interop.md) — guía operador
  paso-a-paso (cómo configurar API key, qué endpoint exponer, cómo verificar).
- [`../../scripts/web-sync/`](../../scripts/web-sync/) — script de referencia
  Node 20+ que implementa el cliente del Patrón A (corre en el entorno del web,
  no en pharma-server).
- [ADR-0001](./0001-freemium-pivot.md) — pivote freemium MSI (contexto comercial).
- [ADR-0004](./0004-license-server-separado.md) — precedente de repo separado
  para componente diferente de stack.
- [ADR-0005](./0005-core-gratis-no-locked-in.md) — invariantes ofline-first
  respetadas por esta ADR.
- [CLAUDE.md § "Scope de este repo"](../../CLAUDE.md) — regla original "no
  cross-imports" que esta ADR codifica.
- Roadmap Fase 12 — "Sync online opt-in entre nodos" (paid tier); este ADR
  establece el patrón HTTP+HMAC que Fase 12 reutilizará.

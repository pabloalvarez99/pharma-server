# ADR-0013: Stock sync ERP→web push + canonical truth matrix

- **Status**: Accepted
- **Date**: 2026-05-24
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: producto, infra, protocolo, interop

## Context and Problem Statement

[ADR-0012](./0012-web-onprem-interop.md) fijó tres patrones HTTP para interop
web ↔ pharma-server: (A) pull catálogo, (B) push stock, (C) push pedidos. El
default recomendado fue **sólo Patrón A** (pull en build/cron desde el web),
porque cubre el 80% de los casos y no requiere endpoint nuevo en el server.

Patrón A tiene una limitación dura: **el storefront siempre muestra stock
viejo**. Si el cron es horario, una venta POS de las 10:05 no aparece como
"agotado" en la web hasta las 11:00. Para la farmacia de Coquimbo (donde la
mayoría de productos rota poco) eso es tolerable. Para farmacias con SKUs
calientes (test de embarazo, ibuprofeno, paracetamol pediátrico) y para
omnichannel real (cliente reserva online, pasa a buscar), el delay rompe
expectativas y produce overselling.

Patrón B (push stock) quedó documentado como contrato pero **sin diseño
concreto** de qué dispara el push, cómo se reintenta, cómo se reconcilia ante
fallo, ni qué pasa si web y ERP discrepan. Falta también el **modelo
canónico**: ¿quién es la verdad para precio? ¿quién para stock? ¿quién resuelve
conflictos cuando ambos lados editaron lo mismo?

Esta ADR cierra ambos huecos:
1. Diseña el push de stock ERP→web (qué dispara, cómo se firma, cómo
   reintenta, cómo se reconcilia).
2. Fija la matriz de verdad canónica entre ambos sistemas.

## Decision Drivers

- **Eventual consistency aceptada, divergencia inaceptable**: el storefront
  puede atrasarse minutos, pero nunca debe quedar fuera de sync indefinidamente.
- **ERP es la verdad para stock** (es donde el POS decrementa). El web no puede
  reescribir stock — sólo refleja.
- **Web es la verdad para precio y catálogo publicable** (es donde el operador
  edita campañas, descripciones marketing, imágenes). El ERP no impone precio
  online — sólo informa costo y disponibilidad.
- **Sin conexión persistente**: invariantes ADR-0005 y ADR-0012 prohíben
  asumir tunnels permanentes o WebSockets desde la LAN del local.
- **Idempotencia obligatoria**: webhooks fallan; reintentos no deben duplicar
  efectos.
- **Offline-first se preserva**: si la web está caída, el POS sigue vendiendo;
  pharma-server acumula deltas y los empuja cuando la web vuelve.
- **Mínima superficie de seguridad**: un secret HMAC dedicado, no JWT regular.

## Considered Options

1. **Webhook push ERP→web con HMAC + retry con backoff bounded + reconcile
   nightly vía pull-catalog** (Patrón B concretizado).
2. **WebSocket persistente ERP↔web** — canal duplex, push instantáneo.
3. **Polling agresivo del web** — el web pulla `/api/v1/public/catalog` cada
   30s en vez de cada hora.
4. **DB compartida** — pharma-server escribe directo a Cloud SQL del web.
5. **NATS / Redis pub-sub** — broker intermedio que recibe del ERP y fanout al
   web.

## Decision Outcome

**Elegida: Opción 1 — Webhook push ERP→web con HMAC + retry bounded + reconcile
nightly**. Concretiza Patrón B de [ADR-0012](./0012-web-onprem-interop.md) con
contrato preciso, política de retry, modelo de fallo y matriz de verdad.

### Diseño del push

**Trigger**: emitir webhook por cada `stock_movement` con `delta != 0` cuyo
producto esté marcado `publish_to_web = true` en el catálogo. Tipos de
movimiento que disparan:
- `pos.sale` — venta normal POS (delta negativo).
- `pos.refund` — devolución (delta positivo).
- `po.receive` — recepción de orden de compra (delta positivo).
- `manual.adjust` — ajuste manual desde admin (delta cualquiera).
- `expiry.write_off` — write-off por vencimiento (delta negativo).

NO disparan (ruido innecesario):
- `inventory.recount` con delta == 0 (re-confirma stock).
- Movimientos en SKUs con `publish_to_web = false` (productos internos, no
  comercializados online).
- Movimientos en tenants sin `[webhooks.stock]` configurado.

**Debounce/coalesce server-side**: si llegan N movements del mismo `sku` dentro
de una ventana de 2 segundos, se emite **un solo webhook** con el stock final
(no N webhooks). Esto absorbe ráfagas (e.g. carga inicial de stock, ajustes
masivos). El campo `new_stock` siempre refleja el valor *después* del último
delta dentro de la ventana.

**Web debounce (opcional, recomendado)**: el web puede acumular eventos por
`sku` por unos segundos antes de hacer UPDATE en Cloud SQL, para reducir
locks. Es responsabilidad del web; el contrato del ERP es exactamente-uno-por-cambio
(modulo coalescing).

### Contrato del payload

```http
POST https://<web>/api/webhooks/pharma-stock
Content-Type: application/json
X-Pharma-Signature: sha256=<HMAC-SHA256(body, stock_webhook.hmac_secret)>
X-Pharma-Timestamp: 2026-05-24T15:34:12Z
X-Pharma-Tenant: coquimbo-centro
Idempotency-Key: <uuid v7 monótono por movimiento, o hash determinístico>

{
  "schema_version": "1.0",
  "tenant_slug": "coquimbo-centro",
  "external_id": "PARA-500-20",
  "new_stock": 42,
  "in_stock": true,
  "ts": "2026-05-24T15:34:12Z",
  "idempotency_key": "01J0K5R8X2..."
}
```

Campos:
- `tenant_slug`: para multi-tenant, mismo formato que `?tenant=` del pull.
- `external_id`: SKU comercial (el mismo campo que expone `/public/catalog`).
- `new_stock`: stock **final** después del movimiento. Nunca el delta — el
  delta no es idempotente bajo retry.
- `in_stock`: `new_stock > umbral_minimo`. Conveniencia para el web (evita
  recomputar reglas de mostrar/ocultar).
- `ts`: timestamp del movimiento que disparó el webhook (no del envío). Usado
  por el web para descartar eventos out-of-order: si `ts < last_applied_ts`
  para ese `sku`, el web descarta sin aplicar.
- `idempotency_key`: UUID v7 (incluye timestamp) o hash determinístico del
  movement_id interno. Web persiste `idempotency_key` en una tabla
  `webhook_received` y skipea duplicados.

### Autenticación

**HMAC-SHA256** sobre el body raw, con un secret dedicado:
- Config server: `[webhooks.stock] hmac_secret_env = "PHARMA_STOCK_WEBHOOK_SECRET"`.
- Secret se genera con `openssl rand -hex 32` (32 bytes random).
- Secret vive **sólo en env vars**, nunca en `config/local.toml` commiteado.
- Header `X-Pharma-Signature: sha256=<hex>` — formato GitHub-webhooks
  compatible.
- Web verifica con timing-safe compare (`crypto.timingSafeEqual` en Node).
- Replay defense: rechazar `X-Pharma-Timestamp` con drift > 5 min vs `Date.now()`.

> Este secret es **distinto** del `PHARMA_PUBLIC_READ_KEY` usado por Patrón A.
> Razones: scope diferente (write-to-web vs read-from-erp), rotación
> independiente, blast radius reducido si uno se filtra. Misma justificación
> que ADR-0012 § Negativas.

### Política de retry y fallo

ERP es canónico → el web es best-effort. Si el web falla, el ERP NO bloquea
ventas. Política:

| Intento | Espera previa | Trigger del siguiente |
|---|---|---|
| 1 (inmediato) | 0s | 5xx, timeout >5s, conexión rechazada |
| 2 | 1s | 5xx, timeout |
| 3 | 5s | 5xx, timeout |
| 4 (último) | 30s | Cualquier no-2xx |

Tras 3 reintentos fallidos (4 intentos en total: 1+5+30s = 36s max wall-clock),
el ERP:
- **Drop**: el webhook se descarta. NO se persiste para reintento posterior
  (evita memory leak en outages largos).
- **Log WARN**: `tracing::warn!(event_id, sku, last_status, "stock webhook
  giving up after 4 attempts")`.
- **Métrica**: `pharma_stock_webhook_dropped_total{tenant,reason}` para
  alerting/observabilidad.
- **NO bloquea** la transacción POS. La venta ya está en el ERP; el webhook es
  side-effect best-effort.

Status 4xx (excepto 408/429) se considera **error de contrato** (firma
inválida, payload malformado, tenant inválido en el web). NO se reintenta —
log ERROR y drop inmediato. Esto evita martillar el web con payloads que
nunca van a ser aceptados.

### Reconcile vía pull-catalog nightly

El push best-effort puede dropear eventos en outages prolongados (e.g. web
caída 1 hora durante CyberMonday). Para garantizar **convergencia eventual**,
el web debe correr el script `scripts/web-sync/pull-catalog.mjs` (Patrón A)
**al menos una vez al día**, recomendado cron nocturno (e.g. 03:00 hora local).

Este pull es la verdad reconciliadora: trae **todo** el catálogo publicable
del tenant, incluyendo `in_stock`. Cualquier drift acumulado por webhooks
dropeados se corrige acá. Es exactamente el mismo endpoint que ya existe en
[`crates/api/src/v1/public_catalog.rs`](../../crates/api/src/v1/public_catalog.rs);
no se agrega superficie nueva para reconcile.

**Importante**: pull-catalog usa `external_id` como llave; el UPSERT del web
debe ser por `(tenant_slug, external_id)`. Si un row en Cloud SQL tiene un
`in_stock` distinto del que viene del pull, **gana el pull** (ERP-canónico).
Esto formaliza la regla "cuando pull y webhook discrepan, gana el más reciente
en wall-clock, y el pull se asume implícitamente más reciente a las 03:00".

### Matriz de verdad canónica

Esta es la fuente única de resolución de conflictos cuando los dos sistemas
tienen el mismo campo con valores distintos:

| Dominio / Campo | Canon | Razón | Replicado a |
|---|---|---|---|
| **Catálogo: name, laboratory, category, active_ingredient, image_url** | Web | El operador los edita en el admin del web (marketing-friendly, multi-canal). | ERP via import manual / CSV (no auto). |
| **Catálogo: external_id (SKU)** | ERP | Es la llave operativa interna; el web la copia pero no la inventa. | Web via Patrón A. |
| **Precio de venta** | Web | El operador maneja precios online (campañas, descuentos por canal). Patrón A trae un "precio sugerido" del ERP, pero el web es el que decide. | ERP no recibe; el ERP sigue usando su precio interno para POS. |
| **Precio de venta POS** | ERP | El POS físico tiene su propia tabla de precios (puede diferir del web por convenios isapre, descuentos cash, etc.). | Web no recibe. |
| **Costo, margen, proveedor** | ERP | No se publica. Nunca sale del LAN. | Nada. |
| **Stock (cantidad)** | ERP | El POS decrementa; el ERP es el único que escribe stock real. | Web via Patrón B push + Patrón A reconcile. |
| **Stock (booleano `in_stock`)** | ERP | Derivado de stock + umbral_minimo. | Web via Patrón B push + Patrón A reconcile. |
| **Pedido online (creado por el cliente)** | Web (origen) → ERP (autoritativo tras aceptar) | El cliente compra online, el web crea el pedido y lo POSTea al ERP via Patrón C. Una vez aceptado en el ERP, el ERP es la verdad (estado de preparación, stock descontado, etc.). | Web recibe updates de estado via Patrón B (futuro: webhook `order_status_changed`). |
| **Cliente / RUT / dirección** | Web (si vino de pedido online); ERP (si vino del POS físico) | Cada canal crea su propia ficha. Reconciliar es problema separado (Fase 13 marketplace o módulo CRM). | No replicado en v1. |
| **Boleta electrónica DTE** | ERP | Sólo el ERP habla con SII. | Nada. |

**Regla de oro cuando un campo aparece en ambos lados con valores distintos**:
1. Si está en la matriz: gana el sistema canónico (sin merge).
2. Si NO está en la matriz: documentar acá antes de implementar — no inventar
   resolución ad-hoc.

### Consequences

#### Positivas
- Storefront refleja stock con latencia <10s típica (push) y converge en <24h
  garantizado (pull nightly), sin requerir conexión persistente.
- Cero overhead en el hot path del POS: el push se emite async desde un task
  Tokio, la transacción POS responde en <50ms aunque el webhook tarde.
- Matriz canónica explícita evita meses de "¿quién manda en X?" cuando
  aparezca el primer conflicto operativo real.
- Compatible con Fase 12 (sync online entre nodos): mismo protocolo
  HTTP+HMAC+idempotency-key se reusa para webhooks ERP→ERP.
- ERP-canónico para stock cierra la puerta a overselling: ningún sistema
  externo puede reservar stock (el web sólo muestra; el ERP es el único que
  decrementa). Pedidos online via Patrón C respetan la regla — el ERP rechaza
  con 409 si el stock cayó entre que el web lo mostró disponible y el POST.

#### Negativas
- Web puede mostrar stock obsoleto durante una outage del web (eventos
  dropeados tras 4 reintentos). Mitigado por reconcile nightly + opción del
  operador de correr `pull-catalog.mjs` on-demand.
- Operador maneja **3 secrets distintos** ahora:
  `PHARMA_PUBLIC_READ_KEY` (A), `PHARMA_STOCK_WEBHOOK_SECRET` (B),
  `PHARMA_ORDERS_WEBHOOK_SECRET` (C). Mitigado por docs + comando CLI futuro
  `pharma secrets rotate-all`.
- Coalescing 2s puede ocultar movimientos individuales del web (que ve sólo el
  estado final). Aceptado: el web no necesita historia, sólo "current stock".
  La historia vive en el ERP (audit log).

#### Neutras
- Hace falta una crate o módulo `crates/api/src/webhooks/stock.rs` nuevo
  (futuro, no en esta ADR).
- Schema del payload está versionado (`schema_version: "1.0"`); bumps
  requieren ADR o sub-ADR cuando lleguen.

## Pros and Cons of the Options

### Opción 1: Webhook push + HMAC + retry bounded + reconcile (elegida)
- **Pros**: ver consequences positivas. Eventual consistency garantizada por
  pull nightly; sin conexión persistente; matriz de verdad clara.
- **Cons**: 3 secrets, eventos pueden dropearse en outages, latencia 0-10s
  (no instantánea).

### Opción 2: WebSocket persistente
- **Pros**: latencia <100ms, push gratis.
- **Cons**: requiere conexión TCP saliente permanente desde la farmacia →
  fragiliza ante NAT/firewall residencial (mismo problema que ADR-0012
  rechazó para Opción 5 gRPC). Reconnect logic, keepalives, heartbeats =
  superficie de complejidad nueva. Si el WS se cae, ¿cómo se replay-ea? Vuelve
  el problema de reconcile. Rechazado.

### Opción 3: Polling agresivo del web
- **Pros**: cero código nuevo en el ERP — Patrón A ya funciona.
- **Cons**: 30s de polling = 2880 requests/día/tenant × N tenants = costo Cloud
  Run del web significativo, y aún así latencia 0-30s. Empeora al subir
  frecuencia. Wasteful: 99% de los pulls devuelven sin cambios. Rechazado.

### Opción 4: DB compartida
- **Cons**: violado por [ADR-0012 § Opción 2 rechazada](./0012-web-onprem-interop.md).
  pharma-server es SurrealKv embedded; el web es Cloud SQL Postgres. Acoplar
  schemas viola separación de repos ([ADR-0004](./0004-license-server-separado.md)
  precedente). Rechazado.

### Opción 5: NATS / Redis pub-sub
- **Pros**: fanout multi-suscriptor barato, replay con persistencia.
- **Cons**: agrega broker que operar (server o servicio managed). La farmacia
  no tiene infra para NATS local; el broker tendría que vivir en el VPS del
  web → vuelve el problema de conexión persistente saliente. Sobre-engineering
  para 1 productor → 1 consumidor. Rechazado para v1; reevaluable cuando haya
  fanout real (N webs / N agentes federados Fase 13).

## Next steps (implementación)

Esta ADR es design-only. Implementación va en **una rama nueva**
`feat/api-stock-webhook` con el siguiente scope:

1. **Config**: agregar `[webhooks.stock]` a `AppConfig` (en `crates/core/src/config.rs`):
   `enabled`, `url`, `hmac_secret_env`, `tenants[]`, `coalesce_window_ms`
   (default 2000), `publish_to_web_filter` (default `true` → sólo SKUs con
   ese flag).
2. **Migración**: campo `publish_to_web: bool` (default false) en tabla
   `product`. Append-only — nueva migración `NNNN_product_publish_flag.surql`.
3. **Emisor**: módulo `crates/api/src/webhooks/stock.rs` con un canal
   `tokio::mpsc` que recibe `StockChanged` events desde los handlers POS / PO
   / adjustments y los coalesce + envía con `reqwest`.
4. **Hook en stock_movements**: cada `domain::stock::record_movement` envía al
   canal cuando `publish_to_web == true && delta != 0`.
5. **Retry**: política exacta de esta ADR (1+5+30s, 4 intentos, drop+WARN).
   Métrica `pharma_stock_webhook_*_total` (sent, succeeded, retried, dropped).
6. **Tests**: integration test con `wiremock` levantando un mock del web,
   validando: payload schema, HMAC correcto, idempotency-key único por
   movimiento, coalescing 2s, retry 5xx, drop tras 4 intentos, no-retry tras
   4xx contractual, no-fire para `publish_to_web=false`, no-fire para tenants
   sin config.
7. **CLI**: `pharma webhook test-stock --tenant <slug>` que dispara un
   webhook sintético para verificar conectividad end-to-end (mismo patrón que
   `pharma license reload`).
8. **Docs**: actualizar
   [`docs/strategy/web-interop.md`](../strategy/web-interop.md) sección
   "Patrón B" con el contrato real implementado.

Lo que NO va en `feat/api-stock-webhook`:
- Webhook de orders state changes (Patrón B variante "order_status_changed")
  → ADR / rama separada cuando llegue Patrón C completo.
- Replay/replay-from-cursor → no se diseña hasta que sea problema real.
- Multi-suscriptor (más de un web por tenant) → Fase 13 marketplace.

## More Information

- [ADR-0012](./0012-web-onprem-interop.md) — fija los 3 patrones HTTP; esta ADR
  concretiza el Patrón B y agrega matriz canónica.
- [ADR-0005](./0005-core-gratis-no-locked-in.md) — invariante offline-first
  respetada: si el web cae, el POS sigue.
- [ADR-0004](./0004-license-server-separado.md) — precedente de separación
  cross-repo (HTTP, no shared schema).
- [`../strategy/web-interop.md`](../strategy/web-interop.md) § "Stock sync" —
  guía operador derivada de esta ADR.
- [`../../crates/api/src/v1/public_catalog.rs`](../../crates/api/src/v1/public_catalog.rs) —
  endpoint del Patrón A; reutilizado como mecanismo de reconcile nightly.
- Roadmap Fase 12 — sync online opt-in entre nodos; el mismo
  HTTP+HMAC+idempotency-key se aplicará para webhooks ERP→ERP.

# ADR-0016: Arquitectura del agente "Pregúntale a tu negocio" (assist)

- **Status**: Accepted
- **Date**: 2026-06-19
- **Deciders**: pabloalvarez99 (fundador), milton (lane V1)
- **Tags**: producto, north-star, agente, offline-first, license

## Context and Problem Statement

La tesis entera del producto es **RutBusiness: 1 RUT = 1 negocio = 1 agente IA**
(GOAL fijado 2026-06-16, ver [`docs/strategy/rutagentia-vision.md`](../strategy/rutagentia-vision.md)).
El ERP es la infraestructura invisible; el diferenciador es que el dueño **le
habla a su negocio** y obtiene respuestas fundadas en sus propios datos:
"¿cuánto vendí hoy?", "¿qué se vence?", "stock de paracetamol", "¿cuánto hay en
caja?".

Hasta ahora ese agente no existía en código. Construir el MVP plantea tres
tensiones de diseño:

1. **Offline-first es SAGRADO** ([ADR-0005](./0005-core-gratis-no-locked-in.md)
   invariantes 1, 2, 6): el core gratis opera sin internet, sin CDN, sin cloud.
   Un agente que dependa de una API LLM externa viola esto de raíz.
2. **El mercado espera "IA"**, que hoy connota LLM. Pero un LLM en el hot path
   implica red, costo por token, latencia, y enviar datos del tenant a un
   tercero (riesgo Ley 19.628 + lock-in).
3. **No podemos pintarnos a una esquina**: si más adelante el fundador quiere
   ofrecer un LLM (opt-in, con la key del propio dueño), la arquitectura del MVP
   no debe requerir reescritura.

## Decision Drivers

- Offline-first no negociable (ADR-0005). El agente debe responder **sin red**.
- Determinismo y testabilidad: respuestas auditables, fundadas en datos reales,
  cubiertas por tests — no "alucinaciones".
- Costo cero ongoing en el tier gratis (sin tokens, sin infra).
- Extensibilidad hacia un proveedor LLM **opt-in** (key del dueño, default OFF,
  estilo telemetría — ADR-0005 invariante 3) sin rehacer el endpoint ni el core.
- Privacidad: por default los datos del tenant **nunca** salen de la máquina.

## Decision

Construir el agente como **parser determinístico de intents es-CL + ejecutores
read-only sobre los servicios de dominio existentes**, detrás de un **seam de
proveedor** (`AssistProvider`) que permite enchufar un LLM opt-in después.

### Componentes (crate nuevo `crates/assist`)

- **`intent.rs`** — `parse(question) -> Intent`. Normaliza (lowercase + strip de
  acentos) y matchea keywords/patrones es-CL contra un set **cerrado** de
  intents. Conservador: lo que no clasifica con confianza cae en
  `Intent::Unknown` → respuesta amable que sugiere preguntas válidas. Cero ML,
  cero red.
- **`provider.rs`** — el seam:
  ```rust
  #[async_trait]
  pub trait AssistProvider: Send + Sync {
      async fn answer(&self, q: &AssistQuery<'_>) -> DomainResult<Answer>;
  }
  ```
  `AssistQuery` lleva la pregunta cruda + el intent parseado + handle read-only a
  la DB + el `tenant`. `Answer { intent, text (español), data? }`.
- **`deterministic.rs`** — `Deterministic`, la impl por **default**. Cada intent
  llama a un servicio de **lectura** existente (`expenses::sales_daily`,
  `expenses::near_expiry`, `expenses::top_products`, `expenses::margins_daily`,
  `catalog::stats`/`list_products`, `cash_register::compute_summary`,
  `inventory::reorder_suggestions`) y compone una respuesta en español + payload
  estructurado. **Nunca muta**: solo toca el read path.

### Endpoint (`crates/api/src/v1/assist.rs`)

`POST /api/v1/assist/ask { question } -> { answer, intent, data? }`.
Read-only, tenant-scoped (JWT `tenant_id`), role-gated (`cashier_plus`). Instancia
el `Deterministic` provider (stateless). El intent de **margen** honra el gate de
licencia `reports.margins_daily`: en el tier Free **degrada** a un nudge de
upgrade en vez de 402-ear — el agente siempre responde algo, nunca le explota en
la cara al dueño (consistente con la postura de `dashboard.rs`).

### Intents v1 (set cerrado)

`ventas_hoy` · `ventas_mes` · `por_vencer` · `stock_producto` · `caja_actual` ·
`top_productos` · `margen_mes` · `stock_bajo` · `resumen_inventario` · `ayuda` ·
`desconocido`. Cada uno con sinónimos es-CL y cubierto por tests (parser +
ejecutor contra kv-mem sembrado + aislamiento de tenant).

### Intents v2 (profundidad — Wave 2, 2026-06-20, milton)

El set se amplía a **20 variantes** agregando las preguntas de alta frecuencia del
dueño, todas mapeadas a servicios de **lectura** existentes (cero servicios nuevos,
cero migración, read-only sagrado). El **contrato del endpoint NO cambia**: sigue
siendo `{ intent, text, data? }` (campo prosa = `text`; ye construye la UI contra
esa forma). Solo se **agregan** intents/contenido.

Nuevos: `ventas_ayer` · `ventas_mes_pasado` · `ventas_vs_ayer` (comparativa día) ·
`ventas_vs_mes_pasado` (comparativa mes) · `ventas_metodo_pago` (efectivo vs
tarjeta) · `por_vencer_semana` (ventana 7d) · `clientes_top` (loyalty) ·
`margen_producto` (margen unitario de un SKU) · `gastos_mes` (total + top
categorías).

Mapeo a lecturas: comparativas/método/mes-pasado/ayer → `expenses::sales_daily`
con distintos rangos; `gastos_mes` → `expenses::list_expenses`; `clientes_top` →
`customers::loyalty_stats`; `margen_producto` → `catalog::list_products` (price −
cost). El parser es-CL se endureció con desambiguación de orden (método antes de
caja y de comparativas; clientes antes de top-productos; margen-producto vs
margen-mes por palabra-período; vence-semana antes de vence-30d) y fechas relativas
(`hoy/ayer/esta semana/este mes/mes pasado`). `margen_producto` honra el mismo gate
de licencia `reports.margins_daily` que `margen_mes` (degrada a nudge, no 402).

## Camino LLM opt-in (futuro, NO en este MVP)

El seam ya está listo. Cuando el fundador lo decida:

### Cómo se prende (el flip, Wave 2)

La plomería ya existe: `assist::AssistConfig { llm_enabled: bool (default OFF),
llm_api_key: Option<String> }` + `assist::select_provider(&cfg) -> Box<dyn
AssistProvider>`. Hoy `select_provider` **siempre** devuelve `Deterministic` (no
hay `LlmProvider` compilado; cero red en este build aunque `llm_active()` sea
true — solo loguea un warning). El endpoint ya llama a `select_provider(&AssistConfig::default())`,
así que enchufar el LLM es un cambio **aislado**, sin tocar endpoint/parser/ejecutores:

1. Implementar `LlmProvider` detrás de `AssistProvider` y devolverlo en
   `select_provider` cuando `cfg.llm_active()`.
2. Cablear `AssistConfig` desde la config real del server (env/`admin_setting`),
   default OFF, key del dueño guardada local.

El resto del camino opt-in:

## Camino LLM opt-in (futuro, NO en este MVP)

El seam ya está listo. Cuando el fundador lo decida:

1. Nueva impl `LlmProvider` detrás de `AssistProvider`, **sin tocar** el endpoint
   ni el parser. Recibe el mismo `AssistQuery` (incluida la pregunta cruda y el
   handle read-only a la DB para tool-calling).
2. **Opt-in explícito** (default OFF), con la API key **del propio dueño**
   guardada local (mismo patrón de consentimiento que telemetría, ADR-0005 inv.
   3). Sin key → se usa `Deterministic`, todo offline.
3. El proveedor LLM podría: (a) usar el intent determinístico como herramienta
   estructurada, o (b) hacer su propio tool-calling sobre los servicios de
   lectura. En ambos casos el core gratis sigue 100% offline y funcional.
4. Datos del tenant solo salen de la máquina si el dueño activó explícitamente el
   proveedor LLM. Default = privacidad total.

## Consequences

**Positivas**
- Agente real, funcional, **offline-first**, en el tier gratis, costo cero.
- Respuestas deterministas, auditables, testeadas — sin alucinaciones.
- Privacidad por default; cumple Ley 19.628 sin esfuerzo.
- Seam limpio: el LLM opt-in se agrega sin deuda de reescritura.

**Negativas / límites aceptados**
- El parser determinístico entiende un set acotado de preguntas; fuera de él
  responde con un nudge en vez de "entender" lenguaje libre. Es el trade-off
  correcto para el MVP offline (el LLM opt-in cubrirá el long tail después).
- Mantener sinónimos es-CL es trabajo manual incremental (aceptable; barato).

## Alternatives Considered

- **LLM directo en el hot path (sin seam)** — rechazado: viola offline-first,
  costo por token, latencia, privacidad, lock-in. Pinta el producto a una
  esquina.
- **Solo búsqueda full-text sobre datos** — insuficiente: no compone agregados
  ("ventas hoy", "margen del mes") ni responde en prosa accionable.
- **Esperar a tener LLM para lanzar el agente** — rechazado: el diferenciador
  north-star quedaría sin implementar indefinidamente; el determinístico ya
  entrega el 80% del valor offline.

## Wave 3 — el agente actúa (write actions)

Fecha: 2026-06-20. El agente deja de ser solo read-only: ahora puede **ejecutar
un set cerrado y seguro de escrituras** sobre el ERP. El ERP se vuelve "las
manos del agente". Seguridad primero — cuatro candados, no negociables:

1. **Dos pasos PROPOSE → CONFIRM.** Una pregunta de escritura **nunca** ejecuta
   en el acto. `POST /assist/ask` devuelve una `ActionProposal`
   `{name, summary, params, confirm_token, expires_at}` y **no escribe nada**.
   La ejecución ocurre sólo cuando `POST /assist/act` recibe de vuelta ese
   `confirm_token`. Los `params` quedan **congelados server-side** al proponer y
   **no viajan** desde el cliente al confirmar → no se pueden manipular entre
   ambos pasos. El cliente sólo reenvía el token opaco.
2. **Whitelist CERRADA.** Sólo las variantes del enum `Action` pueden correr; no
   hay path de escritura arbitraria. v1 = `registrar_gasto` +
   `crear_orden_compra_draft`. Cada acción **reusa** el servicio de dominio de
   escritura existente (`expenses::create_expense`,
   `purchasing::create_purchase_order`); no se reimplementa lógica. La OC se crea
   en estado `draft` (emitir/recepcionar siguen siendo pasos humanos).
3. **Token server-issued, de un solo uso, expirable, tenant-bound.** Se consume
   atómicamente en el primer confirm válido (replay → rechazado), expira a los
   `TOKEN_TTL_SECS` (180 s), y está atado al tenant que lo propuso (un token de
   tenant A jamás ejecuta contra tenant B). Store en memoria, proceso-local
   (`assist::actions::ActionStore`): offline-first, sin DB ni red; un restart
   bota proposals pendientes (fallo seguro — el dueño vuelve a preguntar).
4. **Rol admin/owner + auditoría.** Las escrituras requieren `admin`/`owner`:
   `POST /assist/act` está gated por `role::layer(admin_plus())` → **403** claro
   para roles menores. `POST /assist/ask` sigue `cashier_plus` (read), pero si un
   rol menor pide una escritura, no se le emite token — se le responde un nudge
   amistoso (nunca 403 en `/ask`, que siempre responde algo). Cada ejecución
   escribe una fila en `audit_log` (`method='ACTION'`, `path='assist/act/<label>'`)
   — la **tabla existente** (migración `0002`), sin schema nuevo. El middleware
   HTTP de auditoría además registra el POST `/assist/act` genérico; la fila
   explícita agrega la granularidad de *qué acción* corrió.

**Contrato FROZEN para el cliente (ye):**
- `POST /assist/ask` → si es escritura, `Answer` con campo opcional `action`
  (`{name, summary, params, confirm_token, expires_at}`). Las respuestas de
  lectura **omiten** `action` (contrato de lectura intacto).
- `POST /assist/act` body `{confirm_token}` → `{action, text, data}` al ejecutar;
  `400` con mensaje es-CL si el token es inválido/expirado/reusado; `403` si el
  rol no alcanza.

**Determinismo:** el parser de acciones (`parse_action`) es keyword-based es-CL,
conservador — texto ambiguo → `Incomplete` (nudge) o `NotAnAction` (cae al
agente de lectura); **nunca** adivina una escritura. Mismo trade-off offline que
el resto del agente.

## Referencias

- [ADR-0005](./0005-core-gratis-no-locked-in.md) — invariantes offline-first / opt-in.
- [`docs/strategy/rutagentia-vision.md`](../strategy/rutagentia-vision.md) — visión 1 RUT = 1 agente.
- `crates/api/src/v1/dashboard.rs` — patrón de degradar feature license-gated a `null`/nudge en vez de 402.

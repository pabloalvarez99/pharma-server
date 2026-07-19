# RutAgentIA — Agentic Business Platform, visión multi-rubro (norte 2026+)

**Estado**: visión registrada (directiva fundador 2026-06-09). No reemplaza el roadmap Fases 9-14 — lo extiende como Fase 15+.
**Nombre del producto-plataforma**: **RutAgentIA** (directiva fundador 2026-06-09) — ver §1.5.
**Decisión raíz futura**: cuando se materialice código, abrir ADR propio (estilo ADR-0001).
**Tesis económico-estratégica** (por qué/cuándo de esta visión, etapas Tool→Worker→Team→Company, plan auto-financiado): [`saas-to-agentic-thesis.md`](./saas-to-agentic-thesis.md).

---

## 1. La directiva

> "I want to change this project not only for pharmacies — I want it for any type of business with an agent."
> — fundador, 2026-06-09

pharma-server deja de ser *sólo* un ERP de farmacia. La meta de largo plazo es una **plataforma de operación de negocios agéntica, vertical-agnostic**: cualquier tipo de negocio (farmacia, almacén, ferretería, distribuidora, restaurante…) operado por un humano que declara **objetivos**, y un sistema de **agentes IA** que los ejecuta sobre el software y los datos.

**Farmacia = beachhead, no boundary.** El vertical farmacia sigue siendo el primer mercado (tesis del independiente vs oligopolio, `market-thesis.md`), el que paga las cuentas y el que valida el producto. Pero el core se diseña desde hoy para que el segundo vertical sea un *pack*, no un fork.

## 1.5. Nombre y modelo de identidad: RutAgentIA — un agente para cada chileno, su RUT como ID

> "I want this project to be named **RutAgentIA**, which is an agent for every chilean with his RUT as ID. It manages every chilean RUT agent → finances, businesses, health, etc."
> — fundador, 2026-06-09

**RutAgentIA** es el nombre de la plataforma. Resuelve la deuda de naming de §4.6 (ya no se espera al segundo vertical) y a la vez **amplía el sujeto**: no sólo cada *negocio* tiene su agente — **cada chileno** (persona natural o jurídica) tiene su agente IA, anclado a su **RUT** (Rol Único Tributario), y ese agente le gestiona sus **dominios de vida**: finanzas, negocios, salud, etc.

**Un RUT = un agente = N dominios gestionados**:

```
RUT (persona o empresa)
  └─ Agente RutAgentIA (orquestador personal, identidad did:rut)
       ├─ Finanzas    — gastos, presupuesto, impuestos SII, pagos
       ├─ Negocios    — su ERP/nodo (pharma-server = primer vertical), compras, ventas
       ├─ Salud       — medicamentos, recetas, adherencia, interacciones (la farmacia ya está en el grafo)
       └─ …            — extensible por *dominio pack* (mismo patrón que vertical pack §4.1)
```

Por qué el RUT es el anchor de identidad correcto:
- **Universal en Chile**: toda persona y toda empresa ya tiene uno — cero fricción de onboarding, cero registro nuevo que inventar.
- **Ya es la identidad transaccional del país**: SII (boletas/facturas DTE — el crate `dte` ya gira en torno a RUT emisor/receptor), bancos, salud, contratos. El agente hereda el grafo transaccional existente.
- **Mapea 1:1 al modelo DID existente**: `crates/agent` ya implementa identidad Ed25519/DID + envelopes firmados. El binding `RUT ↔ DID ↔ keypair` convierte cada RUT en un endpoint agéntico verificable — `did:rut:76123456-7` como esquema conceptual.
- **B2B y B2C con la misma primitiva**: el agente de la farmacia (RUT empresa) y el agente del comprador (RUT persona) negocian con el mismo protocolo de envelopes firmados que hoy usa la federación `agent/inbox`. El marketplace federado B2B (Fase 13) se extiende naturalmente a B2C agente-a-agente.

**Sensibilidad regulatoria (registrar desde ya)**: un agente que cruza finanzas+salud+negocios de una persona identificada por RUT es máxima sensibilidad bajo Ley 19.628 (y la salud es dato sensible explícito). Los invariantes ADR-0005 (datos del usuario en su nodo, telemetría opt-in default OFF, sin PII, export completo, sin lock-in) son la base de confianza que hace viable siquiera proponer esto — RutAgentIA sólo funciona si el agente del chileno es *suyo*, corre para él, y sus datos no salen sin su instrucción.

**Alcance del rename (decidido vs pendiente)**:
- ✅ **Decidido hoy**: RutAgentIA = nombre de la plataforma/visión. Docs de estrategia y CLAUDE.md lo registran.
- ⏸ **Pendiente (tarea aparte, requiere go explícito)**: renombres físicos — repo GitHub, crates, binarios (`pharma-api`/`pharma-service`/`pharma`), MSI product name, mirror de releases, branding del cliente Tauri. Son outward-facing y rompen URLs/instalaciones existentes; se secuencian cuando el rebrand sea oportuno comercialmente (mínimo: post v1.0.0 pharma o al lanzar el segundo vertical/B2C). `pharma-server` sigue siendo el nombre del *nodo ERP vertical farmacia* dentro de la plataforma RutAgentIA.

## 2. Modelo operativo objetivo

Dos cadenas, la segunda es el principio rector de la primera:

```
Usuario ──(objetivos)──▶ Agente orquestador IA ──▶ Agentes coordinadores ──▶ Agentes de equipo ──▶ Tools
Humano ──▶ Agente IA ──▶ Software ──▶ Datos
```

- **El humano declara objetivos, no tareas**: "baja el stock muerto a <5%", "sube el margen de la categoría X 2 puntos", "prepara el cierre de mes". No clickea pantallas.
- **Agente orquestador IA**: traduce el objetivo en un plan, lo descompone, lo despacha y reporta progreso/resultado al humano. Es la única interfaz conversacional del humano con el sistema.
- **Agentes coordinadores**: dueños de un dominio (operaciones, comercial, finanzas, cumplimiento). Reciben sub-objetivos del orquestador y dirigen a su equipo.
- **Agentes de equipo**: especialistas ejecutores (inventario, ventas/POS, compras, caja, reportes, regulatorio). Hacen el trabajo concreto.
- **Tools**: la superficie de acción de los agentes = la **API HTTP/JSON versionada (`/api/v1`)** + CLI `pharma`. Los agentes no tocan la DB directo; actúan por la misma API auditada que usa un humano.

La cadena `Humano → Agente IA → Software → Datos` invierte el ERP tradicional (`Humano → Software → Datos`): el software deja de ser la interfaz del humano y pasa a ser la herramienta del agente.

## 3. Mapping a assets existentes

Esta visión NO parte de cero — el repo ya tiene los cimientos:

| Capa agéntica | Asset existente hoy |
|---|---|
| Identidad + firma de agentes | `crates/agent` (Ed25519 identity/DID + Envelope canonical-JSON firmado) |
| Tools (superficie de acción) | `/api/v1` axum + utoipa/OpenAPI (Swagger en `/docs`) + CLI `pharma` |
| Auditoría de acciones de agente | Audit log inmutable (toda mutación de stock/precio/venta ya queda registrada) |
| Multi-actor con permisos | Multi-tenant JWT + roles granulares bitflags (cashier/pharmacist/admin/owner) |
| Agente-a-agente inter-negocio | Federación `POST /agent/inbox` (ping, catalog.lookup, quote.request, po.create) |
| Monetización del agente | `crates/license` feature-gate (`entitled`/`require` + 402) |

Lo nuevo de Fase 15 es la **capa de orquestación LLM** (objetivo → plan → dispatch) encima de estos cimientos, no los cimientos mismos.

## 4. Implicaciones de diseño — desde HOY

1. **Core vertical-agnostic**: inventario, POS, compras, caja, gastos, reportes, usuarios, backup y auditoría ya son genéricos — mantenerlos libres de acoplamiento pharma. Lo pharma-específico (controlados ISP, recetas, interacciones medicamentosas, convenios isapre) se modulariza como **vertical pack**; un rubro nuevo = un pack nuevo, nunca un fork del core.
2. **API tool-first**: cada endpoint nuevo se diseña asumiendo que su consumidor primario será un agente — schema utoipa estricto y completo, errores estructurados accionables (código + mensaje + remedio), idempotencia donde aplique (patrón `Idempotency-Key` ya existente). Futuro natural: exponer la superficie como **MCP server** además de OpenAPI.
3. **Identidad y auditoría de agentes = primera clase**: toda acción de agente se firma (reusa Ed25519 de `crates/agent`) y aterriza en el mismo audit log inmutable que las acciones humanas, con atribución `agent_id` distinguible de `user_id`. Un negocio operado por agentes debe ser MÁS auditable que uno operado a mano, no menos.
4. **Human-in-the-loop por irreversibilidad**: acciones destructivas o de alto riesgo (anular ventas masivas, cambios de precio catálogo completo, pagos salientes, borrado) requieren confirmación humana explícita. El agente propone, el humano aprueba; lo reversible fluye autónomo.
5. **Capa agéntica = opt-in, NUNCA prerequisito**: los agentes LLM requieren conectividad/cómputo que el core no puede asumir. El invariante offline-first (ADR-0005 #2) se mantiene intacto: el ERP opera 100% sin la capa agéntica; los agentes son un nivel de operación adicional, no una dependencia. Sin internet, el negocio sigue vendiendo.
6. **Naming/posicionamiento**: ~~el rename multi-rubro se decide cuando el segundo vertical sea real~~ — **RESUELTO el mismo día**: la plataforma se llama **RutAgentIA** (§1.5). Los renombres físicos (repo/crates/binarios/MSI) siguen pendientes como tarea aparte con go explícito.

## 5. Qué NO cambia

- Roadmap Fases 9-14 sigue tal cual — esta visión no bloquea el camino freemium/MSI ni el primer cobro.
- Los 7 invariantes de ADR-0005 (core gratis offline, sin lock-in, sin dark patterns, sin kill-switch…) aplican también a la capa agéntica.
- Scope de repo: servidor on-prem genérico acá; Tu Farmacia Coquimbo en su repo; license-server en el suyo.
- Performance budget POS (<50ms p99): los agentes consumen la API, jamás se mete inferencia LLM en el hot path de venta.

## 6. Fasing propuesto — Fase 15 (post Fase 14, no antes de revenue)

- **15a — Tool surface formal**: superficie MCP/OpenAPI curada para agentes (catálogo de tools con descripciones operacionales, no sólo schemas), service accounts de agente con scopes.
- **15b — Orquestador MVP**: 1 agente orquestador (LLM API o local) que recibe un objetivo en lenguaje natural y lo ejecuta contra `/api/v1` con plan visible y confirmaciones human-in-the-loop. Sin jerarquía todavía.
- **15c — Jerarquía coordinador/equipo**: protocolo orquestador→coordinadores→equipo (reusa Envelope Ed25519), dominios separados, presupuestos de acción por agente.
- **15d — Vertical packs**: extraer lo pharma-específico a pack; definir el contrato de pack (migraciones propias, tools propias, compliance propio); segundo vertical piloto.
- **Monetización**: la capa agéntica es candidata natural a tier pago (Business/Enterprise) o microtx — consistente con ADR-0005 (el core manual sigue gratis).

## 7. Riesgos / preguntas abiertas

- **Costo LLM vs on-prem**: ¿inferencia cloud (API key del cliente), local (modelo chico), o híbrido? Tensión directa con offline-first y con el perfil de hardware mínimo (i3/8GB).
- **Responsabilidad regulatoria**: en farmacia, acciones de agente sobre controlados/recetas tienen peso legal (Ley 20.000) — el vertical pack debe poder marcar tools como "nunca autónomo".
- **Confianza del comprador**: el independiente compra "infraestructura competitiva"; vender "agentes que operan tu negocio" requiere madurez de mercado — secuenciar detrás del ERP probado.
- **Scope creep**: la visión es 2027+; el riesgo real de hoy es distraer la ejecución de Fases 9-11 (primer cobro). Esta doc existe para *registrar* el norte, no para empezarlo.

## Referencias

- [`ecosystem-roadmap.md`](./ecosystem-roadmap.md) — federación de agentes inter-negocio (la dimensión *horizontal*; esta doc es la *vertical*: agentes dentro del negocio).
- [`latam-master-plan.md`](./latam-master-plan.md) — tesis AI-native 2026-2035 (esta visión la concreta en arquitectura).
- [`market-thesis.md`](./market-thesis.md) — beachhead farmacia independiente.
- [ADR-0005](../adr/0005-core-gratis-no-locked-in.md) — invariantes que la capa agéntica hereda.

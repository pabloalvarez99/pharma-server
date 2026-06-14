# RutAgentIA — Objetivo del proyecto (norte canónico)

> **Este documento es el objetivo raíz del proyecto.** Reconstruido 2026-06-13
> tras pérdida de documentación local por formateo de SSD (la fuente sobrevive en
> el remoto `origin/feature/erp-parity`). Consolida en lenguaje simple la visión
> y enlaza los documentos profundos que la sustentan:
> - [`agentic-business-platform.md`](./agentic-business-platform.md) — la arquitectura RutAgentIA (Fase 15).
> - [`saas-to-agentic-thesis.md`](./saas-to-agentic-thesis.md) — el *por qué* económico (SaaS→Agentic, etapas, plan auto-financiado).
> - [`ecosystem-roadmap.md`](./ecosystem-roadmap.md) · [`market-thesis.md`](./market-thesis.md) · [`b2b-marketplace.md`](./b2b-marketplace.md) — federación, moat y marketplace.
> - Bitácora: entradas 2026-06-09 (PR #116 Fase 15, PR #117 RutAgentIA + tesis).

---

## 0. Una frase

> **1 RUT = 1 agente IA personal.** pharma-server deja de ser "un ERP de
> farmacias" y pasa a ser la **infraestructura invisible** sobre la que agentes
> de IA operan cualquier negocio chileno. La farmacia es el punto de partida
> (beachhead), no el límite.

---

## 1. Qué es RutAgentIA

Plataforma donde **cada persona o empresa en Chile tiene un agente de IA asociado
a su RUT**. En vez de abrir muchas aplicaciones, el humano le habla a un asistente
que actúa por él: maneja finanzas, ayuda con compras, gestiona el negocio, apoya
en salud.

- **RUT = identidad digital única.** Técnicamente: `RUT ↔ DID ↔ keypair Ed25519`,
  esquema `did:rut:`. Reusa la identidad firmada que ya existe en `crates/agent`
  (`identity.rs`, `envelope.rs`, `canonical.rs`).
- **Regla:** `1 RUT = 1 agente = N "domain-packs"` (finanzas, negocio, salud, …).

---

## 2. La tesis: SaaS → Agentic Company

El SaaS existió porque el software podía almacenar/transmitir/calcular pero **no
podía decidir**. Las pantallas (formularios, dashboards, menús) eran andamiaje
para conectar la inteligencia humana con los datos. Con IA confiable y barata, ese
andamiaje muere; lo que sobrevive es el núcleo que siempre fue el producto real:
**ledger íntegro + auditoría + identidad + rieles (pagos/regulatorios)**.

```
Hoy:      Humano → Software → Datos
Mañana:   Humano → Agente IA → Software → Datos
```

Ejemplo concreto:

```
Antes:  entras al ERP → revisas stock → creas orden de compra (a mano).
Después: le dices al agente "compra lo que esté por agotarse" → lo hace solo.
```

**Cuatro etapas** (cada una se paga sola y deja construido el substrato de la
siguiente — disciplina anti-quiebre):

```
Tool ──▶ Worker ──▶ Team ──▶ Company
2024-26   2026-29    2029-32   2032-35+
```

1. **Tool** — humano opera, IA asiste. (= vender el ERP pharma HOY; financia todo.)
2. **Worker** — un agente es dueño de UNA función punta-a-punta con KPI medible;
   el humano revisa excepciones. Pricing pasa de *per-seat* a *por función/resultado*.
3. **Team** — orquestador + coordinadores cruzan funciones por objetivos
   ("sube el margen 2pp"). (= Fase 15.)
4. **Company** — la organización ES agentes + pocos humanos (capital, juicio,
   responsabilidad legal). El ERP es invisible: es el sistema nervioso.

---

## 3. Qué pasa con pharma-server

| Antes | Ahora |
|---|---|
| ERP para farmacias | Plataforma de agentes para cualquier negocio |

Sirve para farmacias → ferreterías → restaurantes → tiendas online → servicios →
cualquier PYME. **Farmacia = beachhead**, no boundary. El núcleo es
**vertical-agnostic + "vertical packs"**; la API `/api/v1` es **tool-first**.

---

## 4. El primer agente real (no construir un framework)

**Agente de reposición y compras.** Loop cerrado, alto valor, bajo riesgo clínico,
y **todos los rieles ya existen** (FEFO, purchase orders, WAC, federación
quote/PO):

- revisa inventario,
- detecta faltantes / forecast,
- cotiza (federado, contra N droguerías),
- genera órdenes de compra automáticamente,
- el humano aprueba excepciones.

KPI: quiebres de stock evitados, horas ahorradas. Se vende como tier/microtx.
**No** empezar por el agente conversacional general — empezar por el que ahorra
plata medible.

---

## 5. Arquitectura futura

```
Usuario  ──(define objetivos / aprueba irreversibles)
   ↓
Orquestador IA
   ↓
Coordinadores (cruzan funciones)
   ↓
Agentes especializados   (Inventario · Compras · Ventas · Finanzas · …)
   ↓
ERP + APIs (/api/v1 + CLI) + Base de datos
```

Ejemplo:

```
Pablo → Agente RutAgentIA → {Agente Inventario, Compras, Ventas, Finanzas} → pharma-server
```

Mapea a assets existentes: `crates/agent` (Ed25519/DID), OpenAPI (utoipa), audit
log inmutable, federación firmada (`/agent/inbox`, envelopes canónicos).

---

## 6. Objetivo final

Un **"sistema operativo de negocios impulsado por agentes IA"**: el ERP deja de
ser la app principal y pasa a ser la infraestructura invisible que los agentes
usan para trabajar por el usuario. Plataforma agéntica para cualquier empresa de
Chile, anclada en el **RUT como identidad digital**.

La interfaz del futuro: **conversación** (declarar objetivos) + **bandeja de
excepciones** (aprobar irreversibles) + **vistas de confianza** (el audit log como
UI — ver qué hizo el agente y por qué).

---

## 7. Qué NO construir nunca

- Framework genérico de agentes (commodity de los frontier labs — usar Agent SDK/MCP).
- Más dashboards/reportes más allá de excepciones + confianza (es la capa que muere).
- Chatbot-skin sobre el ERP ("pregúntale a tus datos") — ROI cero, posiciona en commodity.
- Modelo propio / fine-tuning prematuro — competir "arriba" es suicidio de capital.
- B2C (agente del paciente) antes de probar el B2B — salud+plata de personas
  naturales = máxima sensibilidad (Ley 19.628), cero revenue hoy.

---

## 8. Invariantes que esta visión NO rompe

- **Offline-first / core gratis / sin lock-in** ([ADR-0005](../adr/0005-core-gratis-no-locked-in.md)):
  el agente corre en TU nodo con TUS datos — es la respuesta de soberanía al
  problema de confianza, no una excepción a la regla. Los agentes son **tier
  pago**; el ledger/core sigue gratis.
- **Human-in-the-loop**: el humano aprueba lo irreversible; los agentes firman y
  quedan auditados (`agent_id` distinguible de `user_id`).
- **No bloquea Fases 9–14**: RutAgentIA/Fase 15 es **post-revenue**. Primero se
  vende y valida el beachhead (ERP pharma, etapa Tool). El plan agéntico es
  **opcionalidad encima de un negocio viable**, no un all-in.

---

## 9. Relación con el trabajo actual

- **Fase 9–11** (MSI vendible, licencia, pagos): etapa **Tool**, financia todo. Sin cambios.
- **App user-test (sesión actual, equipo de agentes)**: endurecer el beachhead =
  un farmacéutico real (un RUT) corre el core gratis de punta a punta. Es
  precondición del plan auto-financiado.
- **Substrato de atribución** (barato, pre-Fase 15): `agent_id` en audit log +
  service accounts con scopes. Sin esto no hay Worker confiable.
- **Fase 15a**: exponer `/api/v1` como tools MCP (descripciones operacionales).
- **Fase 15b**: el primer agente Worker = reposición/compras (§4).

**Rename físico del proyecto a RutAgentIA: PENDIENTE de "go" explícito del fundador.**

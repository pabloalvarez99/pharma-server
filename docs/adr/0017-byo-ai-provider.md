# ADR-0017: BYO‑AI‑Provider — el LLM opt‑in detrás del seam del agente (RutAgent)

- **Status**: Proposed
- **Date**: 2026-06-23
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: producto, north-star, agente, IA, LLM, offline-first, license, web
- **Extiende**: [ADR‑0016 agent‑assist](./0016-agent-assist-architecture.md) ·
  [ADR‑0005 core gratis / no lock‑in](./0005-core-gratis-no-locked-in.md)
- **Plan**: [`rutagent-web-platform-master-plan.md`](../strategy/rutagent-web-platform-master-plan.md)

## Context and Problem Statement

Pivote **RutBusiness → RutAgent** (fundador 2026-06-23): el producto es el **agente
IA del negocio**, y el usuario debe poder **usar un proveedor de IA como servicio**
(estilo *OpenClaw / Hermes*: agente self‑hostable, BYO‑provider). Hoy el agente
(`crates/assist`, ADR‑0016) es **determinístico y offline** — sin LLM. ADR‑0016 ya
previó esto y dejó el seam `AssistProvider` *"para enchufar un LLM opt‑in después,
con la key del propio dueño, default OFF"*.

Pregunta: **¿cómo se provee el LLM** sin romper offline‑first (ADR‑0005), sin lock‑in,
sin costo ongoing en el Free, y de forma provider‑agnostic?

## Decision Drivers

- **Offline‑first sagrado** (ADR‑0005 #1/#2/#6): el core nunca depende de un LLM/red.
- **Privacidad** (Ley 19.628): por default los datos del tenant no salen de la máquina.
- **Costo cero en el Free**: sin tokens nuestros en el tier gratis.
- **Provider‑agnostic**: no casarse con un proveedor (OpenAI/Anthropic/local).
- **Revenue**: habilitar un tier "IA gestionada" sin rehacer la arquitectura.
- **Cero rewrite**: reusar el seam `AssistProvider` ya existente.

## Decision

Implementar el LLM como un **segundo `AssistProvider` (`Llm`) detrás del seam de
ADR‑0016**, configurable por tenant vía `AssistConfig`, **opt‑in y default OFF**, con
**fallback automático** al proveedor `Deterministic`. Tres modos de aprovisionamiento,
mismo seam:

1. **BYO‑key on‑prem (modo base del tier IA, gratis)** — la key del dueño
   (OpenAI/Anthropic/…) vive en el server; la llamada sale de la máquina del tenant.
   Sólo viaja el contexto que el dueño autoriza.
2. **Proxy gestionado (tier de pago, "AI as a service")** — RutAgent expone un
   endpoint que proxea con la **key de la plataforma**, **mide tokens** y lo cobra
   (microtx/tier, gate `license::require`).
3. **Local (Ollama/llama.cpp, opt‑in)** — modelo en la LAN; 100% offline.

**Provider‑agnostic** por un trait `LlmBackend` (request/response normalizados) con
impls por proveedor. **Seguridad**: key cifrada at‑rest, nunca en logs, scope
per‑tenant; cada egreso de contexto al LLM se registra en `audit_log`
(atribución `agent_id`). **Tool‑calling** (Fase 4) reusa el contrato propose/confirm
de ADR‑0016 W3 para escrituras (human‑in‑the‑loop, token de un solo uso).

## Consequences

**Positivas**
- El agente "real" (LLM) se habilita **sin tocar el core** — sólo un impl del seam.
- Offline‑first intacto: `Deterministic` sigue siendo default y red de seguridad.
- Dos rieles de negocio: BYO (gratis, su costo) + gestionado (revenue).
- Base para la web companion (mismo `/api/v1/assist`) y para `did:rut:`/federación.

**Negativas / riesgos**
- Mandar contexto a un tercero = superficie de privacidad → opt‑in duro + minimización
  + auditoría obligatorios.
- El proxy gestionado introduce costo/infra y metering (mitigado: es tier de pago).
- Exponer el on‑prem (web/túnel) exige hardening previo (ver master plan §6:
  JWT secret real, rate‑limit, `/api/v1/setup` bloqueado en nodos públicos).

## Alternatives considered

- **LLM siempre‑on en el hot path** — ❌ viola offline‑first, costo/latencia, fuga de datos.
- **Sólo proxy gestionado (sin BYO)** — ❌ lock‑in + costo en Free; contra ADR‑0005.
- **Sólo local (Ollama)** — ✅ privado pero exige hardware; queda como modo, no único.
- **Rehacer `crates/assist`** — ❌ innecesario; el seam de ADR‑0016 ya lo soporta.

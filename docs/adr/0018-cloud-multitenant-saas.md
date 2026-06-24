# ADR-0018: Tier cloud multi-tenant (web zero-install) + signup por RUT

- **Status**: Proposed
- **Date**: 2026-06-24
- **Deciders**: pabloalvarez99 (fundador)
- **Tags**: distribución, cloud, multi-tenant, RUT, onboarding, offline-first, seguridad
- **Extiende**: [ADR‑0005 core gratis / no lock‑in](./0005-core-gratis-no-locked-in.md) ·
  [ADR‑0012 web‑onprem‑interop](./0012-web-onprem-interop.md) ·
  [ADR‑0015 universal client](./0015-universal-cross-platform-client.md) ·
  [ADR‑0017 BYO‑AI](./0017-byo-ai-provider.md)
- **Plan**: [`distribution-todo-chile.md`](../strategy/distribution-todo-chile.md)

## Context and Problem Statement

El norte es **1 RUT = 1 negocio/persona = 1 agente IA**, accesible a **todo chileno**.
El producto hoy es **on-prem single-tenant** (MSI Windows + SurrealKv embebido, datos
locales, offline-first). Eso llega a un nicho (Windows, instalar). Para masas se
necesita **web zero-install: un link, móvil-first**, lo que implica un **tier cloud
hospedado multi-tenant** — en tensión directa con "datos siempre en la farmacia"
(ADR‑0005).

Pregunta: ¿cómo ofrecer acceso cloud masivo **sin** romper offline-first ni la tesis de
soberanía, y **sin** rehacer el server?

## Decision Drivers

- **Alcance masivo** real (móvil, sin instalar) — el MSI no escala a "todo chileno".
- **Offline-first sagrado** (ADR‑0005 inv. 1/2/6): el core gratis offline no se toca.
- **No rehacer**: reusar el server Rust multi-tenant tal cual.
- **Costo casi plano** + IA self-funding (ADR‑0017).
- **Seguridad** de un servicio público multi-tenant (aislación, abuso, PII Ley 19.628).

## Decision

Ofrecer un **tier cloud multi-tenant opt-in**, manteniendo el MSI on-prem como opción
de soberanía. Concretamente:

1. **Front en Vercel** (dominio propio `rutagent.cl`) — SPA estático (el `/app`
   existente) o Next; CDN, HTTPS. Es la cara de masas.
2. **Backend = el mismo server Rust** como **nodo cloud multi-tenant** en host
   always-on (**Fly.io** / VPS) detrás de **Cloudflare** (DNS+WAF), en `api.rutagent.cl`,
   con **CORS** al origen del front. **1 DB SurrealKv** en volumen persistente hasta que
   la escala obligue a shardear. La multi-tenencia ya existe (`tenant_id` JWT + `WHERE`).
3. **Signup público por RUT** — nuevo `POST /api/v1/signup` que crea tenant+dueño keyed
   por RUT (validación módulo 11, 1 RUT = 1 tenant), con **anti-abuso** (Cloudflare
   Turnstile + rate-limit por IP + verificación de email). Reemplaza, **sólo en cloud**,
   al `/api/v1/setup` one-shot install-wide (que sigue siendo correcto on-prem).
4. **IA** = BYO-key gratis + proxy gestionado medido (ADR‑0017): el LLM es el único
   costo que escala y se cubre con la key del dueño o se cobra.

**Invariante de coexistencia**: el tier cloud es **opt-in**. Quien quiere soberanía usa
el MSI offline (datos locales, sin nube). El core gratis offline nunca se rompe ni se le
quita capacidad (ADR‑0005). El tier cloud **declara explícito** que guarda datos en la
nube (consentimiento + export/borrado, Ley 19.628).

## Consequences

**Positivas**
- Alcance real a todo Chile con un link móvil, sin instalar.
- Reusa el server (cero rewrite); la multi-tenencia ya estaba.
- Infra casi plana; IA self-funding (revenue, no pérdida).
- Base para `did:rut:` y la federación B2B (Fase 13).

**Negativas / riesgos**
- Datos del tenant viven en la nube (tensión con la tesis soberanía) → mitigado por ser
  **opt-in** + MSI offline siempre disponible.
- Superficie pública multi-tenant: aislación per-tenant debe ser airtight (ya
  verificada), + signup anti-abuso + WAF + JWT secret real + rate-limit.
- 1 DB compartida = punto único hasta shardear; backups del volumen obligatorios.
- PII en la nube → cumplimiento Ley 19.628 (política, consentimiento, export/borrado).

## Alternatives considered

- **Sólo MSI on-prem** — ❌ no llega a masas (Windows + instalar).
- **Container por tenant** (aislación total) — ✅ aísla, pero ops/costo por tenant alto;
  diferido a cuando la escala lo pida (shard).
- **Reescribir backend cloud-native** — ❌ rompe la tesis "no rehacer" + Rust ya sirve.
- **Cloudflare Workers para el backend** — ❌ serverless sin proceso largo ni disco; no
  corre el Rust + SurrealKv.
- **Mantener `/setup` one-shot en cloud** — ❌ inservible multi-tenant (DB nunca vacía);
  de ahí el `/signup` por RUT.

# RutAgent — Web Platform + BYO‑AI‑Provider Master Plan

> **Directiva fundador (2026-06-23)**: evolucionar RutBusiness → **RutAgent**,
> donde el usuario **usa un proveedor de IA como servicio** (BYO‑provider, estilo
> *OpenClaw / Hermes agent*), **desplegar una web** y **fusionarla con el dominio
> propio** vía el proyecto `tu-farmacia.cl`
> (`D:\Respaldo Proyectos\GitHub\build-and-deploy-webdev-asap`). Este documento es
> el ULTRA‑PLAN + el checklist de pasos **manuales** del fundador.

Relacionado / extiende: [`agentic-business-platform.md`](./agentic-business-platform.md) ·
[`saas-to-agentic-thesis.md`](./saas-to-agentic-thesis.md) ·
[ADR‑0016 agent‑assist](../adr/0016-agent-assist-architecture.md) ·
[ADR‑0017 BYO‑AI‑provider](../adr/0017-byo-ai-provider.md) ·
[ADR‑0012 web‑onprem‑interop](../adr/0012-web-onprem-interop.md) ·
[ADR‑0014 DSS storefront](../adr/0014-dss-storefront-integration.md) ·
[ADR‑0015 universal client](../adr/0015-universal-cross-platform-client.md) ·
[ADR‑0005 core gratis / no lock‑in](../adr/0005-core-gratis-no-locked-in.md).

---

## 0. TL;DR

- **RutAgent** = la cara agéntica de RutBusiness. El ERP (pharma‑server) deja de ser
  el producto y pasa a ser **el conjunto de herramientas (tools) que opera un
  agente IA**. 1 RUT = 1 negocio = 1 agente. (El norte ya existía — ver
  `agentic-business-platform.md`; esto lo concreta y le pone fecha.)
- **El agente ya tiene el seam para esto.** ADR‑0016 construyó `crates/assist` con
  un trait `AssistProvider` cuyo único objetivo declarado es *"enchufar un LLM
  opt‑in después, con la key del propio dueño, default OFF"*. Hoy sólo existe el
  proveedor `Deterministic` (offline, sin red). **BYO‑AI‑provider = implementar un
  segundo proveedor detrás de ese seam.** No es un rewrite.
- **No se rompe offline‑first.** El proveedor LLM es **opt‑in, default OFF**; si no
  hay key/red, el agente cae al proveedor `Deterministic`. El core ERP nunca
  depende de la IA (invariantes ADR‑0005 #1/#2/#6).
- **Web** = un *cloud companion* (Next.js en Vercel, cuenta del fundador) que habla
  con el server on‑prem por el **seam HTTP existente** (`/api/v1`) a través de un
  **Cloudflare Tunnel**. La UI de escritorio (Tauri) sigue; la web es un terminal
  más (ADR‑0015).
- **Fusión con `tu-farmacia.cl`** = a nivel **plataforma** (dominio + cuenta Vercel
  + lenguaje visual + el seam web↔on‑prem), **NO** a nivel código. Ambos repos
  prohíben cross‑import (CLAUDE.md de los dos). `tu-farmacia.cl` queda como
  **tenant flagship + plantilla storefront** (ADR‑0014), bajo un **subdominio** y un
  **proyecto Vercel separado** para no tocar la producción real.
- **Modelo de IA‑as‑a‑service** = **BYO‑key gratis** (la key del dueño, su costo) +
  **proxy gestionado de pago** (RutAgent pone la key, mete metering, lo cobra como
  microtx/tier — freemium, [`freemium-master-plan.md`](./freemium-master-plan.md)).

---

## 1. Qué cambia y qué NO

| | Antes (RutBusiness) | Después (RutAgent) |
|---|---|---|
| Producto | ERP on‑prem vendible (MSI) | **Agente IA** del negocio; el ERP es su infraestructura/tools |
| Interfaz primaria | POS/Admin (clics) | **Conversación** ("háblale a tu negocio") + POS como tool |
| IA | Agente determinístico es‑CL (sin LLM) | **+ Proveedor LLM opt‑in** (BYO‑key) detrás del seam |
| Superficie | Escritorio (Tauri) + CLI | **+ Web** (cloud companion, Vercel) |
| Distribución | MSI local | MSI local **+** web bajo dominio propio |
| Negocio | Freemium MSI + tiers + microtx | **+ AI‑as‑a‑service** (proxy gestionado medido) |

**Invariantes que NO cambian** (ADR‑0005, no negociables):
1. Core ERP **siempre gratis y offline**. La IA/web son capas **opt‑in** que se
   *agregan*; nunca se le quita capacidad al Free ni se vuelve cloud‑dependiente.
2. Por default los datos del tenant **no salen de la máquina**. Mandar contexto a un
   LLM es una acción **explícita y opt‑in** del dueño (estilo telemetría, ADR‑0005 #3).
3. Sin kill‑switch remoto: aunque caiga la web/cloud, el MSI local sigue operando.
4. `pharma-server` y `build-and-deploy-webdev-asap` siguen **separados en código**.

---

## 2. Arquitectura — ¿dónde corre el LLM?

Tres planos, todos detrás del **mismo seam** `AssistProvider` (ADR‑0016). El plano
se elige por `AssistConfig` (per‑tenant, default = `Deterministic`).

```
Usuario ──pregunta──▶ Agente (RutAgent)
                         │
              ┌──────────┴───────────┐
              ▼                      ▼
   AssistProvider::Deterministic   AssistProvider::Llm   (opt-in, default OFF)
   (es-CL intents, offline,        │
    cero red, fallback)            ├─▶ A) on-prem BYO-key  → provider del dueño
                                   ├─▶ B) cloud proxy      → key de RutAgent (pago, metered)
                                   └─▶ C) local (Ollama)   → modelo en la LAN
                         │
            tools = /api/v1 (read-only hoy; write con human-in-the-loop, ADR-0016 W3)
```

- **A) On‑prem BYO‑key (default del modo IA)**: el dueño guarda su key
  (OpenAI/Anthropic/…) en el server. `crates/assist` hace la llamada. Sólo viaja el
  prompt+contexto que el dueño autoriza. Offline‑first intacto (fallback determinístico).
- **B) Cloud proxy gestionado (tier de pago)**: RutAgent expone un endpoint
  (Vercel Edge/Worker) que proxea al provider con la **key de la plataforma**,
  **mide tokens** y lo cobra (microtx/tier). Es el "IA como servicio". Requiere que
  el contexto del negocio llegue a la nube → **opt‑in duro + minimización de datos**.
- **C) Local (Ollama/llama.cpp)**: para quien quiere IA sin mandar nada afuera;
  modelo en la LAN. 100% offline. (Fase posterior.)

**Tool‑calling (Fase 4)**: el LLM no "alucina" sobre los datos — recibe el catálogo
de tools `/api/v1` (function‑calling / MCP) y **ejecuta** consultas reales; las
escrituras pasan por el contrato propose/confirm que ADR‑0016 (assist W3) ya definió
(token de un solo uso, tenant‑bound, admin‑gated). Esto materializa el modelo
`Humano → Agente IA → Software → Datos` del norte.

---

## 3. Web companion (cloud) + fusión con `tu-farmacia.cl`

### 3.1 Qué es la web
Un **cloud companion** (Fase 14 del roadmap, ADR‑0015): app Next.js en Vercel que da
(a) el **chat del agente**, (b) un **dashboard** liviano, (c) la **config del
proveedor IA**. Habla con el server on‑prem del tenant por `/api/v1` vía
**Cloudflare Tunnel** (el puente que el fundador ya eligió). La web **no** reemplaza
al server: es un terminal en red (la verdad y los datos siguen on‑prem).

> Nota: el cliente Tauri actual usa `invoke()` (comandos nativos), **no** corre en
> navegador. La web es un **front nuevo** que habla HTTP directo a `/api/v1` (mismo
> contrato), reusando el **lenguaje visual** del cliente (`client/src/views/ui.ts`,
> `brand.css`) y, donde aplique, las **plantillas storefront** de `tu-farmacia.cl`.

### 3.2 Cómo se "fusiona" con tu-farmacia.cl (sin romper producción)
`tu-farmacia.cl` es la **farmacia real en producción** (Next.js 14, Cloud SQL,
Webpay PROD, Vercel proyecto `tu-farmacia`). Reglas:

- **NO** se mezcla el código ni el deploy (ambos CLAUDE.md lo prohíben; romper la
  prod del cliente real es inaceptable).
- La fusión es **a nivel plataforma**:
  - **Dominio**: RutAgent web vive en un **subdominio** (recomendado:
    `app.rutagent.cl` si se compra el dominio del producto, o `agent.tu-farmacia.cl`
    como showcase) → **proyecto Vercel separado**, mismo dueño/cuenta.
  - **`tu-farmacia.cl` = tenant flagship + plantilla**: es el caso real que prueba el
    storefront por‑tenant (ADR‑0014) y la referencia de diseño/taxonomía (ya es la
    fuente del catálogo de rubros DSS).
  - **Seam, no import**: si RutAgent necesita datos de la farmacia real, los pide por
    HTTP (la prod ya expone `/api/admin/*`), nunca importando su código.
- **Resultado**: un dominio/identidad común y una cuenta Vercel común, dos productos
  que conversan por contrato HTTP. (Si el fundador luego quiere unificar de verdad,
  eso es una decisión aparte con su propio ADR — **no** asumida aquí.)

---

## 4. Roadmap por fases

> Cada fase cierra su loop (GATE → merge → deploy/parked) por las reglas del repo.
> Las fases web/IA son **opt‑in** y nunca bloquean el MSL local.

- **Fase 0 — Puente (AHORA, días)**: Cloudflare Tunnel del server on‑prem +
  **endurecer** (ver §6). Da la URL pública estable que todo lo demás consume.
  *(Es la opción "túnel demo" que el fundador eligió, elevada a fundamento.)*
- **Fase 1 — BYO‑LLM provider (semana)**: implementar `AssistProvider::Llm` en
  `crates/assist` (OpenAI/Anthropic via HTTP), `AssistConfig` per‑tenant (key
  cifrada, default OFF), fallback a `Deterministic`. Endpoint `/api/v1/assist`
  ya existe → sólo cambia el provider. ADR‑0017.
- **Fase 2 — Web companion (semanas)**: Next.js en Vercel (cuenta del fundador,
  subdominio), chat + dashboard + config IA, habla `/api/v1` por el túnel. Reusa UI.
- **Fase 3 — IA‑as‑a‑service (semanas)**: proxy gestionado (Edge) + metering de
  tokens + billing (microtx/tier, reusa `crates/license` + el rail de pagos de
  Fase 11). El revenue de la IA.
- **Fase 4 — Agente con tools (semanas)**: function‑calling / MCP sobre `/api/v1`;
  escrituras con propose/confirm (ADR‑0016 W3). El agente *actúa*, no sólo responde.
- **Fase 5 — Cloud multi‑tenant / federación (meses)**: varios negocios, identidad
  `did:rut:`, el marketplace B2B (Fase 13). El destino de
  `agentic-business-platform.md`.

---

## 5. Modelo de negocio de la IA

- **Free**: BYO‑key. El dueño pone su provider y paga su costo directo. RutAgent no
  cobra por la IA, sólo la habilita. (Honra "core gratis"; cero costo ongoing nuestro.)
- **Pago (tier/microtx)**: proxy gestionado "AI as a service" — RutAgent pone la
  key, mete cuota/metering y lo cobra. Catálogo: `ai.assist` (chat), `ai.actions`
  (tool‑calling), `ai.local` (pack Ollama). Encaja en
  [`freemium-master-plan.md`](./freemium-master-plan.md) + gate `license::require`.

---

## 6. Seguridad e invariantes (CRÍTICO)

- **Exponer el on‑prem al público (túnel) exige endurecer antes**:
  - `PHARMA__JWT__SECRET` **real** (hoy default `change-me-in-production`).
  - **Rate‑limit** activado (el state ya existe en `AppState.rate_limit`).
  - El endpoint **`POST /api/v1/setup` es NO autenticado y crea el primer dueño si
    la DB está vacía** → en un nodo expuesto, un fresh server puede ser **secuestrado**.
    Mitigación: completar el first‑run **antes** de exponer, o gatear `/setup` por
    red/flag en nodos públicos.
  - Túnel con **Cloudflare Access** (auth delante) para nodos sensibles.
- **BYO‑key**: cifrada at‑rest, nunca en logs, nunca commiteada, scope per‑tenant.
- **Datos al LLM**: opt‑in explícito + minimización (mandar sólo lo necesario);
  registrar en `audit_log` cada vez que sale contexto (atribución `agent_id`).
- **Offline‑first**: `Deterministic` es el default y el fallback; sin red/key el
  agente sigue respondiendo; el core ERP nunca llama al LLM en el hot path del POS.

---

## 7. Pasos MANUALES del fundador (lo que sólo tú puedes hacer)

Yo (el agente) hago el código, los builds, los docs, los deploys CLI. **Tú** haces lo
que requiere tus cuentas/credenciales/decisiones. Checklist:

### 7.1 Cuentas / credenciales
1. **Vercel** — autorizar deploy (elige UNA, **no pegues el token en el chat**):
   - **Opción recomendada**: en la terminal de esta sesión escribe
     `! vercel login` y completa el login en tu navegador. El agente usa esa sesión.
   - **O** crea un token en `https://vercel.com/account/tokens`, guárdalo en
     `~/.rutagent/secrets.env` (o variable de entorno `VERCEL_TOKEN`) que está
     **gitignored**; el agente lo lee en el deploy, **nunca** lo escribe en repo/chat.
   - **Decisión**: crear un **proyecto Vercel NUEVO** para RutAgent web (NO el
     proyecto `tu-farmacia` de producción).
2. **Cloudflare** — para el túnel con URL estable:
   - `! cloudflared tunnel login` (abre el navegador, eliges la zona DNS).
   - (cloudflared 2026.5.2 ya está instalado.)
3. **Proveedor de IA** — para probar el BYO‑LLM:
   - Saca una API key (OpenAI o Anthropic) y ponla por
     `! pharma config set assist.provider.key` (cuando exista, Fase 1) **o** en el
     `secrets.env` gitignored. No la pegues en el chat.
   - Decide el provider del **tier gestionado** (Fase 3) y su billing.

### 7.2 Dominio / DNS
4. **Elige el subdominio** de la web RutAgent: `app.rutagent.cl` (si compras
   `rutagent.cl`) **o** `agent.tu-farmacia.cl` (showcase con el dominio actual).
5. **DNS**: en tu registrador / Cloudflare DNS, apunta:
   - el subdominio web → Vercel (CNAME que Vercel te da al añadir el dominio), y
   - el subdominio del túnel (ej. `api.tu-farmacia.cl`) → el Cloudflare Tunnel.
6. Si usas `rutagent.cl`: **comprar el dominio** (registrar).

### 7.3 Hardening on‑prem (antes de exponer al público)
7. Fija `PHARMA__JWT__SECRET` a un secreto real en el server que vas a exponer.
8. Completa el **first‑run** (crear el dueño) **antes** de abrir el túnel, o confirma
   que `/api/v1/setup` quede bloqueado en ese nodo.

### 7.4 Decisiones de producto (responde y lo documento)
9. **Naming**: ¿renombrar RutBusiness → **RutAgent** en repo/binarios/MSI ahora, o
   sólo como marca/UI por ahora? (El rename físico es tarea aparte, necesita tu go —
   CLAUDE.md.)
10. **Énfasis IA**: ¿BYO‑key primero (gratis, tu costo) y proxy gestionado después,
    o ir directo al proxy de pago?
11. **Web**: ¿subdominio del dominio actual (`tu-farmacia.cl`) o dominio nuevo
    (`rutagent.cl`)?

---

## 8. Sobre "darle mi token de Vercel al agente" (seguridad)

**No pegues el token en el chat ni lo commitees** — quedaría en el historial/logs y
sería una fuga. Formas seguras de autorizar el deploy:
- `vercel login` interactivo (preferido) — el agente usa la sesión de tu CLI.
- Token en `VERCEL_TOKEN` / archivo `secrets.env` **gitignored**; el agente lo
  consume en runtime y nunca lo imprime.
Mismo criterio para las keys de IA y de Cloudflare. (Regla #8 del repo: secrets nunca
al repo.)

---

## 9. Estado de arranque (lo que YA habilita esto)

- `crates/assist` + `AssistProvider` seam + `/api/v1/assist` → **listo** para el
  provider LLM (ADR‑0016).
- `/api/v1` completo (catálogo, ventas, inventario, caja, usuarios, DTE…) → **el
  tool surface** del agente ya existe.
- Cliente Tauri + `ui.ts`/`brand.css` → **lenguaje visual** reusable para la web.
- `cloudflared` + `wrangler` instalados → **puente y deploy** listos.
- `tu-farmacia.cl` en Vercel (proyecto `tu-farmacia`) → **infra/dominio** del fundador.

> Primer paso ejecutable sin más decisiones: **Fase 0 (túnel + hardening)** + **Fase 1
> (provider LLM detrás del seam)**. Ambos son código/CLI que el agente hace; sólo
> necesitan del fundador los pasos manuales §7.1–7.3.

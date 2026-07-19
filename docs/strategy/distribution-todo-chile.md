# RutAgent — Distribución "accesible a todo chileno"

> **Directiva fundador (2026-06-24)**: "pensando mejor forma de hacer accesible a
> todo chileno" + desplegar web en **Vercel con dominio propio**. Este es el
> ultra-plan de **distribución/acceso masivo**. Extiende
> [`rutagent-web-platform-master-plan.md`](./rutagent-web-platform-master-plan.md)
> (arquitectura general) y se ancla en [ADR‑0018](../adr/0018-cloud-multitenant-saas.md)
> (tier cloud multi-tenant + signup por RUT).

---

## 0. TL;DR — la decisión

Para llegar a **todo chileno** el camino de masas **no es el MSI** (Windows + comodidad
técnica = nicho) — es **web zero-install: un link, móvil-first**. Resolución:

1. **Front en Vercel + dominio propio `rutagent.cl`** — un link que cualquiera abre
   en el celular, CDN global, HTTPS gratis. Es la cara de masas.
2. **Backend = el MISMO server Rust** corriendo como **nodo cloud multi-tenant** en un
   host always-on barato (**Fly.io** recomendado / VPS), detrás de **Cloudflare**
   (DNS + WAF). `rutagent.cl` → Vercel (front); `api.rutagent.cl` → server Rust. CORS
   al origen de Vercel (tower-http ya lo soporta).
3. **Identidad = RUT** (1 RUT = 1 negocio/persona = 1 agente). Onboarding = **signup
   público por RUT**, NO el `/api/v1/setup` one-shot (eso es on-prem). Nuevo endpoint.
4. **IA = BYO-key gratis + proxy gestionado medido** (ADR‑0017): el costo que escala
   es el LLM; BYO lo paga el dueño, el managed es revenue. Infra casi plana.
5. **El MSI no muere** — queda como **opción soberanía/offline** (ADR‑0005): datos en
   tu máquina, sin nube. El tier cloud es **opt-in** (eliges conveniencia sobre
   soberanía). El core gratis nunca se rompe.

**Tensión central resuelta**: la tesis decía "datos siempre en la farmacia,
offline-first". El tier cloud **guarda datos del tenant en la nube** — esto es un
cambio consciente, pero **opt-in**: el que quiere soberanía usa el MSI offline; el que
quiere "un link y listo" usa el cloud. Ambos coexisten. ADR‑0005 se honra porque el
core gratis offline sigue intacto y nada se le quita.

---

## 1. Por qué web zero-install (no MSI) para masas

| | MSI on-prem | Web cloud |
|---|---|---|
| Alcance | Windows + instalar = nicho | **Cualquier celu/PC, un link** |
| Fricción | descargar, SmartScreen, instalar, configurar red | **abrir URL** |
| Soberanía datos | total (local) | datos en la nube (opt-in) |
| Para | quien quiere todo local, sin internet | **el chileno promedio, móvil** |

El gancho NO es el ERP — es **el agente** ("háblale a tu negocio"). Eso entra por una
conversación en el celular, no por instalar software. → la web es el vehículo.

---

## 2. Arquitectura

```
   Celular / PC de cualquier chileno
            │  https://rutagent.cl
            ▼
   ┌───────────────────────┐         ┌──────────────────────────────┐
   │ Vercel (front)        │  HTTPS  │ Cloudflare (DNS + WAF + cache)│
   │ rutagent.cl           │ ───────▶│ api.rutagent.cl               │
   │ SPA/Next (el /app)    │  CORS   └───────────────┬──────────────┘
   └───────────────────────┘                         ▼
                                        ┌──────────────────────────────┐
                                        │ Server Rust (Fly.io / VPS)    │
                                        │ multi-tenant · 1 DB SurrealKv │
                                        │ en volumen persistente        │
                                        │  ┌──────────────────────────┐ │
                                        │  │ /api/v1 (tools) + assist │ │
                                        │  └──────────────────────────┘ │
                                        └───────────────┬──────────────┘
                                                        ▼
                                      LLM (BYO-key del dueño / proxy gestionado)
```

- **Front**: el SPA `/app` que ya existe (servido hoy por el server) se **lifta a
  Vercel** como estático, apuntando a `api.rutagent.cl`. Custom domain + CDN. (O un
  Next.js si se quiere SSR/landing; el SPA actual basta para MVP.)
- **Back**: el server Rust **as-is**, en un container. Ya es **multi-tenant**
  (`tenant_id` en el JWT + `WHERE tenant=` en cada read — verificado en la lane de
  usuarios). 1 sola DB SurrealKv hasta que la escala obligue a shardear.
- **CORS**: `tower-http` cors layer permitiendo el origen `https://rutagent.cl`.
- **Perf**: SurrealKv embebido = sin red en hot path (<50ms p99). Un nodo aguanta
  miles de tenants antes de pensar en shard.

---

## 3. El cambio clave: signup público por RUT (cloud) ≠ setup one-shot (on-prem)

Hoy `POST /api/v1/setup` es **fail-closed install-wide**: crea UNA cuenta cuando la DB
está vacía. Correcto para on-prem (1 MSI = 1 negocio). **Inservible en un cloud
multi-tenant compartido** (la DB nunca está "vacía").

El tier cloud necesita un **endpoint de signup público nuevo**:
`POST /api/v1/signup` → crea **tenant + dueño** keyed por **RUT** (cualquiera puede
registrar SU negocio). Requisitos (ADR‑0018):
- **Validación RUT** (módulo 11) + unicidad: 1 RUT = 1 tenant.
- **Anti-abuso**: rate-limit por IP, **Cloudflare Turnstile** (captcha), verificación
  de email antes de activar.
- Precedente en el repo: ya existen endpoints públicos opt-in (`public_orders` con
  HMAC, sin JWT) → el patrón de exposición pública controlada existe.

On-prem conserva `/setup` one-shot; cloud usa `/signup`. Ambos detrás del mismo
`select_provider`/gates.

---

## 4. Costo a escala (todo chileno)

| Pieza | Costo | Nota |
|---|---|---|
| Vercel (front) | **$0** (Hobby) | tráfico estático enorme gratis; dominio propio incluido |
| Cloudflare (DNS+WAF) | **$0** | free tier cubre DNS, WAF básico, Turnstile |
| Server Rust (Fly.io) | **~$0–10/mes** | free allowance / 1 VM chica; un nodo = miles de tenants |
| Volumen DB | ~$1–3/mes | SurrealKv = 1 archivo; snapshot = backup |
| **LLM** | **variable** | **el único costo que escala** |

**La clave del costo**: la infra es casi **plana** (un nodo barato sirve a muchísimos).
Lo que escala con uso es el **LLM** → por eso BYO-key (el dueño paga su IA) es el
default, y el **proxy gestionado medido** es **revenue, no pérdida**. El producto se
auto-financia: cero IA en el tier gratis, IA pagada se cobra.

---

## 5. Seguridad (multi-tenant público — CRÍTICO)

Exponer un server a todo Chile sube la vara:
- **JWT secret real** (no el placeholder), rotado; **rate-limit** activado (el state ya
  existe en `AppState.rate_limit`).
- **Aislación per-tenant** airtight — ya verificada (tenant del JWT, `WHERE tenant=`,
  cross-tenant → 404). Auditar de nuevo antes de abrir al público.
- **Signup anti-abuso**: Turnstile + email-verify + rate por IP (§3).
- **Cloudflare WAF + Access** delante de `/docs`, `/metrics`, rutas admin.
- **Backups**: snapshot del volumen SurrealKv programado (el job de backup ya existe).
- **Ley 19.628 (datos personales)**: el tier cloud guarda PII de negocios y clientes →
  política de privacidad, consentimiento, derecho a export/borrado (export CSV ya es
  invariante ADR‑0005 #4). El opt-in cloud lo declara explícito.

---

## 6. Fases (de "link hoy" a "todo Chile")

- **F0 — Link hoy (horas, $0)**: liftar el SPA `/app` a **Vercel** (proyecto nuevo,
  cuenta del fundador `timadapa-6315`), apuntando al **túnel actual** como API. Da un
  link `*.vercel.app` real, prueba el flujo end-to-end. **No compra nada.** ← arranque.
- **F1 — Dominio propio (días)**: comprar `rutagent.cl` (NIC Chile), zona en Cloudflare,
  custom domain en Vercel, server Rust en **Fly.io** con volumen, `api.rutagent.cl` por
  Cloudflare, CORS. **URL estable en tu dominio.**
- **F2 — Multi-tenant público (días)**: `POST /api/v1/signup` por RUT + anti-abuso
  (ADR‑0018). Cualquier chileno registra su negocio. Hardening §5.
- **F3 — IA gestionada + onboarding (semanas)**: proxy LLM medido (revenue) + landing
  + onboarding "elige tu rubro" pulido + el agente como primer contacto.
- **F4 — Escala (meses)**: shard por lote/región, multi-región, federación B2B
  (Fase 13), `did:rut:` (el norte agéntico).

---

## 7. Pasos MANUALES del fundador (lo que sólo tú haces)

1. **Comprar `rutagent.cl`** — los `.cl` se registran en **NIC Chile** (nic.cl),
   ~$9.990 CLP/año (Cloudflare Registrar NO vende `.cl`). Cloudflare igual gestiona la
   zona: cambias los **nameservers** en NIC a los que te dé Cloudflare.
2. **Vercel** — yo hago el deploy con tu sesión (`vercel` CLI, ya logueado). Tú
   confirmas agregar el dominio al proyecto nuevo (Vercel te da el record DNS).
3. **Cloudflare** — agregar `rutagent.cl` como **zona raíz** + nameservers (NIC).
4. **Host backend** — crear cuenta **Fly.io** (`! fly auth login`) — o decidir VPS.
5. **Crédito Anthropic** (~$5) para el tier IA gestionado / pruebas.
6. **Decisión de tesis** — confirmar: tier cloud guarda datos en la nube (conveniencia)
   + MSI offline sigue para soberanía. ¿OK?

---

## 8. Decisiones que te tocan (responde y ejecuto)

1. **Host backend**: **Fly.io** (recomendado — free-ish, escala, volumen persistente) ·
   VPS propio · ~~Cloudflare Workers~~ (NO sirve: el Rust + SurrealKv necesitan proceso
   largo + disco).
2. **Dominio**: **`rutagent.cl`** (recomendado — nuevo, aislado, no toca la prod de
   tu-farmacia) · subdominio `agent.tu-farmacia.cl`.
3. **Arranque**: ¿hago **F0 ya** (SPA a Vercel apuntando al túnel) para que tengas link
   en tu Vercel hoy, sin comprar nada?

> Primer paso ejecutable sin decisiones ni compras: **F0**. Todo lo demás necesita la
> compra del dominio + la cuenta del host.

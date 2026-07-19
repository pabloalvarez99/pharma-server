# Product Improvement Master Plan — "Mejorar el programa" (RutBusiness elevation)

> **Directiva fundador (2026-06-19):** publicar todavía NO importa — **mejorar el
> PRODUCTO**. Saturación full. Este es el ultra-plan que paxoloop corre contra el
> equipo. Compañero de [`rubro-select-experience.md`](./rubro-select-experience.md)
> (misma vara de craft) enmarcado por [`rutagentia-vision.md`](./rutagentia-vision.md)
> (north star). Doc dueño: paxoloop (orquesta + integra).

---

## 0. Tesis — qué significa "mejor producto" acá

RutBusiness es un ERP **feature-complete**. NO es todavía el producto que describe el
north star: *"1 RUT = 1 negocio = 1 agente IA; el ERP se vuelve infraestructura
invisible detrás del agente."* Tres gaps separan "buen ERP" de "RutBusiness el producto":

1. **No hay agente.** El dueño no le puede *hablar* a su negocio. El diferenciador
   entero del producto está ausente (hoy "agente" = envelopes Ed25519 B2B
   máquina-a-máquina, NO un asistente con el que el dueño conversa).
2. **Multi-rubro es una promesa.** La vitrina vende 8 rubros; solo 2 (farmacia,
   minimarket) son reales (seed pack + features nativas). Los otros 6 = "Próximamente".
3. **Solo la vitrina es "producido".** El resto de la app — POS, dashboard, reports —
   todavía se lee "de dev".

**"Mejorar el programa" = cerrar esos tres gaps.** Cinco vectores de elevación, una
lane disjunta por vector. No es QA ni hardening (eso está agotado): es subir el techo
del PRODUCTO. No reescribir: el modelo de datos, el gating y los hot paths ya son
correctos y testeados — esto eleva experiencia, profundidad y diferenciación encima.

---

## 1. Los cinco vectores

### V1 — EL AGENTE (north-star differentiator) · owner: **milton** (backend)

La tesis entera del producto. Hoy ausente. **La apuesta de más alto valor estratégico.**

MVP = **"Pregúntale a tu negocio"**: una superficie de consulta/comando en lenguaje
natural sobre las APIs de lectura existentes. El dueño escribe *"¿cuánto vendí hoy?"*,
*"¿qué se está por vencer?"*, *"stock de paracetamol"*, *"¿cuánto hay en caja?"* y recibe
una respuesta real, fundada en SUS datos.

- **Offline-first SAGRADO ([ADR-0005](../adr/0005-core-gratis-no-locked-in.md)):** el MVP
  es un **parser DETERMINÍSTICO** es-CL (keywords/patrones → intent → query existente).
  Cero dependencias, 100% offline, cero costo, cero PII saliendo. ~8–12 intents que
  cubren las preguntas reales del dueño.
- **Seam para LLM opt-in:** trait `AssistProvider` (intent→respuesta) con impl
  determinística por defecto, de modo que un proveedor LLM opt-in (key del propio dueño,
  default OFF, estilo telemetría opt-in §Ley 19.628) se enchufe **después** sin reescribir.
  NO construir el LLM ahora — construir el seam + el core determinístico.
- **Entregables:** crate nuevo `crates/assist` (intent parser + provider trait +
  ejecutores que llaman a los repos de lectura existentes) · `crates/api/src/v1/assist.rs`
  (`POST /api/v1/assist/ask`, read-only, tenant-scoped, role-gated) + registro en
  `v1/mod.rs`/`routes.rs` (una línea anidada) · **ADR-0016** (arquitectura del agente,
  stance offline-first + camino opt-in LLM) · migración 0031 SOLO si se loguea
  (`assist_query` tenant-scoped, opcional).
- **Por qué reversible/seguro:** es un PR, no un release; es add-only, opt-in, offline.
  No viola ningún invariante. El fundador decide el camino LLM en ADR-0016 review.

### V2 — MULTI-RUBRO REAL (promesa → hecho) · owner: **marvin** (backend seed)

La vitrina promete 8 rubros; solo farmacia/minimarket tienen profundidad real. Hacer
REAL un rubro de **servicio** (belleza/servicios, `physicalStock:false`) — la prueba de
fuego del core agnóstico: **vender un servicio SIN stock físico, end-to-end**, y que la
boleta SII emita igual.

- **Entregables:** seed pack de rubro servicio en `crates/domain/src/seed.rs` (servicios
  como ítems vendibles, sin batch/vencimiento, precio CLP plausible, ≥1 "proveedor"/
  contexto si aplica) + extensión del CLI `seed-demo` + test de integración que prueba el
  POS vendiendo un servicio sin stock y la boleta emitiendo.
- **Aislamiento:** backend-only. NO tocar `vertical.ts` (es de ye) → cero contención. El
  cableado cliente (mapping rubro→seed) es de ye o follow-up; marvin entrega el pack
  invocable + el test que prueba el path agnóstico en vivo.
- **Disciplina anti-framework:** UN rubro de servicio real ahora (no los 6). Profundidad,
  no amplitud especulativa.

### V3 — POS PRODUCIDO (el trato vitrina, generalizado) · owner: **paul** (client POS)

Solo la vitrina recibió craft. El **POS** es la pantalla donde el cajero VIVE todo el día.
Elevarlo al mismo grado.

- **Entregables:** `client/src/views/pos.ts` (+ helpers POS) + brand css (append):
  jerarquía/espaciado refinado, accent system, micro-motion (`prefers-reduced-motion`
  respetado), **keyboard-first** (cajero sin mouse — pagar/buscar/boleta por teclado),
  estados empty/loading/error producidos, feedback de venta <100ms sin jank.
- **Vara de craft:** [`rubro-select-experience.md`](./rubro-select-experience.md) §1
  (principios) + §9 (DoD). "Cada pixel intencional", dark-theme nivel Linear/Stripe.
- **Aislamiento:** su scope (pos/caja/devoluciones/clientes). `format.ts` solo lectura
  (append es de bob). Selectores css propios (no pisar los de ye).

### V4 — ACTIVACIÓN + DASHBOARD (primer valor + home del dueño) · owner: **ye**

Un producto gratis vive o muere en la **activación**: instalar → *"veo mi negocio"*
rápido y con gusto.

- **Entregables:** Dashboard (el home del dueño, hoy "de dev") → grado-vitrina + **guided
  first-value**: importar productos → primera venta → dashboard poblado. Empty-states con
  CTA que enseñan (fresh→Importar, stock-only→POS). Cuando el rubro es de servicio (V2),
  onboarding/dashboard lo reflejan nativo (sin "stock vacío" que confunda a un peluquero).
- **Scope:** `views/{dashboard,first-run,configuracion,shell}.ts` + `vertical.ts`
  (append, suyo) + brand css (append, selectores propios).
- **Vara de craft:** misma que V3.

### V5 — INSIGHT ACCIONABLE (por qué el dueño vuelve) · owner: **bob** (reports + e2e)

Los reports hoy son tablas de datos. **Insight** = *"tus 5 productos que más caen",
"margen bajó 8% vs el mes pasado", "$X en riesgo por vencer este mes"*. Esa es la razón de
abrir la app cada día — y **alimenta V1** (el agente surface estos insights).

- **Entregables:** `client/src/views/reports.ts`: tarjetas de insight accionable (deltas
  vs período previo, money-at-risk por vencimiento, top movers/fallers) sobre los feeds
  existentes + e2e de las superficies nuevas + ambos verticales.
- **Scope:** reports/compliance + `client/e2e/*.test.ts` + `format.ts` (append, canónico
  suyo). Gating Pro respetado donde aplique (margins = Pro).

---

## 2. Secuencia & aislamiento (cero contención de merge)

- Las 5 lanes ramifican off **`origin/feature/erp-parity` @5262607 FRESCO** (local va
  ~159 commits atrás — GROUND TRUTH: la verdad es origin).
- **Frontera de archivos** (no se pisan): `vertical.ts` → SOLO ye · router api
  (`v1/mod.rs`/`routes.rs`) → SOLO milton · `seed.rs` → SOLO marvin · `format.ts` →
  append-only, bob canónico · brand css → append con selectores distintos (paul=pos,
  ye=dashboard).
- **Migraciones append-only:** próxima libre = **0031** → milton si `assist` loguea
  (los demás no necesitan migración). Multi-tenant obligatorio en tabla nueva.
- **Slices chicos:** cada lane = PRs pequeños y verdes (no un PR gigante). GATE antes de
  cada PR. **paxoloop integra por fase** (no apilar 15 PRs — lección de olas pasadas).

## 3. Definición de hecho (elevación de producto, no feature-count)

- [ ] El dueño le **pregunta** algo a su negocio y recibe una respuesta real (V1, offline).
- [ ] Un **3er/4to rubro es REAL**: vende un servicio sin stock, boleta emite (V2).
- [ ] **POS y Dashboard** se sienten tan producidos como la vitrina (V3/V4).
- [ ] **Reports** dicen qué HACER, no solo qué pasó (V5).
- [ ] Todo **offline-first, multi-rubro, GATE verde, e2e cubierto, cero invariante roto**.
- [ ] Sensación: un dueño piensa *"esto no es un ERP, es MI negocio con un asistente"*.

## 4. Lo que esto NO es (anti-patrones)

- NO es QA/hardening/perf (agotado y verde). NO es launch-prep (cert+piloto = acción
  founder, diferida por decisión 2026-06-19). NO es reescribir lo que ya funciona. NO es
  amplitud especulativa (un rubro de servicio real, no los 6). NO es romper offline-first
  por un LLM (el agente arranca determinístico; el LLM es opt-in futuro vía ADR-0016).

---

> Norte: cada vector sube el techo del PRODUCTO en una superficie distinta —
> diferenciador (agente), promesa cumplida (multi-rubro), craft (POS/dashboard), valor
> recurrente (insight). Juntos convierten "buen ERP gratis" en "RutBusiness".

# RutAgent — Arquitectura de superficies de producto (master plan, para TODO cliente)

> **Directiva fundador (2026-06-26)**: dar sentido coherente a tener
> **RutAgent Windows (MSI)**, **RutAgent Business**, **RutAgent Web** y la página
> **tu-farmacia.cl** — pero **de forma general, para cualquier cliente futuro** (no
> sólo la farmacia piloto). Dejarlo documentado.
>
> Este documento es el **mapa paraguas**: define cada superficie, quién la usa, dónde
> corre, cómo se comunican, cómo se generaliza a cualquier rubro, el *customer journey*
> y el empaquetado comercial. **No reemplaza** los ADRs/planes existentes — los **unifica**.
>
> Relacionado: [`agentic-business-platform.md`](./agentic-business-platform.md) ·
> [`rutagentia-vision.md`](./rutagentia-vision.md) ·
> [`saas-to-agentic-thesis.md`](./saas-to-agentic-thesis.md) ·
> [`rutagent-web-platform-master-plan.md`](./rutagent-web-platform-master-plan.md) ·
> [`freemium-master-plan.md`](./freemium-master-plan.md) ·
> [ADR‑0005](../adr/0005-core-gratis-no-locked-in.md) ·
> [ADR‑0012](../adr/0012-web-onprem-interop.md) ·
> [ADR‑0013](../adr/0013-sync-bidireccional-stock.md) ·
> [ADR‑0014](../adr/0014-dss-storefront-integration.md) ·
> [ADR‑0015](../adr/0015-universal-cross-platform-client.md) ·
> [ADR‑0016](../adr/0016-agent-assist-architecture.md) ·
> [ADR‑0017](../adr/0017-byo-ai-provider.md) ·
> [ADR‑0018](../adr/0018-cloud-multitenant-saas.md) ·
> **[ADR‑0019 taxonomía de superficies](../adr/0019-product-surface-taxonomy.md)** (decisión raíz de este doc).

---

## 0. TL;DR — el modelo en una frase

**1 RUT = 1 negocio = 1 núcleo = 1 agente = N superficies.**

El producto NO son cuatro cosas separadas: es **un solo núcleo por negocio** (sus datos +
su agente IA) al que se asoman **varias superficies** según *quién* mira y *desde dónde*.
El agente es la capa que las unifica; el ERP es **infraestructura invisible** detrás de él.

```
                         ┌──────── EL NEGOCIO (1 RUT) ────────┐
   OPERADOR              │                                    │     CLIENTE FINAL
   (dueño / cajero)      │   ░░ NÚCLEO ░░                     │     (el comprador)
   ───────────────       │   RutAgent Windows (MSI)           │     ─────────────
   ┌──────────────┐      │   = datos + agente + API           │     ┌──────────────┐
   │ RutAgent      │◀────▶│   on-prem · offline-first          │     │ Storefront    │
   │ Business      │ /api │   "el cerebro y la verdad"         │     │ tu-negocio.cl │
   │ (app Windows) │  v1  │                                    │     │ (ecommerce/   │
   └──────────────┘      │            ▲          ▲            │     │  web pública) │
   ┌──────────────┐      │            │ /api/v1  │ seam HTTP  │     └──────▲───────┘
   │ RutAgent Web  │◀────▶│            │          │ (pull cat. │            │ pedidos
   │ (/app · PWA)  │ same │            │          │  push stock│            │ del público
   │ navegador/cel │ orig │            │          │  push ped.)├────────────┘
   └──────────────┘      │            │          ▼            │
   ┌──────────────┐      │   ░░ EL AGENTE ░░ (capa que unifica las superficies del operador)
   │ Agente (chat) │◀─────┤   "háblale a tu negocio en español; él opera el ERP por ti"
   └──────────────┘      └────────────────────────────────────┘
```

- **tu-farmacia.cl** = la **instancia piloto** del storefront (rubro farmacia). Para
  cualquier cliente futuro es **tu-negocio.cl** con la plantilla de su rubro.
- **RutAgent Business** y **RutAgent Web** son **la misma app de operador** en dos
  envolturas (Tauri nativo y web/PWA) sobre la **misma** API del núcleo ([ADR‑0015](../adr/0015-universal-cross-platform-client.md)).
- **RutAgent Windows (MSI)** es **el núcleo**, no una cuarta app: empaqueta el servidor.

---

## 1. Las superficies, definidas (qué es cada una, sin ambigüedad)

| # | Superficie | Qué ES | Quién la usa | Dónde corre | Habla con | Estado |
|---|------------|--------|--------------|-------------|-----------|--------|
| **N** | **RutAgent Windows (MSI)** | El **núcleo**: servidor Rust (`pharma-server`) — datos (SurrealKv), agente (`crates/assist`), API `/api/v1`, servicio Windows. | Nadie lo "usa" directo; es la base que sirve a todas las superficies. | **On-prem** (el PC del negocio). Offline-first. | Es el servidor. Expone API + seam. | ✅ Vendible (MSI v0.1.x) |
| **O1** | **RutAgent Business** | App de **operador nativa** (Tauri 2): POS, inventario, caja, compras, reportes, agente. | Dueño / cajero / químico. | Escritorio (Windows hoy; Android/iOS por Tauri 2). | `/api/v1` del núcleo (LAN o local). | ✅ Cliente Tauri (18 vistas) |
| **O2** | **RutAgent Web** | Misma app de operador en **web/PWA** (`crates/api/static/index.html` `/app`). Instalable en el celular. | Dueño / cajero, desde navegador o teléfono. | Navegador (same-origin del núcleo, o `?api=` a un núcleo remoto). | `/api/v1` del núcleo. | ✅ SPA completa (10 secciones, operativa) |
| **O3** | **El agente** | Capa **conversacional** sobre el operador: "háblale a tu negocio". Vive *dentro* de O1/O2, no es app aparte. | Cualquiera del operador. | Donde corra O1/O2. | `/api/v1/assist/*` → ejecuta tools sobre el núcleo. | ✅ Determinista; BYO‑LLM ([ADR‑0017](../adr/0017-byo-ai-provider.md)) |
| **C1** | **Storefront `tu-negocio.cl`** | **Front-office público**: web/ecommerce que el negocio muestra a **sus** clientes. `tu-farmacia.cl` = la instancia piloto. | El **cliente final** (el comprador). | Nube (Vercel/CF/el PC del negocio por túnel). Repo separado. | **Seam HTTP** al núcleo: pull catálogo · push stock · push pedidos ([ADR‑0012](../adr/0012-web-onprem-interop.md)/[ADR‑0013](../adr/0013-sync-bidireccional-stock.md)/[ADR‑0014](../adr/0014-dss-storefront-integration.md)). | 🟡 Piloto (tu-farmacia.cl); generalización pendiente |
| **+** | **Cloud RutAgent (`rutagent.cl`)** | Tier **hospedado** multi-tenant: el núcleo en la nube para "1 link, cero instalación". | Negocios que no quieren instalar nada. | Nube (multi-tenant). | Las mismas superficies O1/O2/O3 apuntando al núcleo hospedado. | 🟦 Propuesto ([ADR‑0018](../adr/0018-cloud-multitenant-saas.md)) |

**Regla de oro de la taxonomía**: hay **dos audiencias** (OPERADOR vs CLIENTE FINAL) y
**dos lugares** (núcleo vs superficie). Toda superficie cae en una celda:

|                | Corre en el núcleo | Es una superficie remota |
|----------------|--------------------|--------------------------|
| **Operador**   | RutAgent Windows (MSI) = el núcleo | RutAgent Business (nativo) · RutAgent Web (PWA) · Agente |
| **Cliente final** | (los datos/stock viven aquí) | Storefront `tu-negocio.cl` |

---

## 2. Por qué tiene sentido tener las cuatro (y no es redundancia)

Cada superficie existe porque resuelve un **eje distinto**; juntas cubren el producto sin solaparse:

1. **RutAgent Windows (MSI) — soberanía + offline.** El dato vive en el negocio, opera
   sin internet, sin SaaS, sin lock-in ([ADR‑0005](../adr/0005-core-gratis-no-locked-in.md)).
   Es el **diferenciador** frente a todo SaaS. Sin él, no hay producto.
2. **RutAgent Business (nativo) — el puesto de trabajo serio.** POS de mostrador,
   teclado, lector de código, impresora, balanza: rendimiento y periféricos que el
   navegador no da igual. Es donde el cajero pasa el día.
3. **RutAgent Web (PWA) — alcance y movilidad.** Cero instalación, abre en cualquier
   teléfono/tablet, se "instala" como app, sirve para el dueño que mira desde afuera o
   para un negocio que arranca sin tocar un .MSI. **Misma** app, otra envoltura.
4. **Storefront `tu-negocio.cl` — la cara al público.** Lo que ve el *cliente del
   negocio*, no el operador: catálogo, pedidos, presencia web. Convierte el ERP interno
   en **ventas hacia afuera**. Es un **producto distinto con otra audiencia**.

> Analogía: el **núcleo** es la cocina; **Business/Web/agente** son las distintas formas
> en que el equipo opera la cocina; el **storefront** es el salón donde entran los
> clientes. Una cocina, varias estaciones, un salón.

---

## 3. Cómo se comunican (contratos, no acoplamiento)

Dos *seams* HTTP, deliberadamente desacoplados (sin cross-import de código):

### 3.1 Operador ↔ Núcleo — `/api/v1` (interno)
- RutAgent Business y RutAgent Web hablan la **misma API versionada** del núcleo.
- Same-origin cuando la Web la sirve el propio núcleo; o `?api=<url>` + CORS
  (`config [cors] allowed_origins`, [ADR‑0018]) cuando la Web está hospedada aparte.
- El **agente** es un consumidor más de `/api/v1` (`/assist/ask`, `/assist/act` con
  propose→confirm). Tool-first: el consumidor primario futuro de la API es **un agente**.
- Contrato: JWT por tenant, errores accionables en es‑CL, idempotencia en escrituras.

### 3.2 Núcleo ↔ Storefront — seam de sincronización (externo)
Tres operaciones, una sola dirección de verdad (el núcleo manda):
- **Pull catálogo**: el storefront lee productos/precios publicables del núcleo.
- **Push stock** ([ADR‑0013](../adr/0013-sync-bidireccional-stock.md)): cada venta/recepción
  del núcleo empuja el stock nuevo al storefront (webhook fire-and-forget, nunca bloquea el POS).
- **Push pedidos**: un pedido del público en el storefront entra al núcleo como orden.
- Sin cross-import, sin DB compartida, sin CI compartido (regla de scope). El storefront
  puede caerse y el negocio sigue operando offline; el núcleo es la verdad.

> Esto es lo que hace que tu-farmacia.cl tenga sentido **junto** al MSI: no son dos
> productos rivales, son **back-office (núcleo) + front-office (storefront)** del **mismo**
> negocio, unidos por un contrato de 3 verbos.

---

## 4. Generalización multi-rubro (para TODO cliente futuro)

Nada de lo anterior es de farmacia. La farmacia es el **piloto**; el modelo es genérico:

| Pieza pharma (piloto) | Forma genérica (cualquier cliente) |
|-----------------------|------------------------------------|
| `pharma-server` MSI | RutAgent MSI — núcleo vertical-agnostic; lo pharma es un *vertical pack* condicional a `business.vertical` |
| Recetas / controlados | Sólo aparecen si `vertical = farmacia`; otro rubro no los ve |
| `tu-farmacia.cl` | **`tu-negocio.cl`** con la **plantilla del rubro** (DSS: restaurant/café/tienda/belleza/servicios/…) — [ADR‑0014](../adr/0014-dss-storefront-integration.md) |
| Catálogo de productos farma | El catálogo real del negocio (cualquier SKU/servicio) |
| Seed demo pharmacy | `pharma seed-demo --vertical <rubro>` |

**Un solo binario, un solo agente, una sola web** sirven a todos los rubros; el rubro es
una **señal de configuración** (`admin_setting business.vertical`), no un fork. El
storefront cambia de **plantilla** por rubro (reusa la taxonomía de DSS), no de motor.
Ver [`rubro-catalog.md`](./rubro-catalog.md) y [`rubro-select-experience.md`](./rubro-select-experience.md).

---

## 5. Customer journey — un cliente futuro, de principio a fin

```
 (1) DESCUBRE  →  (2) PRUEBA           →  (3) OPERA               →  (4) VENDE AFUERA        →  (5) CRECE
 link/MSI/cloud   demo web (cero       núcleo instalado o cloud;    activa tu-negocio.cl       más cajas/sucursales,
                  instalación) +       opera con Business/Web/       (storefront del rubro)     SII/ISP, sync, federación
                  su agente            agente; datos suyos           sincronizado al núcleo     B2B, BYO‑LLM
   GRATIS  ───────────────────────────────────────►  PAGO (tiers + microtx)  ──────────────────────────►
```

1. **Descubre** — por el demo web (un link), por el MSI gratis, o por el tier cloud.
2. **Prueba** — abre **RutAgent Web** (cero instalación), elige su **rubro**, le habla al
   **agente**. La primera prueba tangible de "un ERP, todos los rubros".
3. **Opera** — instala el **MSI** (gratis, offline) o usa el **cloud**; trabaja con
   **Business** (mostrador) y **Web** (móvil); el **agente** lo asiste. Datos suyos.
4. **Vende afuera** — activa su **storefront `tu-negocio.cl`** (tier pago): su catálogo
   sale a la web, los pedidos entran al núcleo. El ERP interno se vuelve ventas externas.
5. **Crece** — más cajas/sucursales, SII/ISP automático, sync entre nodos, marketplace
   B2B federado, y su propio LLM (BYO‑provider). Cada paso es un *upgrade*, nunca un *quitar*.

---

## 6. Empaquetado comercial (cómo se cobra cada superficie)

Mapea sobre el freemium ya decidido ([`freemium-master-plan.md`](./freemium-master-plan.md), [ADR‑0001](../adr/0001-freemium-pivot.md)). **El núcleo y el operar siempre son gratis**; se cobra por *alcance* y por *salir a la web*:

| Tier / microtx | Qué incluye respecto a las superficies |
|----------------|----------------------------------------|
| **Free** | Núcleo (MSI) + RutAgent Business + RutAgent Web + agente determinista. 1 caja, 1 sucursal, offline total. Para siempre. |
| **Pro** | + reportes premium, 3 cajas, integraciones vía microtx. Mismas superficies. |
| **Business** | + storefront `tu-negocio.cl` sincronizado, 5 sucursales/10 cajas, sync online, SII/ISP auto, federación B2B. |
| **Enterprise** | + white-label del storefront y del operador, multi-cluster, SLA. |
| **Microtx (one-time)** | *Storefront unlock* (activar tu-negocio.cl), *dominio propio*, *branding pack*, *SII unlock*, *BYO‑LLM*, *cajero extra*. |
| **Cloud RutAgent** (suscripción) | Núcleo hospedado (`rutagent.cl`) para quien no instala nada; mismas superficies apuntando a la nube ([ADR‑0018](../adr/0018-cloud-multitenant-saas.md)). |

Invariante: la **superficie de operador** (Business/Web/agente) jamás se cobra por
existir — se cobra por **escala** (cajas/sucursales/sync) y por **el storefront** (salir
al público). Coherente con "core gratis, no lock-in" ([ADR‑0005](../adr/0005-core-gratis-no-locked-in.md)).

---

## 7. Reconciliación de nombres (para no confundir al equipo ni al cliente)

| Nombre que se oye | Qué es realmente | Cómo nombrarlo |
|-------------------|------------------|----------------|
| **RutAgent** | La **marca paraguas** + la cara agéntica del producto. | Marca del producto entero. |
| **RutBusiness** | Nombre previo de la **identidad de producto** (CLAUDE.md). | = RutAgent Business (el operador). Converge a "RutAgent". |
| **RutAgent Business** | La **app de operador** (ERP: POS/inventario/caja/…), nativa. | Superficie O1 (y O2 en web). |
| **RutAgent Web** | La **misma app de operador en web/PWA**. | Superficie O2. |
| **RutAgent Windows / MSI** | El **núcleo** empaquetado (servidor). | Superficie N (no es una app de usuario). |
| **RutAgentIA** | El **nombre de la visión** agéntica (1 RUT = 1 agente, N dominios). | Visión/norte; misma familia. |
| **tu-farmacia.cl** | La **instancia piloto** del storefront. | Caso particular de `tu-negocio.cl`. |
| **DSS** | El generador de storefronts por rubro del fundador. | Motor de plantillas del storefront. |

> Decisión: **"RutAgent"** es la marca; las superficies se nombran **RutAgent Business**
> (operador nativo), **RutAgent Web** (operador web), **RutAgent (núcleo/Windows)** (el
> servidor), y el storefront es **`tu-negocio.cl`**. `RutBusiness` y `RutAgentIA` son
> sinónimos históricos de la misma familia. El **rename físico** (repo/crates/binarios/MSI)
> sigue **diferido** hasta go explícito del fundador (CLAUDE.md). Ver [ADR‑0019](../adr/0019-product-surface-taxonomy.md).

---

## 8. Estado hoy vs objetivo (qué falta para que esto sea real para clientes)

| Superficie | Hoy | Falta para "listo para cliente cualquiera" |
|------------|-----|---------------------------------------------|
| Núcleo MSI | ✅ vendible, offline, multi-rubro | Cert Authenticode (Fase 9), 2º vertical validado |
| RutAgent Business (Tauri) | ✅ 18 vistas | Builds Android/iOS firmados ([ADR‑0015](../adr/0015-universal-cross-platform-client.md)) |
| RutAgent Web (PWA) | ✅ 10 secciones operativas, instalable | Service worker offline real; hosting en rutagent.cl |
| Agente | ✅ determinista + acciones | BYO‑LLM productivo ([ADR‑0017](../adr/0017-byo-ai-provider.md)); orquestador multi-paso (Fase 15) |
| Storefront `tu-negocio.cl` | 🟡 piloto pharma | Generalizar plantilla por rubro + activación 1‑click + seam sync L1‑L4 ([ADR‑0014](../adr/0014-dss-storefront-integration.md)) |
| Cloud `rutagent.cl` | 🟦 propuesto | Implementar tier hospedado ([ADR‑0018](../adr/0018-cloud-multitenant-saas.md)) |

**Camino crítico para "vender a cualquier cliente"**: (a) cert MSI; (b) generalizar el
storefront a `tu-negocio.cl` por rubro con activación 1‑click; (c) cloud opt-in para los
que no instalan. Todo lo demás ya está o es *upgrade* incremental.

---

## 9. Invariantes (lo que NUNCA cambia, sea cual sea la superficie)

1. **El núcleo es la verdad y vive en el negocio** (o en su tenant cloud aislado). Las
   superficies son **vistas**; ninguna es dueña del dato.
2. **El operar es gratis y offline-first.** Se cobra alcance y storefront, nunca el derecho a operar.
3. **Sin lock-in**: export CSV/JSON completo; el cliente es dueño de sus datos ([ADR‑0005](../adr/0005-core-gratis-no-locked-in.md)).
4. **Seams, no merges**: operador↔núcleo por `/api/v1`; núcleo↔storefront por 3 verbos.
   Cero cross-import entre repos.
5. **Multi-rubro por configuración, no por fork**: un binario, un agente, una web; el
   rubro es una señal.
6. **El agente es opt-in y nunca rompe offline** ([ADR‑0005](../adr/0005-core-gratis-no-locked-in.md) #2; [ADR‑0017](../adr/0017-byo-ai-provider.md)).

---

*Doc vivo. Decisión raíz: [ADR‑0019](../adr/0019-product-surface-taxonomy.md). Última actualización: 2026-06-26.*

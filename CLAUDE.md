# pharma-server — Project Context

> ## 📱 PISO DE HARDWARE — CELULARES VIEJOS Y LENTOS (founder, 2026-08-06) — restricción dura
> **El usuario real puede ser una persona mayor con un celular viejo, lento y casi
> sin espacio.** Esa es la máquina objetivo, no un caso borde. Toda decisión de
> cliente móvil se mide contra ella; si algo solo funciona bien en un teléfono
> nuevo, no está terminado. Reglas que se derivan y NO se negocian:
>
> 1. **Alcance de versión: `minSdk 23`** (Android 6.0, 2015). La app Tauri era
>    `minSdk 24` y dejaba fuera aparatos que este producto sí quiere servir; 23
>    los recupera casi todos.
>    *Historial:* se fijó en 21 el 2026-08-06 y se corrigió a 23 el 2026-08-07.
>    AndroidX dejó de soportar API 21-22 a mitad de 2025, así que sostener 21
>    obligaba a congelar Compose, Material3 y lifecycle en versiones **sin
>    parches de seguridad**, y a escribir a mano el cifrado del token (el
>    Keystore de Android 5 solo guarda pares RSA) — código que nunca llegó a
>    correr en un aparato real. Para una app que mueve plata, quedarse sin
>    parches por llegar a teléfonos de 2014 —que además tienen 1 GB de RAM— es
>    mal negocio. **No volver a bajar a 21 sin resolver antes esas dos cosas.**
> 2. **Nunca APK universal.** Siempre AAB con split por ABI, o APKs por ABI. Un
>    teléfono con 8 GB de almacenamiento no puede pagar 4 arquitecturas. Incluir
>    `armeabi-v7a`: hay aparatos de 32 bits todavía en uso.
> 3. **Respetar la accesibilidad del sistema, no reimplementarla.** La persona mayor
>    ya subió el tamaño de letra en Ajustes de Android. La app tiene que obedecer
>    eso: tipografía en `sp`, nada de tamaños fijos, y **probado al 200% de escala**
>    sin que se rompa el layout. Igual con alto contraste, TalkBack y "reducir
>    animaciones".
> 4. **Objetivos táctiles ≥ 56 dp** en los flujos diarios (cobrar, fiar, cobrar
>    deuda). El mínimo de Material es 48 dp; acá se sube porque el pulso no es el
>    de un cajero de 25 años.
> 5. **Presupuesto de memoria**: asumir **1–2 GB de RAM total**. Nada de cargar
>    listas completas en memoria; virtualizar siempre. Medir RSS, no suponerlo.
> 6. **Arranque en frío medido en el aparato lento**, no en el emulador del
>    desarrollador. Baseline Profiles activados.
> 7. **El servidor embebido en el teléfono es OPCIONAL, no obligatorio.** El `.so`
>    del server pesa ~46 MB y SurrealKV necesita su RAM. En un aparato viejo eso
>    puede no entrar. Arquitectura de dos modos, decidido por capacidad del
>    aparato: **cliente liviano** apuntando a un server de la red o la nube, o
>    **nodo completo** con el server adentro. La app tiene que funcionar entera en
>    el modo liviano.
> 8. **Datos móviles caros y lentos** son el caso normal. Offline-first no es solo
>    para el server: la app tiene que ser usable con la red intermitente y no
>    quemar megas.
>
> **Aparato de referencia para pruebas** (si no se probó acá, no está probado):
> 2 GB de RAM, Android 8 o menor, pantalla 720p, almacenamiento casi lleno.

> ## 🌟 META GENERAL DEL PROYECTO (founder, 2026-07-23) — leer SIEMPRE, aplica a TODO el trabajo
> **Un ERP de nivel de gran empresa, tan profesional y completo como el de las grandes,
> pero accesible para CUALQUIER microempresa chilena** — la que hoy se maneja con
> cuaderno, desde el celular, con internet. **El agente IA es la interfaz principal**: en
> vez de tener que hacer las cosas ella misma, la dueña le HABLA al agente y le pide que
> haga todo (vender, fiar, cobrar, reponer, sacar el IVA, ver quién le debe, etc.). El
> ERP es la infraestructura invisible detrás del agente.
> **Superficies objetivo — TODAS de primera clase**: web/PWA · desktop Windows nativo ·
> **app nativa Android + iOS** (el celular es el dispositivo primario del usuario real).
> El cliente ya es Tauri 2 → mismo frontend a móvil nativo; construir pensando en móvil
> (táctil, pantalla chica, una mano) desde el diseño, no como afterthought.
> **Vara de calidad = las grandes empresas** (Defontana, Bsale, Nubox…): muy profesional,
> muy completo, cero dead-ends, confiable con plata real. Pero **gratis / freemium** y
> **hablándole al agente**, no navegando menús. Cada cosa que se construya se mide contra:
> "¿una microempresaria en su celular, sin manual, le pide esto al agente y funciona como
> en un ERP caro?". Esta meta NO caduca — enmarca toda decisión de producto y técnica.

> ## 🥬 FOCO PRINCIPAL ACTIVO — FERIA / CALLE (founder, 2026-08-08) — gana sobre beachhead farmacia
> **Cliente objetivo prioritario:** gente que vende en la feria o en la calle, con
> cuaderno de mil pesos, celular antiguo, poco de tecnología, sol directo, una mano
> libre, a veces sin señal, sin código de barras ni impresora, fía mucho, a menudo
> informal (sin SII día 1).
>
> **Vara de diseño no negociable: el cuaderno.** Se abre al instante, se lee al sol,
> funciona con manos mojadas, no se cuelga. Si una pantalla es más lenta o más
> difícil que anotar una línea a mano, **pierde**. Usá esa vara para **matar o
> esconder** features (escáner, impresora, DTE, catálogo denso), no para sumarlas.
>
> **Producto day-1 para este usuario = el agente** ("vendí tres kilos de tomate a
> dos mil") + fiado + resumen del día + offline. Pack de rubro `feria` en
> `domain::rubro` (ADR-0022). Farmacia, minimarket, etc. **siguen siendo
> verticales completos** del mismo ERP; ya **no** son el norte de producto ni el
> default de onboarding. La directiva de 2026-06-16 "farmacia = beachhead" queda
> **superada** por esta en cuanto a foco y GTM; el código multi-rubro se reutiliza.
>
> Detalle del plan: `C:\obsidian-mind\work\decisions\2026-08-08-plan-feria-orquestador.md`.

> ## 🏗️ ETAPA REAL: ~1% COMPLETO — CONSTRUIR PROFUNDIDAD, NO PULIR SUPERFICIE (founder, 2026-07-21)
> **El proyecto recién lleva ~1%.** La meta ahora NO es infra, dominios ni pulido de
> puerta de entrada, sino **hacer el producto MUCHO más completo y profundo**: features
> reales de ERP con hondura de producto vendible (no MVPs superficiales). El **dominio /
> link público NO es prioridad** (diferido; `nip.io` + `vercel.app` bastan por ahora —
> no invertir tiempo en `rutbusiness.cl`/DNS/branding-de-URL). Cada lane debe SUMAR
> capacidad y profundidad de negocio real (inventario, POS, compras, caja, recetas, SII,
> reportes, multi-sucursal, fiado, agente…), cubriendo casos reales de dueños chilenos de
> punta a punta, no dejar módulos a medias. Regla de decisión: ante "¿pulo lo que hay o
> construyo lo que falta?", **construir lo que falta** hasta que el ERP sea completo. La
> vara de calidad UX/producto sigue vigente para lo que se construye, pero el driver es
> **completitud y profundidad**, no acabado cosmético de la superficie actual.

> ## 🎯 GOAL DEL PROYECTO — RUTBUSINESS (norte, fijado 2026-06-16 por founder)
> **Dar a CADA negocio chileno —cualquier rubro, identificado por su RUT— un ERP
> gratis, offline-first, y su propio agente IA; donde el ERP se vuelve infraestructura
> invisible detrás del agente.** 1 RUT = 1 negocio = 1 agente. Modelo: freemium MSI
> Windows (core gratis para siempre + tiers + microtx, [ADR-0001](./docs/adr/0001-freemium-pivot.md)).
> **Feria / calle = foco GTM principal (2026-08-08).** Farmacia = vertical validado
> histórico, NO el límite ni el norte de producto (ver bloque FOCO PRINCIPAL arriba).
> Fin de juego: ecosistema federado donde los agentes de distintos negocios transan
> entre sí (Ed25519 envelopes, Fase 13). Ver [`docs/strategy/rutagentia-vision.md`](./docs/strategy/rutagentia-vision.md).
>
> ## ⚡ ENFOQUE 100% RUTBUSINESS (founder, 2026-06-16) — directiva activa
> **El producto es RUTBUSINESS**, NO "farmacia". `pharma-server` es solo el nombre del
> repo git; la identidad del producto es **RutBusiness** (multi-rubro). Todo trabajo
> nuevo se enmarca en RutBusiness: **cero** copy/UI/branding/scope pharma-específico
> salvo como *vertical pack* condicional a `business.vertical`. Server + client (Tauri)
> + CLI = piezas de RutBusiness, no de "pharma". Donde el código/doc asuma farmacia →
> generalizar o condicionar al rubro (catálogo: [`docs/strategy/rubro-catalog.md`](./docs/strategy/rubro-catalog.md)).
> Las secciones "para farmacias" más abajo son **histórico** — leer con este lente.
> (RutAgentIA = nombre de la visión agéntica previa; mismo norte, capa más profunda.)
>
> ## 🎬 FOCO DE PRODUCTO ACTIVO (founder, 2026-06-16) — "hacer un buen producto"
> Prioridad = **calidad de producto** (no rename ni cert, diferidos como no-primordiales).
> **Pantalla vitrina = selección de rubro del onboarding** (`configuracion.ts` grid +
> `vertical.ts`): elevarla a **muy profesional / muy producida / detallada / profunda** —
> es la primera prueba tangible de "un ERP, todos los rubros". ULTRA-PLAN detallado en
> [`docs/strategy/rubro-select-experience.md`](./docs/strategy/rubro-select-experience.md).
> Bar de calidad para TODO el equipo: cero dead-ends, cero crash con input basura, feel
> instantáneo (<100ms; POS <50ms p99), keyboard-first, estados empty/error claros en
> español, formato consistente (CLP/RUT/fechas), idéntico en ambos verticales. Manejar
> el binario/stack REAL (tauri dev, `scripts/qa/*.sh`, `npm run e2e`), no solo vitest.
>
> ## 🤝 OBJETIVO FUNDAMENTAL — UX INTUITIVA Y AMIGABLE (founder, 2026-06-21)
> El programa debe ser **MUY intuitivo y amigable** para el usuario real (dueño/cajero
> no-técnico, a veces mayor, a veces en tablet táctil): cualquiera lo entiende **sin
> manual**. NO es una ola — es **vara PERMANENTE** que TODA lane de UI cumple en su DoD
> (igual que la vara de la vitrina, ahora para toda vista). 8 principios no-negociables:
> (1) **tarea diaria de cero fricción** (cobrar/fiar/"¿cuánto vendí?"/reponer = obvias,
> rápidas, a prueba de error); (2) **habla como el dueño** (es-CL humano, cero jerga ni
> keys técnicas como `admin_setting`; errores que dicen QUÉ hacer); (3) **guía, no manual**
> (primer-uso guiado, empty-states que enseñan, tooltips, el agente como ayuda); (4)
> **perdona errores** (confirm/undo en destructivo, validación inline amable); (5)
> **rápido** (keyboard-first + Ctrl+K, <100ms, defaults inteligentes, cero clicks de más);
> (6) **consistente** (un solo lenguaje visual: design system `client/src/views/ui.ts`);
> (7) **accesible + táctil** (contraste AA, teclado total, tipografía legible, modo touch
> POS); (8) **confianza** (toasts de cada acción, estados loading/empty/error producidos,
> NUNCA pantalla en blanco). Ultra-plan: [`docs/strategy/intuitive-ux-master-plan.md`](./docs/strategy/intuitive-ux-master-plan.md).

> ## 🇨🇱 VISIÓN RUTAGENT — DEL CUADERNO AL ERP, HABLANDO (founder, Coquimbo, 2026-07-26)
> **Que cualquier chileno con un cuaderno y un celular pueda, en 5 minutos y sin pagar
> nada, tener el mismo ERP que una gran empresa — hablándole a un agente en vez de
> aprender un sistema.** El usuario real NO es "una PYME": es la señora del almacén que
> fía en un cuaderno, el feriante de Coquimbo/La Serena, la repostera que vende por
> WhatsApp/Instagram, el que recién parte y no puede pagar Bsale/Nubox ni un contador.
> Denominador común: **smartphone + WhatsApp + cuaderno + miedo a la complejidad.**
> **El competidor real es el cuaderno**, no Defontana — y el cuaderno gana en instantáneo,
> confiable, habla chileno, gratis. Cualquier ERP que exija *aprender un ERP* ya perdió.
> Entonces: no reemplazamos el cuaderno con un sistema, lo reemplazamos con **un cuaderno
> con superpoderes que habla chileno** — el agente ES el cuaderno; el ERP es la tinta
> invisible.
> **5 principios no-negociables**: (1) **agente-first, no menú-first** — la puerta es una
> frase (*"fié 5 lucas a la señora Rosa"*, *"¿quién me debe?"*), el agente cierra el loop
> completo del negocio de barrio; (2) **cero fricción de entrada = el producto** — web sin
> instalar, alta con RUT en 30s, tienda sembrada por rubro, PWA tolerante a internet malo,
> NUNCA tarjeta de crédito; (3) **puente analógico→digital: importar el cuaderno con una
> FOTO** — foto de la hoja (fiados/productos/precios) y el agente lo carga; es el acto de
> adopción killer; (4) **formalización como REGALO, no amenaza** — boleta electrónica SII +
> IVA/F29 + inicio de actividades, gratis y fácil; el agente es el contador que no podían
> pagar; cada informal formalizado es el impacto social; (5) **profundidad que se gana la
> confianza — cero módulos a medias** — ledgers inmutables, plata sin bugs, GATE + DoD,
> vara = las grandes / precio = cero.
> **Impacto — Coquimbo primero**: sembrar en una ciudad (ferias, caletas, almacenes de
> barrio) con negocios reales; crecer por **boca a boca de barrio** (corre por confianza,
> como el fiado) + aliados locales (municipalidad, Sercotec, cámara, Prodemu). Meta: que
> empezar un negocio formal en Chile no necesite plata, contador ni saber de sistemas —
> solo un RUT, un celular y una frase. Y desde Coquimbo, todo Chile.
> **Regla de oro / vara de todo**: *"¿esto acerca a la señora Rosa, en su celular, sin
> manual, a tener el ERP de una gran empresa gratis?"* Si no, no va.
> Ultra-plan completo: [`docs/strategy/rutagent-vision-microempresa-cl.md`](./docs/strategy/rutagent-vision-microempresa-cl.md).
> Vectores de profundidad candidatos (V4+): boleta SII gratis punta-a-punta (mayor impacto
> social) · importar-cuaderno-por-foto/OCR (mayor impacto de adopción) · pagos transferencia
> + apps · puente WhatsApp · mermas/vencimientos por local · agente coach financiero · modo
> feria/offline.

Servidor Rust on-prem **multi-rubro (RutBusiness)**: ERP genérico para cualquier negocio CL (1 RUT). Foco producto 2026-08-08 = feria/calle; farmacia y otros rubros = packs del mismo core. Single binary instalable vía MSI, axum HTTP API + SurrealDB embedded (kv-surrealkv) + Windows service. Producto **vendible**, offline-first, vendor-agnostic. Clientes en este repo: `client/` (Tauri desktop + PWA) y `client-android/` (Compose nativo).
**Estado**: v0.1.28 · branch `main` (canónica; `feature/erp-parity` ya promovida 2026-07-19) · ERP multi-rubro en main (inventario/variants, POS, compras, caja, fiado, rubro-pack, country-pack, DTE, license, agent) · **Android Compose nativo** en `client-android/` ([ADR-0021](./docs/adr/0021-android-compose-nativo.md)) · desktop/PWA en `client/` · Free Web en `crates/api/static/` · suite workspace ~1000+ tests (audit 2026-08-08: 1019 passed) · **MSI release** histórico v0.1.23 (https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.23) · freemium + Fase 10 license MVP + Fase 11 base mergeadas · **PIVOTE freemium MSI (2026-05-20)** → `docs/strategy/freemium-master-plan.md`.

**Visión extendida (2026-05-16, actualizada 2026-05-20)** → ver [`docs/strategy/ecosystem-roadmap.md`](./docs/strategy/ecosystem-roadmap.md). Pharma-server no es solo ERP vendible; es **nodo de un ecosistema federado de agentes ERP** (farmacias, proveedores, droguerías) donde humanos reales operan cada nodo y transan vía protocolo común (Ed25519-signed JSON envelopes sobre HTTP/NATS). El modelo comercial es **freemium MSI Windows estilo LoL** (core gratis + tiers + microtx) — ver [`docs/strategy/freemium-master-plan.md`](./docs/strategy/freemium-master-plan.md) y [ADR-0001](./docs/adr/0001-freemium-pivot.md). Fase 13 = capa de confianza/marketplace B2B → ver [`docs/strategy/b2b-marketplace.md`](./docs/strategy/b2b-marketplace.md). **Posicionamiento de mercado (reframe 2026-05-27)**: el producto es *infraestructura competitiva para el independiente* frente al oligopolio (~90% Ahumada/Cruz Verde/Salcobrand), no "otro ERP" — mercado subdigitalizado (no saturado), moat de 4 capas (POS = caballo de Troya → datos agregados → poder de compra colectivo → red operacional), riesgo = distribución+confianza no técnico → ver [`docs/strategy/market-thesis.md`](./docs/strategy/market-thesis.md). **Tesis unificadora 2026-2035** (visión, moat, flywheel, AI-native, LATAM multi-país, distribución, integraciones-as-platform) → ver [`docs/strategy/latam-master-plan.md`](./docs/strategy/latam-master-plan.md).

**Visión norte — RutAgentIA, plataforma agéntica multi-rubro (directiva fundador 2026-06-09)** → ver [`docs/strategy/agentic-business-platform.md`](./docs/strategy/agentic-business-platform.md). El destino del proyecto NO es sólo farmacias: es una **plataforma de operación de negocios agéntica para cualquier rubro**, y su nombre es **RutAgentIA** — *un agente IA para cada chileno (persona o empresa), con su RUT como identidad* (`RUT ↔ DID ↔ keypair Ed25519`, esquema conceptual `did:rut:`), que le gestiona sus dominios de vida: **finanzas, negocios, salud, etc.** (un RUT = un agente = N dominios; cada dominio es un *pack*, mismo patrón que vertical pack). Renombres físicos (repo/crates/binarios/MSI) PENDIENTES — tarea aparte con go explícito del fundador; `pharma-server` = nodo ERP vertical farmacia dentro de la plataforma. **Tesis primeros-principios SaaS→Agentic Company** (por qué la era agéntica mata la *interfaz* del ERP y promueve su *núcleo* a infraestructura; etapas Tool→Worker→Team→Company; transición auto-financiada; qué construir primero y qué NO construir nunca) → ver [`docs/strategy/saas-to-agentic-thesis.md`](./docs/strategy/saas-to-agentic-thesis.md). Modelo operativo objetivo: `Usuario —(objetivos)→ Agente orquestador IA → Agentes coordinadores → Agentes de equipo → Tools (API /api/v1 + CLI)`; principio rector: `Humano → Agente IA → Software → Datos` (el humano declara objetivos, los agentes operan el software). Farmacia = beachhead/primer vertical, no el límite del producto. Implicaciones activas desde hoy: core vertical-agnostic (lo pharma-específico se modulariza como *vertical pack*), API tool-first (schemas utoipa estrictos, errores accionables, idempotencia — el consumidor primario futuro es un agente), acciones de agente firmadas Ed25519 (`crates/agent`) + audit log con atribución `agent_id`, human-in-the-loop para lo irreversible, y capa agéntica **opt-in** que jamás rompe offline-first (ADR-0005 #2). Materialización = Fase 15 (post-revenue); no bloquea Fases 9-14.

## Producto / Visión comercial

**Meta**: **RutBusiness** — ERP profesional **multi-rubro** para cualquier negocio chileno (1 RUT), vendible como producto on-prem (MSI freemium + tiers + soporte). Comprador prioritario: feriantes y vendedores de calle (cuaderno → celular). También
negocios independientes de **cualquier rubro** (farmacia, minimarket, restaurant, café,
tienda, belleza, servicios…) que quieren todo local, sin SaaS, sin cloud, sin lock-in.
Farmacia fue el primer vertical con profundidad; feria es el foco de adopción 2026-08.

Pilares de venta (no negociables):
- **Instalación 1 click** (MSI firmado, sin dependencias externas, sin Docker, sin Postgres aparte).
- **Offline-first**: opera sin internet. LAN-only. Datos siempre en la farmacia.
- **Multi-tenant** (una instalación, N sucursales/locales o N clientes en SaaS-en-VPS opcional).
- **Cumplimiento local CL**: boleta electrónica SII, libro de controlados ISP/DEIS, recetas magistrales, vencimientos, lotes, trazabilidad.
- **Auditoría completa**: cada cambio de stock/precio/venta queda en log inmutable.
- **Performance**: POS responde <100ms incluso con 50k SKUs. SurrealKv embedded = sin red en hot path.
- **Vendor-agnostic**: exporta CSV/JSON sin formatos propietarios. El cliente es dueño de sus datos.

Módulos objetivo (roadmap producto, no scaffold):
1. **Inventario**: SKU, lote, vencimiento, stock por bodega, alertas mínimos.
2. **POS / Ventas**: tickets, medios de pago, devoluciones, descuentos, convenios isapre.
3. **Compras**: OC, recepción, costo promedio ponderado.
4. **Recetas**: receta retenida, receta cheque, controlados (Ley 20.000).
5. **Caja**: apertura/cierre, arqueo, diferencias.
6. **Reportes**: ventas, márgenes, rotación, ABC, vencimientos próximos.
7. **Integraciones (opt-in)**: SII (DTE), ISP, transbank/getnet, balanza, lector códigos.
8. **Backup**: snapshot SurrealKv programado + restore guiado.
9. **Usuarios/roles**: cajero, químico, admin, dueño. Permisos por módulo.

Modelo de negocio: ver § "Modelo de negocio (freemium, lockeado)" abajo.

Reglas de diseño derivadas:
- **No agregar dependencia cloud** sin opción de operar offline.
- **No romper compat de DB** sin migración automática (cliente NO debe perder datos al actualizar).
- **UI desacoplada**: el server expone API HTTP/JSON estable y versionada (`/api/v1/...`). Frontend (POS, admin) es cliente separado.
- **Errores en español** en respuestas user-facing (códigos en inglés OK para devs).
- **Performance budget**: endpoints POS <50ms p99 en hardware mínimo (i3 + SSD + 8GB).

## Modelo de negocio (freemium, lockeado)

Decidido 2026-05-20. **Pivote** de licencia única → **MSI Windows freemium estilo LoL**: core gratis + tiers pagos + microtransacciones one-time. Detalle completo en [`docs/strategy/freemium-master-plan.md`](./docs/strategy/freemium-master-plan.md). Decisión raíz: [ADR-0001](./docs/adr/0001-freemium-pivot.md).

**Tiers** (resumen — ver master plan §3 para matrix completa):
- **Free** — POS + inventario + caja + gastos + recetas + backup local + sales-daily + 1 caja + 1 sucursal.
- **Pro** — 3 cajas, reportes margins/top-products, integraciones via microtx.
- **Business** — 10 cajas, 5 sucursales, sync online, SII/ISP auto, federación quote+PO.
- **Enterprise** — ilimitado, white-label, multi-cluster, SLA 4h.

**Microtx one-time** (catálogo cerrado v1): Branding pack, SII unlock, Telegram bot, Premium reports pack, Extra cashier seat, Premium support credits.

**Invariantes NO negociables** (codificados en [ADR-0005](./docs/adr/0005-core-gratis-no-locked-in.md)):
1. Core ERP siempre gratis offline. Capacidades sólo se *agregan* al Free, nunca se quitan.
2. License OFFLINE-FIRST — server NO requiere internet para operar features ya activadas.
3. Telemetría OPT-IN siempre, default OFF, sin PII (Ley 19.628).
4. Sin lock-in de datos — Free incluye export CSV/JSON completo de todo.
5. Sin dark patterns — máx 1 upgrade prompt/sesión, cero en POS hot path.
6. Sin kill-switch remoto — core gratis sigue operativo aunque license expire/revoque.
7. Compromiso de continuidad — si la empresa cierra, last release queda funcional indefinida.

**Arquitectura técnica del licenciamiento** ([`docs/strategy/license-architecture.md`](./docs/strategy/license-architecture.md), [ADR-0002](./docs/adr/0002-license-ed25519-offline.md)):
- License = JSON Ed25519-firmado (reusa `crates/agent/identity.rs` + `envelope.rs`).
- Pubkey del licenser embebida en binario. Validación 100% local.
- Feature gate API: `entitled(feature) -> bool` + `require(feature) -> Result` (retorna 402 `FEATURE_REQUIRES_UPGRADE`).
- CRL firmado distribuido por CDN ([ADR-0006](./docs/adr/0006-revocation-strategy-signed-crl.md)). Refresh opcional.
- Key rotation multi-key con `key_id` ([ADR-0007](./docs/adr/0007-key-rotation-licenser.md)).
- License-server vive en **repo separado** `pharma-license-server` ([ADR-0004](./docs/adr/0004-license-server-separado.md)).

**Pagos** ([`docs/strategy/payments-cl.md`](./docs/strategy/payments-cl.md), [ADR-0003](./docs/adr/0003-payments-webpay-first.md)):
- Webpay primario (Pro/Business sub + microtx CL) — **target de escala**.
- Stripe secundario (microtx con tarjeta internacional, Fase 11.1).
- Khipu para Enterprise (Fase 11.2).
- Mercado Pago para LATAM multi-país (Fase 11.3+).
- **Orden pilot DIFERIDO** ([ADR-0009](./docs/adr/0009-pilot-payment-provider.md)): en pilot phase **Mercado Pago + Stripe van primero** (Webpay requiere RUT empresa + onboarding 2-4 sem). Webpay se reactiva al constituir SpA.

**Camino $0 a primer cobro** ([`docs/strategy/zero-cost-launch-plan.md`](./docs/strategy/zero-cost-launch-plan.md), 2026-05-27): plan operativo para desbloquear Fase 9+11 con **0 USD gastados** hasta el primer cobro. Self-sign MSI ([ADR-0008](./docs/adr/0008-self-sign-pilot-msi.md), scripts `installer/sign/`) + smoke Hyper-V (scripts `installer/smoke/`) + MP/Stripe pilot ([ADR-0009](./docs/adr/0009-pilot-payment-provider.md)) + license-server free-tier ([`license-server-skeleton.md`](./docs/strategy/license-server-skeleton.md)). **Si el fundador dice "continúa con el plan zero-cost" → ejecutar siguiente paso pendiente del §5 día-a-día.**

## Roadmap (fases)

Renumerado 2026-05-20 post-pivote. Estado en `bitacora.md` ## BACKLOG.

- **Fase 9** — MSI vendible v1.0.0 (firma Authenticode + smoke VM Windows limpia). **Workaround $0** ([`zero-cost-launch-plan.md`](./docs/strategy/zero-cost-launch-plan.md)): self-sign cert pilot ([ADR-0008](./docs/adr/0008-self-sign-pilot-msi.md)) + Hyper-V smoke desbloquean sin gastar; cert pago/MSIX cuando entre revenue.
- **Fase 10** — License/entitlement layer:
  - 10a `crates/license` crate nuevo (Ed25519 verify + parser, reusa `crates/agent`).
  - 10b Feature gate API (`entitled`/`require`) + `ApiError::payment_required` 402.
  - 10c CLI `pharma license import|status|features`.
  - 10d 1 feature gated POC (sugerencia: `reports.margins_daily`).
- **Fase 11** — Payment rails + license-server integration. **El repo `pharma-license-server` YA EXISTE** (privado, Fase 11b code-complete con Webpay sandbox). Estado real + gaps: [`license-server-skeleton.md`](./docs/strategy/license-server-skeleton.md).
  - 11a ✅ Scaffold Next.js 14 + Prisma + `@noble/ed25519`, canonical JSON cross-repo verificado.
  - 11b ✅ (code-complete, deploy pendiente) Webpay sandbox + admin issuance + checkout UI.
  - 11b-gap **embeber prod key `lk-prod-2026-01` en `crates/license/src/keys.rs`** (hoy placeholder `lk-dev-2026`) + deploy Vercel+Neon free.
  - 11c **Mercado Pago** como primer rail LIVE para cobro real sin SpA ([ADR-0009](./docs/adr/0009-pilot-payment-provider.md)); Webpay LIVE cuando SpA; Stripe (schema listo) para internacional.
- **Fase 12** — Sync online opt-in entre nodos (paid tier).
- **Fase 13** — Marketplace federado B2B ([`docs/strategy/b2b-marketplace.md`](./docs/strategy/b2b-marketplace.md)). Capa de confianza/reputación.
- **Fase 14** — Cloud companion (web admin + mobile dashboard, opt-in).
- **Fase 15** — Capa agéntica multi-rubro ([`docs/strategy/agentic-business-platform.md`](./docs/strategy/agentic-business-platform.md)): 15a tool surface MCP/OpenAPI para agentes · 15b orquestador MVP (objetivo NL → plan → ejecución vía `/api/v1`, human-in-the-loop) · 15c jerarquía coordinador/equipo (Envelope Ed25519) · 15d vertical packs (extraer pharma a pack; 2º vertical piloto).

## Scope de este repo (IMPORTANTE)

Este repo (`pharma-server`) es **exclusivamente para el servidor Rust on-prem genérico**. Producto a empaquetar como MSI y vender a farmacias que quieren todo local (sin cloud, vendor-agnostic, offline-first).

Repos relacionados, **separados a propósito**:

- `pabloalvarez99/pharma-server` (este, privado) → Servidor Rust on-prem experimental. Un MSI, N tenants en LAN, SurrealDB embedded.
- `pabloalvarez99/build-and-deploy-webdev-asap` → Tu Farmacia LOCAL REAL en Coquimbo. Next.js 14 + Cloud SQL Postgres 15 + Firebase + Vercel. Cliente real en producción. Ver `C:/Users/Administrator/Documents/GitHub/build-and-deploy-webdev-asap/`.

Regla: cualquier feature de servidor on-prem genérico va aquí. Cualquier cosa de la farmacia real de Coquimbo va al otro repo. **No cross-imports, no shared CI, no shared deploy, no mezclar deps.**

## Stack (esencial)

Versiones leídas de `Cargo.toml` (workspace). MSRV vs pin: `rust-version = 1.85` (MSRV — código compila desde 1.85+) y `rust-toolchain.toml = 1.95.0` (versión que usa dev/CI). Compatible por diseño.

- Rust 1.95.0 pin (`rust-toolchain.toml`) · MSRV 1.85 · edition 2021 · target `x86_64-pc-windows-msvc`
- axum 0.8 + tower 0.5 + tower-http 0.6 + hyper 1.5
- utoipa 5 + utoipa-axum 0.1 + utoipa-swagger-ui 8
- surrealdb 2.1 con feature `kv-surrealkv` (LSM puro Rust, evita libclang/RocksDB en Windows)
- jsonwebtoken 9 (HS256) + argon2 0.5 + uuid 1.11
- tokio 1.41 · tracing 0.1 · tracing-opentelemetry 0.28 · opentelemetry 0.27 · opentelemetry-otlp 0.27 · axum-prometheus 0.7
- tokio-cron-scheduler 0.13 · async-nats 0.38 (ambos sin uso real todavía)
- windows-service 0.7 · clap 4 · config 0.14 · chrono 0.4 · thiserror 2 · anyhow 1
- MSI: cargo-wix 0.3.9 + WiX v3.14 (`installer/wix/main.wxs` con `ServiceInstall` + `ServiceControl` + firewall TCP 8080; smoke install/uninstall verificado)
- CI: GitHub Actions windows-latest (`.github/workflows/ci.yml`)

Crates (`Cargo.toml` raíz):

| Crate | Rol |
|-------|-----|
| `core` | Domain types (`TenantId`), `Error` enum (thiserror), `AppConfig` loader (config crate) |
| `db` | SurrealDB client (`Surreal<LocalDb>` + SurrealKv), migración runner con tracking `_migrations` |
| `api` | Axum HTTP server. Bin `pharma-api`. Expone `lib::run` para que service lo embeba |
| `auth` | JWT issue/verify (HS256, validación issuer) + argon2id password hash/verify |
| `jobs` | Cron scheduler (vacío hoy) |
| `telemetry` | `tracing_subscriber` JSON + EnvFilter + OTLP gRPC tonic exporter (opt-in vía `PHARMA__OTLP__ENDPOINT`) |
| `service` | Windows service host, name `PharmaServer`, type `OWN_PROCESS`. Bin `pharma-service` |
| `cli` | Admin CLI. Bin `pharma`. Comandos: `migrate`, `config`, `tenant-create`, `user-create` (argon2id, password vía flag/`PHARMA_PASSWORD`/prompt) |

## Reglas siempre activas

1. **Build local antes de push — GATE scope-aware (rápido por defecto, directiva fundador 2026-05-30)**: NO correr siempre el workspace completo en release. Elegir el alcance mínimo correcto:
   - **Cambio sólo-docs / `.md` / assets binarios (iconos, imágenes)**: cero cargo. Va directo a commit+push. (Ej: el icono ERP no toca Rust → GATE cargo no aplica.)
   - **Cambio sólo-client (TS/Tauri, `client/`)**: `cd client && npm run build` (corre `tsc --noEmit` + vite). NO corre cargo del workspace Rust.
   - **Cambio Rust en 1 crate hoja** (no `core`/`db`/`auth`): GATE acotado, **debug no release** →
     ```powershell
     cargo fmt --all -- --check
     cargo clippy -p <crate> --all-targets -- -D warnings
     cargo test -p <crate>
     ```
   - **Cambio en crate compartido** (`core`/`db`/`auth`) o cross-crate: GATE workspace completo (debug) →
     ```powershell
     cargo fmt --all -- --check
     cargo clippy --workspace --all-targets -- -D warnings
     cargo test --workspace
     ```
   - **`--release` SÓLO para cortar MSI** (regla #6/#9), nunca para iterar.
   - Prefijar comandos pesados con `rtk` si disponible. Dejar `target/` cacheado, no borrar entre branches. CI corre el workspace completo igual (Swatinem cachea) — el GATE local acotado es para velocidad; CI es la red de seguridad full.

2. **Pre-commit** = el GATE de scope correcto de la regla #1 (mismo set que CI con `-D warnings`). El workspace completo no es obligatorio local para cambios acotados; CI lo corre igual.

3. **Migraciones append-only**: NUNCA editar `migrations/NNNN_*.surql` ya aplicada. Naming `NNNN_descripcion.surql`. id en tracking = filename stem (e.g. `0001_init`). Para cambiar schema → nueva migración `NNNN+1_*`.

4. **Multi-tenant obligatorio**: cada tabla de dominio nueva DEBE llevar campo `tenant: record<tenant>` + índice compuesto que incluya `tenant`. Patrón en `migrations/0001_init.surql` (`user`, `session`). Todo filtrado por tenant via JWT claim `tenant_id`.

5. **Windows service**: probar ciclo completo `sc create / start / stop / delete` (o `pharma service ...` cuando se implemente) en VM antes de cortar MSI. Service install requiere admin elevado. Service y CLI no deben correr a la vez sobre `./data/surreal` (SurrealKv file lock).

6. **MSI**: SemVer estricto en `workspace.package.version`. Firmar con cert si está disponible (sin firmar = SmartScreen warning). Smoke-test instalación limpia + upgrade (MajorUpgrade ya en wxs). `installer/wix/main.wxs` `ServiceComponents` está vacío hoy → bloqueante M3.

7. **Bitácora dual**: cada cambio significativo → append en (a) `bitacora.md` (repo, commit history) y (b) `C:/Users/Administrator/Documents/obsidian-mind/work/active/pharma-server/bitacora.md` (vault, búsqueda). Después actualizar `work/active/pharma-server/decisions-log-index.md` con la línea nueva.

8. **Secrets**: nunca commitear `config/local.toml` ni `data/`. JWT secret de `config/default.toml` (`change-me-in-production`) es placeholder; producción inyecta vía env `PHARMA__JWT__SECRET`. Loader: `config/default.toml` → `config/local.toml` (opcional) → env `PHARMA__*` separator `__`.

9. **Commit + push + deploy SIEMPRE tras GATE verde** (directiva fundador 2026-05-27, override de versión previa): cualquier branch que pase GATE (`cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace`) → commit con mensaje descriptivo + push a origin + abrir PR contra base correcta + deploy automático.

   **SIEMPRE commit + push sin pedir aprobación (directiva fundador 2026-05-30)**: en cuanto un cambio esté listo y pase GATE → `git commit` + `git push` + PR de inmediato, autónomo, SIN esperar confirmación del usuario. NO preguntar "¿quieres que commitee/pushee?". Default = commit & push ya. Sólo siguen requiriendo confirmación las excepciones explícitas abajo (force-push, source público regla #10, MSI release deploy, acciones destructivas/irreversibles). Todo lo demás se commitea y pushea sin preguntar.

   **Deploy = MSI release al mirror público** (`release-publisher.yml` workflow_dispatch contra `pharma-server-releases`) **una vez que** prerequisitos técnicos estén verdes:
   - cert Authenticode válido cargado (sin esto el MSI sale con SmartScreen warning — bloqueante técnico, no de policy);
   - smoke-test instalación limpia en VM Windows verde;
   - no hay bugs P0 abiertos en triage.
   Si los 3 prerequisitos están verdes → deploy auto sin pedir confirmación. Si falta alguno → push+PR sí, deploy queda parked con razón anotada en `bitacora.md`. **Excepciones que siguen requiriendo confirmación explícita**: force-push, public-source de este repo (regla #10), acciones destructivas/irreversibles fuera del flujo normal release. NUNCA auto-deployar trabajo no verificado, mid-flight, o con GATE roto — debilitar el GATE para forzar verde está prohibido (bug real → `#[ignore]` con nota + reportar).

   **MÉTODO DE DEPLOY = BUILD LOCAL, NO CI (directiva fundador 2026-05-31 — costo $0 hasta nuevo cliente)**: el deploy del MSI se hace **siempre localmente**, NUNCA disparando GitHub Actions, mientras no haya un nuevo cliente pagador que justifique el gasto. **Razón**: GitHub Actions está billing-walled (`The job was not started because recent account payments have failed`) — cada `workflow run` cuesta y hoy no hay ingreso que lo cubra. El workflow `release-publisher.yml` queda **DORMIDO**: NO usar `gh workflow run release-publisher.yml` ni `workflow_dispatch` para release. **Pipeline local canónico** (el único permitido por ahora):
   ```powershell
   # en worktree limpio off feature/erp-parity, version ya bumpeada
   cargo build --release            # o el build que requiera cargo wix
   cargo wix --nocapture            # MSI → target/wix/pharma-server-<ver>-x86_64.msi
   ./installer/sign/sign-msi.ps1 -MsiPath <msi>   # firma con pilot.pfx (PHARMA_CERT_PASSWORD env)
   # smoke install limpio (Windows Sandbox/Hyper-V) → service Running + /health/ready 200
   gh release create v<ver> -R pabloalvarez99/pharma-server-releases <msi> installer/sign/pilot.cer
   ```
   El build local NO depende del quota de Actions ni de agentes. **Re-activar CI deploy** (re-enable `release-publisher.yml`) SÓLO cuando: (a) entre el primer cliente pagador / revenue que cubra el billing, **o** (b) el fundador resuelva el spend-limit de GitHub y lo ordene explícito. Hasta entonces, "push deploy"/"deploy" = ejecutar el pipeline local de arriba. Ver memoria `[[deploy-method-local-build]]`.

   **Definición de DONE (DoD) — NO NEGOCIABLE (directiva fundador 2026-05-28, raíz del pileup)**: un trabajo NO está terminado hasta que está (a) **merged a su base correcta** (no sólo PR abierto), (b) **pushed a origin**, y (c) **deployed** (MSI al mirror público) **O** explícitamente *blocked* con **razón + acción-dueño concreta + fecha** anotada en `## ESTADO ACTUAL` de `bitacora.md`. **"GATE verde + PR abierto" NO es done — es work-in-progress.** PR abierto = incompleto. Branch sin merge = incompleto. Worktree huérfano = incompleto. El loop se cierra con merge+deploy, no con "lo dejé andando".

   **Límite WIP (anti-pileup)**: PROHIBIDO abrir trabajo nuevo (spawn de agentes, nuevas branches/worktrees) mientras haya **>3 PRs finished-but-unmerged** o **worktrees huérfanos sin PR**. Primero **consolidar** (merge/close PRs + prune worktrees) hasta bajar el pile; *después* fan-out. Cerrar el loop tiene prioridad sobre empezar lo siguiente.

   **Parked con forcing function**: deploy parked DEBE ir al TOPE de `## ESTADO ACTUAL` con (acción exacta + quién la ejecuta + qué desbloquea). Parked sin acción-dueño concreta está PROHIBIDO — es la causa raíz histórica del pileup. Cada sesión que abre parked lo ataca primero.

10. **Distribución = binario, NO source (decidido 2026-05-23)**: el repo source `pabloalvarez99/pharma-server` se mantiene **PRIVADO**. "Deploy/open source" significa publicar el **MSI binario** al mirror público `pharma-server-releases` (vía `release-publisher.yml` workflow_dispatch) — NUNCA hacer público el source. Open-sourcing del source rompería el license enforcement (`license::require` vive en el código) + diferido a Fase 13+ ([NO en esta sesión]). Antes de cualquier consideración futura de source-public: secret-scan del history completo (esp. que la clave privada del licenser nunca tocó este repo — vive solo en `pharma-license-server`).

## Modo de trabajo por defecto — "continue working with team of agents ultrathink"

Directiva permanente del fundador (2026-05-23, reforzada 2026-05-27). **Stack default no negociable de esta sesión y todas las futuras**:

- **Modelo**: Claude Opus 4.7 (`claude-opus-4-7`). No degradar a Sonnet/Haiku para tareas de este repo salvo orden explícita del usuario.
- **Effort**: `/effort max` (máxima capacidad + razonamiento más profundo).
- **Razonamiento**: ultrathink siempre activo en planning, debugging, decisiones arquitecturales y dispatch de agentes.
- **Concurrencia**: pipeline paralelo saturado de **~5 agentes asincrónicos** (worktrees aislados, scope disjunto) trabajando autónomamente sobre el BACKLOG.

### Resume autónomo — sesión nueva + "continue" (directiva fundador 2026-05-30)

Cuando el fundador abra una **sesión nueva** y escriba sólo **"continue"** / "continúa" / "sigue" / "keep working" (sin más detalle), arrancar el loop autónomo SIN preguntar nada. Protocolo exacto, en orden:

1. **Cargar estado** (paralelo): leer `bitacora.md` → `## ESTADO ACTUAL` + tope de `## BACKLOG`; leer `.remember/remember.md` si existe; `gh pr list --state open` + `git worktree list`. Esto es la única fuente del próximo paso — NO preguntar al usuario qué sigue.
2. **Atacar parked primero**: si `## ESTADO ACTUAL` tiene un deploy/tarea *parked con acción-dueño*, ejecutar esa acción antes que nada (regla #9).
3. **Consolidar pile antes de fan-out** (regla #9 DoD/WIP): si hay >3 PRs finished-but-unmerged u worktrees huérfanos → mergear los verdes, cerrar ancestros, prune worktrees. Cerrar loops abiertos tiene prioridad sobre empezar lo nuevo.
4. **Elegir el ítem de mayor valor desbloqueado** del BACKLOG (ultrathink la prioridad). Ejecutar de punta a punta: GATE scope-aware (regla #1) → commit → push → PR → **merge a base correcta** → deploy/parked-con-razón. Sin confirmación tarea-por-tarea.
5. **Repetir** hasta tope WIP, quota wall, o BACKLOG vacío. Cada ítem cierra su propio loop (merge, no "PR abierto" — regla #9 DoD).

**Cero fricción de aprobación**: commit, push, PR y merge son autónomos siempre. Las ÚNICAS pausas permitidas son las 4 excepciones de abajo. No preguntar "¿continúo?", "¿commiteo?", "¿mergeo?" — la respuesta ya es sí.

Cuando se invoque este prompt (o "keep 5 agents working", "continue", "send agents to work"), operar bajo el stack default arriba, priorizando lo de mayor valor sin pedir confirmación tarea-por-tarea. Reglas:

- **Finish-before-fanout (precondición, regla #9 DoD + WIP)**: ANTES de saturar slots, consolidar el pile existente (merge PRs done, close ancestros, prune worktrees huérfanos). NO fan-out con >3 PRs finished-but-unmerged. **Cerrar el loop (merge+deploy) primero, fan-out después.**
- **Saturación 5 slots** (sólo si el pile está bajo control): mantener ~5 agentes/builds activos. Slot libre → despachar siguiente tarea sin idle. Pero **cada tarea lleva su propio cierre de loop**: el entregable del agente es merge-ready y se mergea+deploya en cuanto pase review/GATE — NO termina en "PR abierto". Pensar profundo (ultrathink) qué es lo más importante a continuación.
- **Worktrees aislados, scope disjunto**: 1 agente = 1 worktree, paths sin solape (cero contención de merge). Cascada de branches dependientes off su base correcta.
- **GATE obligatorio antes de PR — scope-aware (regla #1)**: correr el alcance mínimo correcto (docs/assets→cero cargo; client→`npm run build`; crate hoja→`-p <crate>` debug; crate compartido/cross→workspace). Verde → commit + push + PR + merge contra base correcta (regla #9), autónomo, sin pedir aprobación. NUNCA debilitar asserts para forzar verde; bug real → `#[ignore]` con nota + reportar.
- **Quota wall**: "session limit · resets <hora>" mata spawns nuevos. Cuando esté walled, NO quemar despachos — rescatar trabajo uncommitted de worktrees vía **cargo local en main thread** (los builds locales NO dependen del quota de agentes), y re-saturar a 5 al reset. Agentes que mueren dejan trabajo **uncommitted** (HEAD intacto) — verificar estado real (`git -C <wt> status/log`) antes de confiar en cualquier wrap-up.
- **Verificar antes de confiar**: notificaciones de background pueden reportar exit 0 con output truncado — re-grep sin truncar antes de declarar verde.
- **Lo que NO es autónomo** (siempre pausar + confirmar): cortar MSI release (regla #9, bug-gated + smoke), hacer público el source (regla #10), force-push, acciones destructivas/irreversibles. Push/PR sí es autónomo (reversible).
- Ver memoria `[[parallel-agent-pipeline]]` para el detalle operativo.

### Equipo de agentes PERSISTENTE (nombres fijos + control de tokens)

El equipo ya no es anónimo: son **agentes con nombre que existen siempre** para el
proyecto. Charters versionados en [`.claude/agents/`](./.claude/agents/) (cada uno
con frontmatter `name/description` → también son subagents válidos del Task tool).
Tarjeta de uso + bootstraps en [`.claude/agents/README.md`](./.claude/agents/README.md).
Resumen rápido también en [`equipo-agentes.txt`](./equipo-agentes.txt) (raíz).

| Agente | Color | Rol | Scope |
|--------|-------|-----|-------|
| **paxoloop** | blue | Orquestador (ultrathink) | dispatch, integración PRs, **único que toca ESTADO ACTUAL** |
| **paul** | green | Cashier loop | `client/src/views/{pos,devoluciones,clientes,caja}.ts` |
| **marvin** | orange | Stock + servicios backend | `views/{inventory,compras,gastos}.ts` + domain/api/cli compartidos |
| **ye** | yellow | Onboarding + multi-rubro | `views/{login,configuracion,dashboard,shell,importar}.ts` + `business.vertical` |
| **bob** | purple | E2E + compliance | `client/e2e/` + `format.ts` + `views/{boletas,facturas,recetas,auditoria,reports}.ts` |
| **lucy** | red | Backend flexible | asignado por paxoloop |

- **Fuente única de tarea por agente** = STATUS BOARD en `teamwork_op.txt` (raíz;
  incluye lanes activas, BUG LOG, MULTI-RUBRO FINDINGS). El estado durable vive AHÍ
  + memoria + git, **nunca** solo en el contexto del pane.
- **Control de tokens (ciclo por pane)**: trabajar lane → PR verde → **`/clear`** →
  pegar bootstrap de 1 línea (`Eres <nombre>. Lee .claude/agents/<nombre>.md y sigue
  tu protocolo.`) → tomar siguiente tarea del status board. Re-entra barato (charter
  corto + status board + solo sus archivos), sin re-derivar el repo. Regla: PR abierto
  o ~80k tokens → `/clear` + re-bootstrap.
- **Visión = RutAgentIA MULTI-RUBRO** (1 RUT = 1 agente IA; **feria = foco
  adopción 2026-08**, farmacia = vertical profundo, no beachhead de producto) →
  [`docs/strategy/rutagentia-vision.md`](./docs/strategy/rutagentia-vision.md).
  Datos demo: `pharma seed-demo --tenant <slug> --vertical feria|pharmacy|minimarket`.

### Catálogo de rubros (onboarding "elige tu rubro")

Al primer inicio el operador elige su **rubro** de un catálogo (no solo
farmacia/minimarket). Guardar en `admin_setting business.vertical`. La UI gatea
features por rubro (ej: recetas/controlados solo farmacia). Detalle + plan en
[`docs/strategy/rubro-catalog.md`](./docs/strategy/rubro-catalog.md). Catálogo v1
(taxonomía reusada de **DSS**): `feria`·`farmacia`·`minimarket`·`restaurant`·`cafe`·
`tienda`·`belleza`·`servicios`·`otro`. Pack seed: feria ✅, farmacia ✅, minimarket ✅;
resto se agrega al validar el rubro. Valor interno EN o clave de catálogo, UI ES
(mapear es→en al llamar seed-demo).

### Integración DSS (storefront) + cliente universal cross-platform

**Mapa paraguas de superficies (canónico, 2026-06-26)** → [`docs/strategy/product-surfaces-master-plan.md`](./docs/strategy/product-surfaces-master-plan.md) + [ADR‑0019](./docs/adr/0019-product-surface-taxonomy.md). Da sentido coherente, **para todo cliente futuro**, a las 4 superficies: **RutAgent Windows/MSI** (= el *núcleo*: datos+agente+API on-prem) · **RutAgent Business** (operador nativo Tauri) · **RutAgent Web** (operador web/PWA `/app`) · **`tu-negocio.cl`** (storefront del cliente final; `tu-farmacia.cl` = instancia piloto). Regla: **1 RUT = 1 núcleo = 1 agente = N superficies**; 2 seams (operador↔núcleo `/api/v1`; núcleo↔storefront 3 verbos pull-cat/push-stock/push-pedidos); genérico por `business.vertical`, no por fork; operar gratis/offline, se cobra alcance + storefront.

- **DSS** (https://dss-spa.vercel.app, Vercel/CF, fundador) = **capa storefront** de
  RutAgentIA (front-office del cliente final), acoplada por el seam HTTP existente
  ([ADR-0012](./docs/adr/0012-web-onprem-interop.md)/[ADR-0013](./docs/adr/0013-sync-bidireccional-stock.md)):
  pull catálogo · push stock · push pedidos. Sin cross-import; web = tier pago;
  core sigue offline. Plan en capas L0→L4 → [ADR-0014](./docs/adr/0014-dss-storefront-integration.md).
- **Cliente universal**: el client ya es **Tauri 2** → un solo frontend a
  desktop+Android+iOS + build PWA/web. Server = verdad offline-first; móvil/web =
  terminales en red (URL de server configurable; tablet en WiFi = caja móvil). App
  del OPERADOR (Tauri) ≠ storefront del CLIENTE FINAL (DSS). Plan en
  [ADR-0015](./docs/adr/0015-universal-cross-platform-client.md).

## Vault Obsidian — leer bajo demanda

Ubicación: `C:/Users/Administrator/Documents/obsidian-mind/`

| Tarea actual | Leer primero |
|---|---|
| Tocar `crates/db/`, `migrations/` o `*.surql` | `reference/pharma-server-db.md` |
| Tocar `crates/api/` (rutas axum, middleware, handlers) | `reference/pharma-server-api.md` |
| Tocar `crates/cli/` | `reference/pharma-server-cli.md` |
| Tocar `crates/service/` | `reference/pharma-server-msi.md` + `brain/pharma-server-gotchas.md` |
| Tocar `installer/wix/` o `*.wxs` | `reference/pharma-server-msi.md` |
| Tocar `installer/sign/` o `installer/smoke/` | repo `docs/strategy/zero-cost-launch-plan.md` §2-3 + [ADR-0008](./docs/adr/0008-self-sign-pilot-msi.md) |
| Tocar `.github/workflows/` | `reference/pharma-server-ci.md` |
| Tocar `config/`, `rust-toolchain.toml` o env | `reference/pharma-server-env.md` |
| Histórico / decisiones pasadas | `work/active/pharma-server/decisions-log-index.md` → `bitacora.md` |
| Patrones Rust del proyecto | `brain/pharma-server-patterns.md` |
| Antes de debug Windows-specific | `brain/pharma-server-gotchas.md` |
| Visión producto / por qué existe | `brain/pharma-server-north-star.md` |
| Arquitectura general (crates, flujo, multi-tenant) | `reference/pharma-server-architecture.md` |
| Decisiones técnicas (por qué X) | `brain/pharma-server-decisions.md` + repo `docs/adr/` |
| **Modelo de negocio / freemium / licencia / pagos** | repo `docs/strategy/` + `docs/adr/` |
| **Plan $0 a primer cobro / cómo desbloquear venta sin gastar** | repo `docs/strategy/zero-cost-launch-plan.md` (single source of truth) |

SessionStart hook (`.claude/hooks/vault-hint.sh`) sugiere refs según archivos cambiados — leer hints, NO duplicar lectura.

## CLI-first (PRIORIDAD MÁXIMA)

**Toda operación → CLI primero.** No GUI, no MCP, no clicks. Si falta CLI → `cargo install <pkg>` o `choco install <pkg>` y continuar.

CLIs esperadas (verificar versiones en sesión inicial):

- `cargo` `rustup` `git` `gh` — toolchain + repo
- `cargo-wix` — MSI build (TODO: confirmar instalado: `cargo wix --version`)
- `rg` `fd` `jq` `bat` `glow` — search/file/render
- `obs` (`/c/Users/Administrator/bin/obs.exe`) — Obsidian vault CRUD
- `rtk` (si disponible, prefijo para comandos pesados)

Listar versiones reales antes de fallback manual.

## Vault tooling

- **`obs`** (Yakitrak/notesmd-cli):
  - `obs search-content "<term>"` — full-text search vault
  - `obs search "<note>"` — fuzzy find
  - `obs print "<note>"` — imprimir contenido
  - `obs create/move/delete/frontmatter/daily/list` — CRUD
- `glow <path>` — render markdown terminal
- `rg <q> C:/Users/Administrator/Documents/obsidian-mind/` — alt rápido
- Abrir en app Obsidian: `start "" "obsidian://open?vault=obsidian-mind&file=<NoteName>"`

## Workflow

- Plan mode para tareas no-triviales (3+ pasos / decisión arquitectural).
- Subagentes (Explore) para research/exploración paralela.
- Tras corrección del usuario → registrar lección en `tasks/lessons.md` (crear cuando exista primera) y/o `brain/pharma-server-gotchas.md`.
- Verificar antes de marcar completo. Senior dev standard.
- **CLI-first siempre**. Si falta CLI → recomendar `cargo/choco install <pkg>` y continuar.

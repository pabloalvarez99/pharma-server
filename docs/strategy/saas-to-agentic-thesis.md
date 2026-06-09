# SaaS → Agentic Company — tesis de primeros principios (horizonte 2026-2035)

**Estado**: tesis registrada 2026-06-09 (directiva fundador: "create your own better plan idea" sobre seed-prompt SaaS→Agentic). Complementa [`agentic-business-platform.md`](./agentic-business-platform.md) (la arquitectura RutAgentIA) con el *por qué* económico-estratégico y el plan de transición auto-financiado.
**No es un roadmap ejecutable** — es el marco para decidir qué construir, qué no, y en qué orden. El roadmap sigue siendo Fases 9-15.

---

## 1. Por qué existió el SaaS (y qué era en realidad)

El computador podía **almacenar, transmitir y calcular** — pero no podía **decidir**. La inteligencia del negocio vivía exclusivamente en humanos. Entonces toda la industria del software empresarial resolvió un único problema de impedancia: *cómo conectar la cognición humana con los datos*.

- **Formularios** = la API de entrada para humanos.
- **Dashboards y reportes** = la API de salida para humanos.
- **Menús y workflows** = el control de flujo, ejecutado a mano.
- **CRUD** = el único verbo que el software sabía hacer solo.

Nada de eso era valor de negocio. Era **andamiaje compensatorio por falta de inteligencia**: el humano era el motor de inferencia, y el SaaS le arrendaba las pantallas y la base de datos donde ejecutar esa inferencia. El modelo de precios lo delata — **per seat** = se cobra por humano-operador, es decir, por unidad de inteligencia conectada al sistema. El moat del SaaS era doble: lock-in de workflow (entrenaste a tu gente en mis pantallas) y gravedad de datos (tu historia vive en mi base).

**Corolario que casi nadie acepta**: si la inferencia se vuelve barata y confiable, el 80% de la superficie visible del SaaS — las pantallas — pierde su razón de existir. Lo que NO pierde su razón de existir es el otro 20%, que siempre fue el verdadero producto: **el ledger transaccional íntegro, auditado, con identidad y permisos**.

## 2. La inversión de supuesto

```
Supuesto SaaS:        Humanos usan software.
Supuesto agéntico:    Software usa software. Los humanos sólo definen objetivos
                      y responden por las consecuencias.
```

Bajo mejora sostenida de modelos frontier (razonamiento, tool-use, memoria, coordinación multi-agente, costo de inferencia cayendo ~10x/2años):

| Categoría | Destino |
|---|---|
| Formularios, dashboards, menús, capacitación de usuarios | **Desaparecen** (quedan vistas de excepción y confianza) |
| Modelos LLM, frameworks de agentes, chatbots, CRUD genérico | **Commodity** — los frontier labs lo regalan; competir ahí es morir |
| Ledger transaccional, identidad/firma, rails de pago, rails regulatorios (SII/ISP), audit | **Infraestructura** — lo que los agentes *necesitan* para actuar |
| Confianza + responsabilidad legal, datos operacionales propios, red de agentes transando, distribución/relación con el cliente | **Defensible** — el moat real de la década |

La paradoja central: **los agentes necesitan el sistema de registro MÁS que los humanos, no menos**. Un humano tolera ambigüedad ("después cuadro la caja"); un agente autónomo necesita estado consistente, transacciones atómicas, idempotencia, atribución firmada de cada acción y un log inmutable contra el cual verificarse. La era agéntica no mata el ERP — mata la *interfaz* del ERP y **promueve su núcleo a infraestructura crítica**.

## 3. Las cuatro etapas (y dónde se captura valor en cada una)

```
Software-as-Tool ──▶ Software-as-Worker ──▶ Software-as-Team ──▶ Software-as-Company
   (2024-26)            (2026-29)              (2029-32)             (2032-35+)
```

1. **Tool** — el humano opera, la IA asiste (copilots, chat-in-app). Valor capturado por quien ya tiene distribución. *Trampa de etapa: creer que "poner un chat en la app" es la transición. Es sólo el peaje de entrada.*
2. **Worker** — un agente es dueño de UNA función de punta a punta (reposición de stock, cobranza, cumplimiento) con KPI medible; el humano revisa excepciones. Valor capturado por quien demuestre **ROI por función**: "este agente te ahorra X horas / $Y al mes". Aquí cambia el pricing: de seat → **por resultado o por función delegada**.
3. **Team** — orquestador + coordinadores cruzan funciones para perseguir objetivos ("sube el margen 2pp"). El humano define metas y aprueba irreversibles. Valor capturado por quien tenga el **substrato de coordinación**: identidad, firma, audit, protocolo entre agentes. (= exactamente Fase 15 de RutAgentIA.)
4. **Company** — la organización ES agentes + pocos humanos que aportan lo no-delegable: capital, juicio, responsabilidad legal, relaciones. El software deja de ser algo que *usas*; es algo que *empleas*. El "ERP" es invisible: es el sistema nervioso.

**Segunda derivada (la que casi todos pierden)**: cuando las empresas son agénticas, el volumen de transacciones **entre** empresas lo generan agentes negociando con agentes. El que posea los *rails* de ese comercio agente-a-agente (identidad verificable + envelopes firmados + reputación) está en la posición de Visa en 1975. Eso es Fase 13 (marketplace federado) visto desde 2035 — y es la apuesta más asimétrica de todo el plan.

## 4. Qué quieren los usuarios (nadie quiere software)

Nadie quiso nunca un ERP. El dueño de farmacia quiere: *no quebrar stock, no perder plata, no tener problemas con el SII/ISP, irse a la casa temprano*. El software fue siempre el costo que pagaba por esos outcomes.

- **Producto futuro**: outcomes garantizados con SLA. "Tus crónicos nunca quiebran stock." "Cumples SII sin pensarlo." "Tu margen no baja de X sin que lo sepas hoy."
- **Interfaz futura**: conversación (declarar objetivos) + **bandeja de excepciones** (aprobar irreversibles, resolver ambigüedad) + vistas de confianza (ver qué hizo el agente y por qué — el audit log como UI). Las pantallas que sobreviven son las que producen *confianza*, no las que producen *operación*.
- **Modelo de negocio futuro**: freemium se sostiene — el ledger/core gratis (ya es invariante ADR-0005), los **agentes son el tier pago** (por función Worker, luego por equipo). Microtx encaja: "contrata el agente de compras", "contrata el agente SII". A largo plazo: participación en el outcome (% del ahorro generado) — pero eso exige medición confiable que sólo da... el ledger propio.
- **Moat futuro**: (1) **confianza + responsabilidad** — alguien tiene que responder cuando el agente se equivoca, y un proveedor local con cara, contrato y datos en la farmacia gana contra una API anónima; (2) **datos operacionales** que nadie más tiene (ventas reales, mermas, vencimientos de miles de farmacias independientes); (3) **red** de agentes transando (compra colectiva, B2B); (4) los **rails regulatorios** locales (DTE nativo, libro de controlados) que un player global no va a construir para Chile. Nota: es el MISMO moat de 4 capas de `market-thesis.md` — la era agéntica no lo cambia, lo *amplifica*.

**Ganadores y perdedores**: pierden los SaaS cuyo valor era la pantalla y el workflow (la capa media completa). Ganan: frontier labs (commodity arriba), dueños de rails e infraestructura (abajo), y dueños de la **relación + responsabilidad** (al frente del cliente). La posición de pharma-server/RutAgentIA es deliberadamente **abajo + al frente**: rails locales + relación con el independiente. Nunca competir en la capa media (frameworks, chatbots) ni arriba (modelos).

## 5. La farmacia LATAM 2035 (end-state como organización agéntica)

- **Operación diaria**: el local abre; nadie "abre el sistema". El agente de la farmacia ya cuadró caja, ya repuso, ya reportó. El QF atiende pacientes — su día es clínico, no administrativo.
- **Inventario**: se gestiona solo — FEFO, vencimientos negociados con proveedores *antes* de vencer (devolución/liquidación automática), mínimos dinámicos por estacionalidad y epidemiología local (el agente ve que viene el invierno y los datos agregados de la red).
- **Compras**: el agente de la farmacia cotiza simultáneamente con los agentes de N droguerías (la federación quote/PO de hoy, masificada), arma la compra colectiva con otras farmacias de la red para lograr precio de cadena, y el humano aprueba una excepción al mes.
- **Pricing**: dinámico dentro de bandas regulatorias y éticas que fija el dueño una vez ("nunca subas crónicos más de X%").
- **Compliance**: continuo, no batch — cada venta de controlado queda en el libro firmada en el momento; el "reporte ISP" deja de existir como tarea porque el estado siempre está reportable. DTE por venta, libro mensual auto-presentado.
- **Atención al cliente**: el agente RUT del *paciente* (RutAgentIA B2C) pide el refill al agente de la farmacia; adherencia monitoreada; interacciones medicamentosas chequeadas contra TODO lo que el paciente compra (no sólo en esta farmacia). El QF interviene en lo clínico-humano.
- **Forecast**: deja de ser un reporte que alguien mira — es el insumo interno de los agentes de compra/stock.
- **La organización**: dueño = capital + objetivos + responsable final; QF = juicio clínico + responsabilidad sanitaria (los "humanos no-delegables"); todo lo demás, agentes. Una farmacia independiente opera con la sofisticación logística de Cruz Verde — **ese es el punto**: la era agéntica es la primera tecnología que des-economiza la escala del oligopolio. La ventaja de la cadena era amortizar gerentes/analistas/sistemas sobre 400 locales; cuando eso lo hace un agente, la ventaja se evapora.

## 6. El plan (mi versión, mejor que el seed-prompt): transición auto-financiada

El error fatal sería construir 2035 en 2026 y morir sin revenue. La disciplina: **cada etapa se paga sola y deja construido el substrato de la siguiente**.

### 6.1 Qué construir primero (orden estricto)

1. **HOY (Fases 9-11, sin cambios)**: vender el ERP pharma. Es la etapa Tool y financia todo. *Reframe interno*: cada decisión técnica se evalúa también como "¿sirve al substrato agéntico?" — atomicidad, audit, idempotencia, API estricta ya puntúan doble.
2. **Substrato de atribución** (barato, pre-Fase 15): `agent_id` distinguible de `user_id` en audit log + service accounts con scopes. Sin esto no hay Worker confiable.
3. **Tool surface MCP** (15a): exponer `/api/v1` como tools con descripciones operacionales. Costo bajo, opcionalidad enorme — convierte el ERP en empleable por cualquier agente del ecosistema (Claude, etc.) sin que construyamos el agente nosotros.
4. **UN agente Worker con ROI medible** (15b reformulado): el **agente de reposición/compras** — loop cerrado (stock → forecast → cotización federada → OC → recepción), alto valor, bajo riesgo clínico, y ya existen todos los rails (FEFO, PO, WAC, federación quote/PO). KPI: quiebres de stock evitados, horas ahorradas. Se vende como tier/microtx. **No** empezar por el agente "conversacional general" — empezar por el que ahorra plata medible.
5. **Bandeja de excepciones** como vista nueva del cliente: la UI de la era Worker. Aprobar/rechazar/explicar. (Las vistas ERP actuales quedan como vistas de confianza/fallback — no construir más allá de eso.)
6. **Sólo después**: orquestador multi-dominio (15c), packs (15d), B2C RUT-agent.

### 6.2 Qué NO construir nunca

- **Framework genérico de agentes** — commodity de los frontier labs; usar el suyo (Agent SDK/MCP), construir sólo lo específico: tools, audit, rails.
- **Más dashboards/reportes** más allá de excepciones + confianza — es expandir la capa que muere.
- **Chatbot-skin sobre el ERP** ("pregúntale a tus datos") — demo bonita, ROI cero, posiciona el producto en la capa commodity.
- **Modelo propio / fine-tuning prematuro** — competir arriba es suicidio de capital.
- **B2C antes que el B2B esté probado** — el agente del paciente toca salud+plata de personas naturales: máxima sensibilidad (Ley 19.628), confianza que aún no se ganó, y cero revenue hoy.

### 6.3 Supuestos que la mayoría de los founders tiene mal

1. *"Agregar IA a mi SaaS me protege"* — falso: si el valor era la pantalla, el copiloto sólo acelera la migración fuera de la pantalla.
2. *"Los agentes reemplazan el sistema de registro"* — falso: lo necesitan más (§2). Quien posee el ledger posee el suelo donde trabajan los agentes.
3. *"El moat es el modelo"* — falso para todos salvo 3 empresas en el mundo: el moat es confianza, datos, red y rails locales.
4. *"La UI es el producto"* — la UI era el síntoma de la falta de inteligencia; el producto siempre fue el outcome.
5. *"Esto es para 2030, hay tiempo"* — la ventana de posicionamiento (quedarse con los rails y la relación) se cierra ANTES de que la tecnología madure; los rails se construyen lentos.

### 6.4 Oportunidades escondidas hoy (asimetrías)

- **Rails de comercio agente-a-agente para PyMEs LATAM** con RUT como identidad (§3, segunda derivada) — nadie lo está construyendo para este mercado; nosotros ya tenemos envelopes firmados + federación operando.
- **La economía de la excepción**: cuando los agentes operan todo, el producto premium es el manejo de la excepción (juicio, garantía, seguro). Posicionarse como quien la administra.
- **Compliance-as-API local**: DTE nativo en Rust + libro controlados = activos que un player global no replica para Chile; venderlos como rail, no como feature.
- **Datos agregados de la red de independientes** (opt-in, ADR-0005): el único dataset de demanda farmacéutica independiente de LATAM — valioso para la red misma (forecast colectivo, poder de compra), no para venderlo a terceros.

### 6.5 Si esta empresa se fundara en 2035

No vendería software: operaría farmacias de terceros a cambio de un % del margen mejorado — una **management company agéntica** donde el dueño pone local, capital y QF, y la plataforma pone toda la operación. Cada feature de hoy se evalúa contra esa foto: *¿esto me acerca a poder operar la farmacia entera, o es una pantalla más?*

## 7. Auto-crítica (desafiar las propias conclusiones)

- **¿Y si la adopción es mucho más lenta?** (regulación, costo, desconfianza) — el plan no muere: el ERP freemium se sostiene solo en etapa Tool indefinidamente. La apuesta agéntica es opcionalidad encima de un negocio viable, no un all-in.
- **¿Y si los frontier labs bajan a la capa de aplicación?** — bajarán a lo horizontal (oficina, código). Una farmacia de Coquimbo con libro de controlados ISP y DTE del SII es exactamente el último lugar al que llegan. Los rails locales + on-prem + responsabilidad legal local son el escudo.
- **¿Y si el on-prem/offline-first estorba la era agéntica?** — al revés: "tu agente corre en TU nodo con TUS datos" es la única respuesta creíble al problema de confianza de §4. El diferenciador de hoy (offline, sin lock-in) se convierte en el argumento de soberanía del agente mañana.
- **Riesgo real número 1**: secuenciación — gastar el foco de 2026 en la visión de 2032. Mitigación: este doc prohíbe explícitamente construir Fase 15 antes de revenue (§6.1 punto 1 manda).

## Referencias

- [`agentic-business-platform.md`](./agentic-business-platform.md) — la arquitectura RutAgentIA (el *cómo*; este doc es el *por qué* y el *cuándo*).
- [`market-thesis.md`](./market-thesis.md) — moat 4 capas (este doc lo extiende a la década agéntica).
- [`latam-master-plan.md`](./latam-master-plan.md) — tesis 2026-2035 (este doc profundiza el pilar AI-native).
- [`b2b-marketplace.md`](./b2b-marketplace.md) — Fase 13, releída como rails agente-a-agente (§3).
- [ADR-0005](../adr/0005-core-gratis-no-locked-in.md) — invariantes = la base de confianza de todo §4-§5.

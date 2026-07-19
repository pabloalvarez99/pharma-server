# Intuitive & Friendly UX Master Plan — objetivo fundamental

> **Directiva fundador (2026-06-21):** "app que sea muy intuitiva y amigable con el
> cliente, interfaz muy intuitivo" → elevado a **objetivo fundamental** del proyecto
> (codificado en [`CLAUDE.md`](../../CLAUDE.md) § OBJETIVO FUNDAMENTAL — UX). NO es una
> ola: es vara permanente. Este doc detalla los principios + los vectores a despachar.

---

## 0. Tesis

El usuario real NO es técnico: dueño de almacén/farmacia/peluquería, a veces mayor, a
veces operando una tablet táctil en el mostrador, sin tiempo ni ganas de leer un manual.
Si tiene que pensar, ya perdimos. El programa debe sentirse **obvio, humano y rápido** —
el equivalente RutBusiness a "lo prendí y ya estaba vendiendo". Esto va más allá del
config center (W5/W6): es la calidad de CADA pantalla, para siempre.

## 1. Los 8 principios (no-negociables, parte del DoD de toda lane de UI)

1. **Tarea diaria de cero fricción.** El 80% del uso = cobrar, fiar, "¿cuánto vendí?",
   reponer, arquear. Esas deben ser obvias, de pocas teclas, y a prueba de error.
2. **Habla como el dueño.** es-CL humano. Cero jerga, cero keys técnicas (`admin_setting`,
   `tipo:39` → "Boleta"). Cada error dice **qué hacer** ("Falta el RUT del cliente para
   fiar"), no un código.
3. **Guía, no manual.** Primer-uso guiado (coach-marks), empty-states que enseñan con un
   CTA, tooltips, y el agente como ayuda en lenguaje natural ("¿cómo hago una devolución?").
4. **Perdona errores.** Confirmación clara en lo destructivo, **undo** donde se pueda,
   validación inline amable (explica + sugiere, no solo bloquea en rojo).
5. **Rápido y sin fricción.** Keyboard-first + command palette (Ctrl+K, hecho), feedback
   <100ms (POS <50ms p99), defaults inteligentes, cero clicks de más, foco donde toca.
6. **Consistente.** Un solo lenguaje visual: el design system `client/src/views/ui.ts`
   (.ui-* botones/cards/empty/loading/error) + tokens en brand css. Nada "de dev".
7. **Accesible + táctil.** Contraste AA, navegación 100% por teclado, tipografía legible
   (cajero mayor), targets grandes + modo touch para POS en tablet.
8. **Confianza.** Toast/feedback de cada acción, estados loading/empty/error **producidos**
   en toda vista, NUNCA una pantalla en blanco ni un spinner infinito.

## 2. Cómo se opera (permanente)

Toda lane que toca UI honra esto en su DoD, no como extra. El GATE de craft (antes solo
para la vitrina rubro, `rubro-select-experience.md` §9) ahora aplica a TODA vista:
copy es-CL humano · estados `ui.*` · camino completo por teclado · confirm/undo en
destructivo · ayuda/empty-state que enseña · cero jerga. paxoloop lo revisa en integración.

## 3. Vectores de mejora a despachar (UX-wave, off el tip post-W6)

| # | Vector | Owner | Qué |
|---|--------|-------|-----|
| U1 | **Primer-uso guiado completo** | ye | bienvenida → elegí rubro → tu negocio → 1er producto → 1ra venta, con coach-marks; self-onboard en minutos, cero confusión |
| U2 | **Ayuda contextual + agente-guía** | ye / milton | `?` en cada vista, tips inline, y el agente responde "¿cómo hago X?" (intents de ayuda) |
| U3 | **Auditoría de copy + errores amables** | bob | barrer TODO mensaje/validación: es-CL, dice qué hacer, sin jerga/códigos; toasts consistentes; confirm/undo en destructivo |
| U4 | **Flujos diarios pulidos** | paul | cobrar/fiar/arqueo/reponer: medir clicks/teclas y quitar fricción; defaults inteligentes; foco/atajos |
| U5 | **Accesibilidad + táctil + legibilidad** | paul / ye | contraste AA, teclado total, tipografía legible, **modo touch** para POS en tablet |

(El design system base `ui.ts` ya existe (W5/W6) — estos vectores lo *aplican* y profundizan.)

## 4. Definición de hecho

- [ ] Un dueño no-técnico instala y vende **sin que nadie le explique**.
- [ ] Cero jerga / cero keys técnicas / cero pantallas "de dev" en toda la app.
- [ ] Todo mensaje de error dice **qué hacer**; lo destructivo pide confirmación.
- [ ] Operable 100% por teclado + usable en tablet táctil; contraste AA.
- [ ] Cada vista tiene estados loading/empty/error producidos (cero pantalla en blanco).
- [ ] Sensación: "esto es facilísimo, lo entendí solo".

> Norte: el mejor ERP del mundo no sirve si el almacenero no lo entiende. La intuitividad
> NO es pulido opcional — es tan fundamental como offline-first y multi-rubro.

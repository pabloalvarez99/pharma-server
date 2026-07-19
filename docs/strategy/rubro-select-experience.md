# Rubro Select Experience — ULTRA-PLAN (la vitrina de RutBusiness multi-rubro)

> **Directiva fundador (2026-06-16):** enfocarnos en mejorar ESTA pantalla — la
> selección de rubro del onboarding — y dejarla **muy profesional, muy producida,
> detallada, profunda**. Es la primera prueba tangible de la promesa RutBusiness
> ("un ERP, todos los rubros"). Cuando un dueño la ve, debe pensar *"esto fue hecho
> para MI negocio"*. Doc dueño: **ye** (configuración/onboarding). Integra: paxoloop.
>
> Relacionado: [`rubro-catalog.md`](./rubro-catalog.md) (taxonomía v1) ·
> [`rutagentia-vision.md`](./rutagentia-vision.md) (norte multi-rubro) ·
> memoria `[[rubro-select-showcase]]`.

---

## 0. Por qué esto es primordial

El producto entero es "un ERP que se adapta a cualquier rubro chileno por su RUT". El
**único momento** donde esa promesa se vuelve visible y emocional es esta pantalla:
el dueño elige su rubro y el sistema **se transforma frente a él**. Es el equivalente
RutBusiness al onboarding de Stripe/Linear/Notion: barato de hacer mal, decisivo
hecho bien. Hoy funciona pero se ve "de dev" (emoji + tarjetas planas). El salto a
**muy producido** es lo que convierte un demo en un producto que se vende solo.

Estado actual (código real, `origin/feature/erp-parity` @37c6966):
- `client/src/vertical.ts` — fuente única: `RUBRO_CATALOG` (8 cards: value/label/icon
  emoji/help/seedVertical), `featuresForRubro` (recetas/lotes/physicalStock/clinical),
  `parseRubro`, `seedVerticalFor`, `loadRubro`.
- `client/src/views/configuracion.ts` (~L372-560) — `.rubro-grid` de `.rubro-card`
  (emoji + label + help + tag "datos demo") + preview `.rubro-modules` ("Este rubro
  muestra N secciones" + chips on/off vía `visibleModulesForRubro`/`MODULE_LABELS`
  de `first-run.ts`) + "Guardar rubro" + "Cargar datos demo" (con confirms).
- Gaps: iconos emoji (no on-brand, dependen de fuente del SO), tarjetas sin jerarquía
  ni estados producidos, preview funcional pero seco, sin color por rubro, sin
  navegación por teclado del grid, sin "vista previa de TU ERP" rica, copy plano.

**No reescribir desde cero**: el modelo de datos (`vertical.ts`) y el gating ya son
correctos y testeados. Esto es una **elevación de UX/diseño/contenido** sobre esa base.

---

## 1. Principios de diseño (no negociables)

1. **Production value.** Iconografía custom SVG (no emoji), tarjetas refinadas,
   micro-motion, craft de dark-theme nivel Linear/Stripe. Cada pixel intencional.
2. **Mostrar, no contar.** Elegir un rubro hace *preview en vivo* del ERP exacto que
   recibirás: secciones on/off, terminología propia del rubro, datos demo disponibles.
3. **Confianza + cero lock-in.** "Podés cambiar tu rubro cuando quieras, no se borran
   tus datos." Compliance claro (boleta/factura SII = universal; recetas/controlados =
   solo farmacia).
4. **Keyboard-first + accesible.** Grid navegable con flechas, Enter/Espacio selecciona,
   focus ring visible, `role=radiogroup`/`radio` + `aria-checked`. AA contraste.
5. **Offline-first (ADR-0005).** SVG self-hosted, **cero CDN** (fuentes/iconos). Cae a
   system fonts. Nada que requiera internet.
6. **Performance.** Feedback de selección <100ms, sin layout shift, sin jank; respeta
   `prefers-reduced-motion`.
7. **Una sola fuente de verdad.** Toda la data (icono, color, copy, features) cuelga de
   `vertical.ts`; la vista solo renderiza. Reusable entre Configuración y onboarding
   first-run (mismo componente).

---

## 2. La experiencia (layout + flujo)

**Patrón = "configurador" de dos paneles** (el mismo lenguaje de los configuradores de
producto premium):

```
┌──────────────────────────────────────────────────────────────────────┐
│  Elegí el rubro de tu negocio                                          │
│  RutBusiness se adapta: mostramos solo lo que tu rubro necesita.       │
├───────────────────────────────┬──────────────────────────────────────┤
│  GRID DE RUBROS (izquierda)    │  VISTA PREVIA DE TU ERP (derecha,     │
│  ┌────┐ ┌────┐ ┌────┐ ┌────┐   │  sticky, se actualiza al hover/focus/ │
│  │ 💊 │ │ 🛒 │ │ 🍽 │ │ ☕ │   │  selección)                          │
│  └────┘ └────┘ └────┘ └────┘   │                                      │
│  ┌────┐ ┌────┐ ┌────┐ ┌────┐   │  ▸ <Icono+nombre rubro> + tagline    │
│  │ 🛍 │ │ 💅 │ │ 🔧 │ │ ➕ │   │  ▸ Qué incluye (Ventas/Stock/        │
│  └────┘ └────┘ └────┘ └────┘   │     Compliance/Reportes) — chips on  │
│                                │  ▸ Específico de tu rubro (recetas/  │
│                                │     lotes/servicios-sin-stock…)      │
│                                │  ▸ Datos demo: [Cargar] / Próximam.  │
│                                │  ▸ Secciones ocultas (sutil)         │
│                                │  ▸ Nota compliance SII               │
├───────────────────────────────┴──────────────────────────────────────┤
│  "Podés cambiarlo después."     [Cargar datos demo]  [Guardar y seguir]│
└──────────────────────────────────────────────────────────────────────┘
```

- **Header**: value prop en 1 línea + sub. Sin jerga.
- **Card** (estado rest): icono SVG, label, 1 línea, badges (`DATOS DEMO` /
  `Próximamente`), borde sutil. Accent color del rubro al hover/selección.
- **Preview panel**: se actualiza al **hover/focus** (peek) y se fija al **seleccionar**.
  Es el corazón del "mostrar, no contar": el dueño ve su ERP antes de confirmar.
- **Footer**: `Guardar y seguir` (primary), `Cargar datos demo` (secondary),
  reassurance "podés cambiarlo después".
- En **onboarding first-run** el mismo componente es el paso "Elegí tu rubro";
  `Guardar y seguir` avanza al siguiente paso. En **Configuración** es una sección;
  `Guardar` hace toast + re-render del shell nav.

---

## 3. Profundidad por rubro (el "profundo" — que cada rubro se sienta nativo)

Cada rubro define: **icono** (concepto SVG), **accent**, **tagline**, **value copy**,
**terminología propia**, **secciones on/off** (deriva de `featuresForRubro` +
`visibleModulesForRubro`), **estado de pack demo**. Esto fuerza que cada rubro se sienta
diseñado, no "farmacia menos recetas".

| rubro | icono (concepto) | accent | tagline | nativo (terminología/lo que prende) | demo |
|---|---|---|---|---|---|
| `farmacia` | píldora + mortero | teal/verde salud | "Tu farmacia, en regla." | Recetas, Libro controlados (Ley 20.000), principio activo, lotes/vencimiento, interacciones | ✅ |
| `minimarket` | carro de compras | ámbar | "Tu almacén, al día." | Lotes/vencimiento (perecibles), proveedores, POS rápido por código | ✅ |
| `restaurant` | plato + cubiertos | rojo cálido | "Tu cocina, bajo control." | Insumos + stock; *(futuro: comandas/mesas)*; sin recetas clínicas | ⬜ próximamente |
| `cafe` | taza | café/ámbar oscuro | "Tu café, listo cada mañana." | Lotes/vencimiento (pastelería perecible), producción *(futuro)* | ⬜ |
| `tienda` | bolsa + etiqueta | azul | "Tu tienda, ordenada." | POS + inventario; *(futuro: variantes/tallas)*; sin lotes | ⬜ |
| `belleza` | tijera/sparkle | rosa-violeta | "Tu salón, agendado." | **Venta de servicios sin stock físico**; *(futuro: agenda)* | ⬜ |
| `servicios` | llave/herramienta | slate | "Tu oficio, facturado." | **Servicios sin inventario**; *(futuro: orden de trabajo)* | ⬜ |
| `otro` | plus / grilla | neutro | "Tu negocio, a tu manera." | ERP genérico, sin secciones de rubro | ⬜ (vacío) |

**Disciplina anti-framework** (rubro-catalog §Disciplina): los *(futuro: …)* se
**documentan aquí como dirección** pero NO se construyen hasta validar el rubro con un
cliente real. La pantalla ya puede mostrar la terminología/tagline nativa (copy, barato)
sin construir las features. Pack demo: solo farmacia/minimarket hoy → el resto muestra
`Próximamente` con gracia (no dead-end: igual obtienen un ERP vacío funcional).

**Servicios sin stock** (belleza/servicios, `physicalStock:false`) es la prueba de fuego
del core agnóstico: el preview debe mostrar honestamente "sin inventario/lotes" y el ERP
debe vender un servicio sin pedir stock. Verificar en vivo.

---

## 4. Sistema visual (design system)

- **Icon set**: 8 SVG line-style, stroke consistente (1.75px), opcional 2-tono con el
  accent. Sprite self-hosted (`client/src/brand/rubro-icons.svg` o inline en brand).
  **Reemplaza los emoji** del `RubroCard.icon`. Offline, escalable, nítido en cualquier DPI.
- **Accent por rubro** (CSS var `--rubro-accent`): teal/ámbar/rojo/café/azul/rosa/slate/
  neutro (col. tabla §3). Tematiza borde selección, chip-on, icono, CTA.
- **Estados de card**: `rest` (borde sutil) · `hover` (lift 2px + borde accent + icono
  accent) · `focus` (ring accesible) · `selected` (relleno accent tenue + check ✓) ·
  `demo`/`próximamente` (badge; **igual seleccionable**, no disabled dead-end).
- **Motion**: transiciones 150-200ms ease; preview cross-fade al cambiar; lift en hover.
  `@media (prefers-reduced-motion: reduce)` → sin movimiento, solo cambio de estado.
- **Tipografía/espaciado**: jerarquía clara (header > label > help); scale de spacing
  consistente con `brand.css`/`rutbrand.css`. Sin tocar `styles.css`/`main.ts` (reglas
  de lane existentes — append en brand css, link global ya existe).
- **Responsive**: grid 4 cols → 2 (tablet) → 1 (móvil/ventana angosta); preview pasa
  de panel lateral a bloque inferior en 1 col.

---

## 5. Contenido / copy (español CL profesional, orientado al dueño)

- **Header**: "Elegí el rubro de tu negocio" · sub "RutBusiness se adapta: mostramos
  solo lo que tu rubro necesita. Las boletas y facturas (SII) funcionan en todos."
- **Taglines** por rubro (col. §3) — emocionales, cortas.
- **Preview labels**: "Qué incluye" · "Específico de tu rubro" · "Datos de ejemplo" ·
  "Secciones que se ocultan".
- **Reassurance**: "Podés cambiar tu rubro cuando quieras. Tus datos no se borran."
- **Compliance note**: "Boleta y factura electrónica SII: en todos los rubros. Recetas
  y libro de controlados (Ley 20.000): solo Farmacia."
- **Demo confirms** (ya existen, mantener): primer load + regenerar + advertencia
  "úsalo en instalación de prueba, no sobre datos reales".

Toda copy es-CL, voz de producto (no dev), sin anglicismos innecesarios.

---

## 6. Implementación técnica (archivos + funciones reales)

1. **`client/src/vertical.ts`** (fuente única — extender `RubroCard`, append-only):
   agregar a cada card `accent: string`, `tagline: string`, `valueLines?: string[]` y
   cambiar `icon` a un id de SVG sprite (mantener compat: helper `rubroIcon(rubro)`).
   Mantener `featuresForRubro`/`visibleModulesForRubro` como verdad del on/off.
2. **`client/src/brand/rubro-icons.svg`** (NEW) — 8 iconos line-style + helper para
   inyectarlos. Self-hosted, sin CDN.
3. **`client/src/views/configuracion.ts`** (~L372-560) — reconstruir el render del grid
   a **2 paneles** (grid + preview sticky). Reusar `visibleModulesForRubro`,
   `MODULE_LABELS`, `featuresForRubro`, `seedVerticalFor`. Selección actualiza el preview
   en vivo (hover/focus = peek, click = fija). Conservar "Guardar rubro" + "Cargar datos
   demo" (lógica de confirm intacta).
4. **Reuso onboarding**: extraer el componente (render + handlers) a una función pura/
   reutilizable que `first-run`/onboarding invoque como paso "rubro" (evita divergencia
   UI Configuración ↔ first-run).
5. **CSS**: en `brand.css`/`rutbrand.css` (append; link global ya existe vía index.html;
   NO tocar `styles.css`/`main.ts`). Clases `.rubro-grid/.rubro-card/.rubro-preview/…`
   + var `--rubro-accent`.
6. **Teclado/a11y**: roving tabindex en el grid; `role=radiogroup`+`radio`+`aria-checked`;
   flechas mueven, Enter/Espacio selecciona; focus ring. `bindModalKeys` no aplica (no es
   modal) pero seguir el patrón de teclado del proyecto.

Sin migración, sin backend nuevo (salvo packs demo §7). `api.ts` intacto (append-only si
algo). Money/RUT/fechas vía `format.ts` (no reformatear ad-hoc).

---

## 7. Backend de apoyo (marvin, solo cuando se valide un rubro)

- Packs seed nuevos = un array por rubro en `crates/domain/src/seed.rs` (hoy
  pharmacy/minimarket). **No construir los 6 de una** — se agregan cuando un cliente real
  valida el rubro (anti-framework). El front ya maneja `seedVertical: null` →
  `Próximamente` con gracia.
- `featuresForRubro` vive en el cliente; si algún gating necesita parity en backend
  (ej. validación server-side de módulo), marvin lo expone — hoy no es necesario.

---

## 8. Estados y edge cases (parte del "muy producido")

- **Sin server / hiccup de settings** → `loadRubro` cae a `otro`, nunca dead-end.
- **Pack demo ausente** (`próximamente`) → rubro **igual seleccionable**; CTA demo
  muestra "pack próximamente", no botón muerto.
- **Ya sembrado** → confirm de regenerar (existe).
- **Cambiar de rubro con datos existentes** → copy explícito: "no se borran tus datos,
  solo cambia qué secciones ves". (El gating es de UI, no destructivo.)
- **Ventana muy angosta / DPI alto** → grid reflow + SVG nítido.
- **prefers-reduced-motion** → sin animación.

---

## 9. Definición de hecho ("muy profesional / muy producido")

- [ ] Se ve intencional, on-brand, dark-theme pulido. **Cero emoji**; iconos SVG custom.
- [ ] Seleccionar cualquier rubro muestra al instante un preview **preciso y útil** de
      ESE ERP (secciones on/off correctas vs `visibleModulesForRubro`).
- [ ] Operable 100% por teclado; a11y AA; `prefers-reduced-motion` respetado.
- [ ] Verificado en vivo: farmacia (recetas/controlados/lotes), minimarket (sin recetas),
      y un rubro de servicio (belleza → **sin stock/lotes**, vende servicio).
- [ ] Offline: sin CDN; cae a system fonts; SVG self-hosted.
- [ ] Reusado en onboarding first-run (no dos implementaciones divergentes).
- [ ] GATE cliente verde (`npm run build && npm test`); e2e cubre selección + preview +
      ambos verticales.
- [ ] Sensación: un dueño piensa "esto fue hecho para mi negocio".

---

## 10. Fases (slices, no un PR gigante)

- **P1 — Estructura + preview** (alto valor, bajo riesgo): layout 2 paneles + "Vista
  previa de tu ERP" en vivo reusando la data existente. Sin iconos nuevos todavía.
- **P2 — Producción visual**: icon set SVG custom + accent por rubro + estados + motion.
- **P3 — Profundidad por rubro**: tagline/terminología/value copy nativos + UX de datos
  demo (incl. "próximamente" con gracia).
- **P4 — Reuso + verificación**: componente compartido con onboarding first-run + e2e +
  a11y/teclado + verificación en vivo de los 3 perfiles (farmacia/minimarket/servicio).

Cada fase = un PR chico verde contra `feature/erp-parity`. P1 entrega valor sola.

---

## 11. Ownership

- **ye** (lead) — `configuracion.ts` + `vertical.ts` (append) + brand css + onboarding.
- **marvin** (apoyo) — packs seed por rubro cuando se valide (no especulativo).
- **bob** (apoyo) — e2e: selección de rubro + preview + ambos verticales + servicio.
- **paxoloop** — integra por fase, GATE de record, rewrite ESTADO ACTUAL.

> Norte: esta pantalla es la **vitrina** de RutBusiness. Si una sola pantalla tiene que
> gritar "producto profesional, multi-rubro, hecho para vos", es esta.

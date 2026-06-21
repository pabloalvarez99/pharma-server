# Professional & Complete Master Plan — Centro de Configuración + UX Intuitiva + Pulido

> **Directiva fundador (2026-06-20):** enfocar en (1) la **página de configuración**,
> (2) **funcionalidades para el usuario muy intuitivas y útiles**, (3) que el programa
> sea **muy completo y profesional en general**. Ultra-plan que paxoloop corre contra el
> equipo. Compañero de [`product-improvement-master-plan.md`](./product-improvement-master-plan.md)
> y [`rubro-select-experience.md`](./rubro-select-experience.md) (misma vara de craft).

---

## 0. Tesis

El programa ya es un ERP **funcional** con agente, multi-rubro y compliance — pero
todavía se siente **"de dev"** en bordes decisivos para un dueño real. El founder pide
cerrar exactamente eso: la **página de configuración**, **UX intuitiva/útil**, y
**completitud/profesionalismo** general. Hoy:

- La config es un **scroll plano** que expone **keys crudas** (`admin_setting`,
  `cfg-key` técnico) como una herramienta de desarrollador.
- Tareas clave viven **SOLO en CLI**: crear usuarios/roles (`pharma user-create`), crear
  tenant, respaldo (`pharma backup`), importar licencia. → **un dueño no puede operar su
  negocio sin terminal.** Ese es el mayor gap de profesionalismo.
- Falta una **capa de UX** (command palette, atajos globales, búsqueda/filtros
  consistentes) y un **design system** coherente.

Cerrar esto convierte "ERP funcional" en **"producto profesional y completo"**.

## 1. Estado actual (grounded en el código)

`client/src/views/configuracion.ts` (~783 líneas): scroll plano. Secciones: Conexión al
servidor · Apariencia · Rubro (vitrina ✓) · **Parámetros del servidor** (= lista cruda
de `admin_setting` key/value, muestra el `cfg-key` técnico → se ve "de dev") · Boleta
emisor SII. **CLI-only (sin UI):** usuarios/roles, tenant, respaldo, importar licencia.

## 2. Los tres pilares

### Pilar A — CENTRO DE CONFIGURACIÓN (la "página para configurar") — flagship

Reconstruir la config en un **hub profesional** con **navegación lateral/tabs**, cada
sección una tarjeta cuidada (no key/value crudo, labels humanos). Secciones:

- **Negocio** — identidad: RUT, razón social, giro, dirección, logo, rubro.
- **Usuarios y roles** — CRUD + roles (cajero/químico/admin/dueño). *HOY CLI-only.*
- **Facturación electrónica SII** — emisor, certificado (upload), CAF (upload), folios,
  ambiente sandbox/prod.
- **Medios de pago** — efectivo/débito/crédito/convenios aceptados.
- **Sucursales y cajas** — multi-tenant/multi-caja que el producto promete.
- **Hardware** — impresora de boleta, lector de código, balanza.
- **Respaldo** — programar, ejecutar ahora, restaurar guiado. *HOY CLI-only.*
- **Licencia y plan** — estado, activar, features, upgrade.
- **Preferencias** — tema, idioma, telemetría opt-in (default OFF).
- **Agente** — ajustes; LLM opt-in cuando el founder lo decida (ADR-0016).

Con: **búsqueda dentro de settings**, validación inline, guardar con feedback claro,
defaults sanos, ayuda contextual. **Esconder keys técnicas** (mostrar labels, no
`admin_setting` crudo).

### Pilar B — UX INTUITIVA Y ÚTIL

- **Command palette global (Ctrl/Cmd+K):** navegar a cualquier vista + ejecutar acciones
  + invocar al agente. El acelerador que define un producto pro.
- **Atajos de teclado globales** + **cheatsheet descubrible** (tecla `?`).
- **Búsqueda + filtros consistentes** en todas las listas (productos/ventas/clientes…).
- **Empty states que enseñan**, tooltips, ayuda inline, **confirm/undo** para destructivo,
  toasts consistentes.
- **Onboarding wizard completo** (instalar → negocio → usuario → rubro → primera venta).

### Pilar C — COMPLETO Y PROFESIONAL (consistencia + pulido + cerrar gaps)

- **Design system:** tokens (color/espaciado/tipografía) + componentes compartidos
  (botón/input/card/modal/tabla/drawer + estados empty/loading/error) coherentes —
  extraer lo que ya existe a un set único.
- **Pasada de consistencia:** cada vista al mismo bar de craft; cero pantallas "de dev".
- **Cerrar gaps CLI→UI** (usuarios, respaldo, licencia) → el dueño nunca toca terminal.
- **Ayuda / Acerca de** in-app, versión, soporte.

## 3. Lanes (5, disjuntas)

1. **ye — CENTRO DE CONFIGURACIÓN** [Pilar A, flagship]: reconstruir `configuracion.ts`
   en hub multi-sección (nav + tarjetas), labels humanos, búsqueda-en-settings,
   validación, save feedback; montar la vitrina rubro adentro; secciones Negocio/
   Preferencias/Licencia/Agente + monturas para Usuarios/SII/Respaldo/Sucursales. +
   Tauri commands necesarios (único dueño de src-tauri).
2. **milton — backend config UI-only-hoy** [Pilar A enable]: endpoints admin para
   **usuarios/roles** (crear/listar/editar/deshabilitar + asignar rol, vía auth) +
   **medios de pago** + persistencia de settings de negocio. (NO client, NO src-tauri.)
3. **marvin — completeness backend** [Pilar A/C]: **sucursales/cajas** + **CAF/cert
   upload** plumbing (SII) + **respaldo-trigger** endpoint (envuelve `backup`). mig 0032
   si hace falta. (Módulos disjuntos de milton.)
4. **paul — UX intuitiva** [Pilar B]: **command palette (Ctrl+K)** + **atajos globales**
   + **cheatsheet (`?`)**, módulo propio (`command-palette.ts`/`keymap.ts`), montado por
   1 línea en shell (coord ye). Keyboard-first, craft vitrina.
5. **bob — design system + pulido + e2e** [Pilar C]: tokens + componentes/estados
   producidos (additive, no fuerza adopción same-wave) + pulido de sus vistas (reports/
   boletas/facturas/recetas/auditoría) + e2e del centro de config (follow-up de ye).

## 4. Definición de hecho

- [ ] El dueño configura **TODO** desde la UI (usuarios, SII, respaldo, licencia,
      sucursales) — **cero CLI**.
- [ ] **Ctrl+K** abre command palette; **`?`** muestra atajos; búsqueda en todas las listas.
- [ ] **Cero pantallas "de dev"**; settings con labels humanos, no keys crudas.
- [ ] Design system consistente; estados empty/loading/error producidos en toda vista.
- [ ] Offline-first, multi-rubro, GATE verde, e2e. Sensación: "esto es un producto pro".

## 5. Coordinación (cero contención)

- **domain/api split:** milton (users/payment/settings) ↔ marvin (branches/caja/
  dte-config/backup). Ambos tocan `v1/mod.rs` con líneas mínimas → paxoloop reconcilia.
- **src-tauri = solo ye** (Tauri commands del config center).
- **command palette = paul** (módulos propios) + 1 línea de mount en shell (coord ye).
- **design system = bob** (additive); otros adoptan en olas siguientes, no same-wave.
- **format.ts** append = bob · **css** split (paul, ye `.cfg-*`/`.agent-*`, bob `.ui-*`).
- **mig** solo marvin = 0032.

> Norte: la pantalla de configuración + la capa de UX intuitiva son lo que un dueño
> "toca" para sentir que el programa es profesional y completo. Si una superficie tiene
> que gritar "producto serio, hecho para operar tu negocio entero sin terminal", es esta.

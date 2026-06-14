# MULTI-RUBRO FINDINGS

Hallazgos de la lane onboarding/selección-de-rubro (Session "ye",
`feat/client-onboarding-vertical`, 2026-06-13). Insumo para las lanes hermanas
(compliance/reporting, branding) y para el integrador.

## Estado del concepto "rubro" antes de esta lane

- **NO existía** ninguna noción de rubro/vertical en el branch
  `feature/erp-parity` (v0.1.28). `grep -rn vertical crates/ client/src` = 0
  hits funcionales.
- **El seeder NO existe.** El prompt de la lane afirmaba que ya estaba hecho
  (`pharma seed-demo --vertical pharmacy|minimarket`, `crates/cli/src/seed_cmd.rs`).
  Verificado contra el branch: `crates/cli/src/` sólo tiene `main.rs`,
  `backup_cmd.rs`, `dte_cmd.rs`. No hay `seed_cmd.rs` ni comando `seed-demo`
  en `git log --all`. → la premisa del prompt es falsa.

## Entregado en esta lane (cliente puro, GATE cliente)

- **`client/src/vertical.ts`** (NUEVO) — single source of truth del concepto:
  tipo `Vertical = farmacia|minimarket|otro`, claves `business.vertical` /
  `business.name` (admin_setting), `parseVertical` (default `otro`, NUNCA
  farmacia), `hasRecetas(v)` (true sólo farmacia — gate Ley 20.000),
  `hasDte(v)` (true siempre — DTE es universal en CL), loaders async sin throw.
  **Contrato compartido**: la lane de compliance debe importar `hasRecetas` /
  `hasDte` de aquí para condicionar `recetas.ts` / `facturas.ts`, en vez de
  re-derivar la regla.
- **`configuracion.ts`** — sección "Rubro del negocio": selector
  farmacia/minimarket/otro + nombre del negocio, persistidos en admin_setting.
  Ayuda inline explica que Recetas/controlados depende del rubro y que las
  boletas/facturas SII funcionan en todos.
- **`shell.ts`** — branding dinámico: el nombre de la barra lateral sale de
  `business.name` (fallback genérico `pharma-server`, ya NO "Tu Farmacia"
  hardcodeado). El nav "Recetas" se oculta cuando el rubro no es farmacia
  (`hydrateBranding`, post-render para no cambiar la firma de `renderShell`).
- **`dashboard.ts`** — copy genérico ("tu negocio" en vez de "tu farmacia").
- **`vertical.test.ts`** (NUEVO, 7 tests) — parse/default/gates/catálogo.

## Pendiente / bloqueado para lanes hermanas

1. **Botón "cargar datos demo" (task 3) — BLOQUEADO**: requiere un seeder que
   no existe. Necesita primero una lane backend que cree
   `crates/cli/src/seed_cmd.rs` (`pharma seed-demo --vertical <v>`,
   idempotente, datos rotulados DEMO) y/o un comando Tauri que lo invoque.
   Recién entonces el cliente puede exponer el botón. **No se fabricó un botón
   sin backend.**
2. **`login.ts` sigue con defaults farmacia-only** (`DEFAULT_TENANT="tufarmacia"`,
   `DEFAULT_EMAIL="admin@tufarmacia.cl"`, marca "Tu Farmacia", tagline "Tu
   farmacia, lista."). Es **pre-auth**: no se puede leer `business.*` antes de
   iniciar sesión (no hay token ni tenant aún). Degenerizarlo requiere otra
   fuente (config de build, o un endpoint público de branding) → fuera de
   scope de esta lane, anotado para la lane de branding.
3. **`server-side` no lee `business.vertical`** — hoy es señal 100% de UI. Si
   en el futuro alguna regla de negocio debe depender del rubro (p.ej. rechazar
   crear una receta en un minimarket), hay que leerlo en el backend. Por ahora
   el ocultamiento es sólo cliente.
4. **Doc `docs/operator/01-primer-inicio.md`** referencia la marca "Tu
   Farmacia" y "panel azul con logo"; con el branding dinámico, el nombre real
   ahora lo fija el operador. La lane de docs debería generalizar esa redacción.

## Compliance lane (Lucy) — contrato

- `boletas.ts` / `facturas.ts`: NO condicionar por rubro (DTE universal,
  `hasDte` siempre true). Probar con seed minimarket cuando exista.
- `recetas.ts`: condicionar visibilidad/entrada con `hasRecetas(vertical)`
  importado de `client/src/vertical.ts`. El nav ya se oculta en `shell.ts`;
  falta el guard dentro de la vista por si se navega por URL/estado.

---

## Compliance lane — entregado (`feat/client-test-compliance`, 2026-06-14)

Cumple el contrato de arriba. **Cliente puro, GATE cliente verde.** No tocó
backend ni `shell.ts`/`vertical.ts` (de ye) → cero contención de merge.

### Gating aplicado
- **`recetas.ts`** — guard a nivel de vista: `renderRecetas` carga el vertical
  (`loadVertical(serverUrl)`) y, si `!hasRecetas`, muestra "Módulo exclusivo del
  rubro Farmacia" en vez de la vista de controlados. Cubre el deep-link que
  saltaría el ocultamiento de nav (gap #3 del contrato cerrado en cliente).
  Self-contained: la vista ya recibía `serverUrl`, no cambió firma ni `shell.ts`.
- **`boletas.ts` / `facturas.ts`** — confirmado universal, **sin** gating de
  vertical. Verificado con seed `minimarket` en la lane E2E
  (`feat/client-e2e-harness`, PR #167): minimarket emite boleta y no se le exige
  receta.

### Extracción + cobertura (`client/src/dte.ts` + `dte.test.ts`)
Las vistas eran renderers DOM con la lógica de compliance **duplicada inline**
(boletas + facturas). Se extrajo a `client/src/dte.ts` puro y testeable —
fuente única, dedup real — y las vistas ahora la importan (comportamiento
idéntico). El test es la cobertura de los riesgos cazados:

| Riesgo | Helper / hallazgo | Estado |
|---|---|---|
| 402 sin crash (upsell) | `upgradeNote(err, plan)` — nota calmada Pro/Business, nunca crash/dark-pattern; reportes margins → tarjeta soft | ✅ |
| neto/IVA | `computeDocTotals` replica `desglose_iva` (`crates/dte/src/emit.rs`): trunc CLP entero, IVA absorbe redondeo, exento aparte | ✅ |
| CAF/folio | `cafTone`: `<=0` danger / `<=low` warn (boleta 50, factura 20) / ok; "Sin CAF" con `cafs.length===0` | ✅ |
| Reportes vacío/grande/TZ | `pickToday` cae al último row (borde TZ no deja blanco), `undefined` con 0 rows → "$0"; rotación corta a 15, top a 5 | ✅ |
| CSV export | `exportLibroRecetas` / `exportProducts` exponen CSV crudo (sin lock-in, ADR-0005 §4); contenido/headers aún sin asserción E2E | ⚠️ observado |

### Gaps abiertos (recomendaciones)
1. El guard de vista es **defensivo, no autoritativo** — el server no rechaza
   `GET /prescriptions` por vertical (sigue siendo señal de UI; ver gap #3 de ye).
   Para cumplimiento duro: gatear también en el API.
2. `loadVertical` default `otro` en error de red podría ocultar Recetas a una
   farmacia real durante un hipo de settings. El nav ya gatea, así que el guard
   sólo se alcanza por deep-link; aceptable, anotado.
3. **CSV export sin verificación E2E** — candidato a la suite `client/e2e`.

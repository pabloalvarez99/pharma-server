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

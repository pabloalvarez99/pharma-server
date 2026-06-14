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

1. **Botón "cargar datos demo" (task 3) — AÚN BLOQUEADO (parcial)**:
   - **Actualización 2026-06-13**: marvin landeó `pharma seed-demo` como **CLI**
     (PR #163, `crates/cli/src/seed_cmd.rs`, `pub async fn run(tenant_slug,
     vertical, force)`). Pero **NO existe el endpoint HTTP**
     `POST /api/v1/admin/seed-demo` que el botón del cliente necesita
     (`git grep seed.demo origin/feat/cli-seed-demo-rutagentia -- crates/api/`
     = 0 hits). El cliente Tauri llama endpoints HTTP, no la CLI del server.
   - **Falta** una lane que exponga el CLI seed como endpoint admin
     (`POST /api/v1/admin/seed-demo {vertical}` → summary), idempotente.
     Recién ahí se cablea el botón. **No se fabricó botón sin endpoint.**
   - **⚠️ MISMATCH DE NOMBRES**: el CLI de marvin usa `pharmacy|minimarket`
     (inglés); `client/src/vertical.ts` usa `farmacia|minimarket|otro`
     (español). El endpoint puente DEBE mapear `business.vertical` (es) →
     pack del seeder (en): `farmacia→pharmacy`, `minimarket→minimarket`,
     `otro→` (sin pack, error claro). Integrador: alinear o mapear.
2. **`login.ts` branding genérico — ✅ HECHO (esta lane)**: removidos
   `DEFAULT_TENANT="tufarmacia"` / `DEFAULT_EMAIL="admin@tufarmacia.cl"`; marca,
   tagline, wordmark, pillars y footer ya no dicen "farmacia". Branding pre-auth
   resuelve: `VITE_BRAND_NAME`/`VITE_BRAND_TAGLINE` (override de build) >
   `localStorage["pharma:brand-name"]` (persistido por `shell.ts` tras login) >
   fallback neutral `pharma-server` / "Tu negocio, listo.". La sucursal se
   recuerda en `localStorage["pharma:last-tenant"]` (sin default de rubro).
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

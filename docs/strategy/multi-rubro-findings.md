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

## First-run LIVE QA (ye, lane feat/firstrun-live-qa) — DB fresca

Manejé el primer-inicio REAL contra una DB vacía (sin tenant ni usuario), el
escenario del RUT solo que recién instaló el MSI. Fricción hallada + acción:

1. **Chicken-and-egg de cuenta (P0 de onboarding)** — DB fresca no tiene tenant
   ni usuario → `/login` nunca puede pasar → el operador queda atrapado en la
   pantalla de login. La única salida hoy es el **CLI** (`pharma tenant-create` +
   `pharma user-create`), lo que viola "primer-inicio SIN tocar CLI" y el modelo
   freemium de un dueño solo (no hay admin que le cree la cuenta).
   → **FIJADO**: endpoint UNAUTENTICADO `GET /api/v1/setup/status` +
   `POST /api/v1/setup` (`crates/api/src/setup.rs`) que crea el primer
   tenant+owner cuando la DB no tiene usuarios y deja al operador logueado.
   **Fail-closed**: en cuanto existe 1 usuario, status→false y un segundo setup
   da 409 `SETUP_ALREADY_DONE` (no es backdoor). Cableado en cliente:
   `login.ts` sondea setup al renderizar y, si hace falta, muestra el formulario
   "Crea tu cuenta" (nombre del negocio + rubro + correo + clave). El rubro
   elegido se guarda como `business.vertical` en el mismo paso → el dashboard no
   arranca asumiendo farmacia. Resuelve además el pendiente #2 de arriba
   (defaults farmacia pre-auth): ahora hay un paso pre-auth multi-rubro real.

2. **Defaults de login engañosos + farmacia-hardcoded** — `login.ts` pre-llenaba
   `DEFAULT_TENANT="tufarmacia"` y `DEFAULT_EMAIL="admin@tufarmacia.cl"`. En una
   DB fresca esos valores apuntan a nada → el operador toca ENTRAR y recibe
   "credenciales no coinciden", confuso porque los campos PARECEN configurados.
   Además es marca farmacia (multi-rubro). → **FIJADO**: defaults genéricos
   (`principal` / correo vacío). "principal" coincide con el nombre de sucursal
   sugerido en `docs/operator/01-primer-inicio.md`.

3. **Login sin salida en instalación fresca** — la pantalla de login no ofrecía
   ninguna pista de "¿primera vez? crea tu cuenta"; era un dead-end. → **FIJADO**
   por el sondeo de #1 que intercambia la tarjeta por el formulario de setup.

4. **Doc `docs/operator/01-primer-inicio.md` ↔ app divergen** — el manual asume
   que "el dueño o el admin" ya registró tu usuario; para el install freemium de
   un solo RUT NO existe ese admin. Con el setup in-app el operador se auto-crea
   la cuenta. → Anotado; generalizar la redacción del manual es de la lane de
   docs (fuera de mi scope de views), pero el flujo real ya no requiere admin.

Prueba REAL contra DB fresca: `crates/api/tests/setup_firstrun.rs` levanta la
app axum sobre una DB kv vacía y corre el viaje completo por HTTP —
status(needs_setup) → setup → /me(owner) → business.vertical persistido →
fail-closed(409) → login con las credenciales recién elegidas → seed-demo →
catálogo poblado (camino de la primera venta alcanzable). Sin CLI en ningún paso.

## Ola 5 — first-run en vivo (ye, `feat/firstrun-vertical-polish`, 2026-06-15)

5. **Dead-end del camino "datos demo"** — el CTA de panel vacío
   (`dashboardCta` action `seed-demo`) ruteaba a **Importar** (CSV) con la
   etiqueta "Cargar productos", pero el botón **"Cargar datos demo"** vive en
   **Configuración** (grilla de rubro). El operador que quería demo aterrizaba
   en una pantalla de import CSV sin botón demo → dead-end. → **FIJADO**: el CTA
   `seed-demo` ahora rutea a `configuracion` con etiqueta "Cargar datos demo"
   (`client/src/views/dashboard.ts` `CTA_NAV`). Cierra el loop: panel vacío →
   CTA → Configuración (elige rubro + Cargar datos demo) → panel poblado → POS.

6. **Copy marca-farmacia residual (i18n multi-rubro)** — barrido de strings
   hardcodeados con farmacia en las vistas de onboarding:
   - `login.ts` placeholder de correo `usuario@farmacia.cl` →
     `usuario@minegocio.cl` (genérico, alineado con el form de setup que ya usa
     `dueno@minegocio.cl`).
   - `login.ts` footer "datos siempre en tu **farmacia**" → "en tu **negocio**".
   - `configuracion.ts` placeholders del emisor DTE "Farmacia Ejemplo SpA" /
     "Venta al por menor de productos farmacéuticos" → "Mi Empresa SpA" /
     "Venta al por menor en comercios especializados" (genérico).
   → **FIJADO**. Boleta/factura SII es universal CL, el emisor no debe presumir
   farmacia.

7. **`docs/operator/01-primer-inicio.md` aún dice "Tu Farmacia" / `tufarmacia`**
   — el manual sigue marca-farmacia (`cajera@tufarmacia.cl`, sucursal por defecto
   `tufarmacia`), pero la app ya migró a defaults genéricos (`principal`, correo
   vacío) → la app va ADELANTE del manual. Generalizar el manual es de la lane de
   docs (fuera de mi scope de views). Anotado.

## Migración de catálogo CSV (ye, lane feat/import-csv-migration-qa)

Blocker #1 de onboarding: el operador llega con su catálogo de miles de SKU
exportado de **otro sistema** (o de Excel CL) y debe migrarlo sin perder datos.
El importador asumía un CSV "limpio" estilo `scripts/extract_tufarmacia_full.py`
(inglés, `,`, sin BOM, precios enteros). Contra archivos reales fallaba silencioso
o **corrompía datos**. Cazado contra backend vivo (`crates/api/tests/import_endpoint.rs`,
13 tests) + unit tests de parsing (`v1::catalog::import_parsing_tests`):

1. **Separador `;` (Excel Chile) → archivo entero rechazado** — Excel en locale CL
   exporta con `;` (porque `,` es la coma decimal). El lector usaba `,` fijo → toda
   la fila de cabecera era una sola columna → "falta name/price". → **FIJADO**:
   `sniff_delimiter` detecta `;`/`,`/TAB de la primera línea.

2. **BOM UTF-8 → cabecera rota** — "Guardar como CSV UTF-8" de Excel antepone
   `EF BB BF`, que se pega a la primera cabecera (`name` → `\u{feff}name`) y rompe la
   detección de columnas. → **FIJADO**: `strip_bom`.

3. **Punto de miles CLP = CORRUPCIÓN SILENCIOSA** — `1.990` (1990 pesos) parseaba
   como `Decimal` 1.99; `12.500` → 12.5. El peor bug de import: no error, dato malo
   guardado. → **FIJADO**: `normalize_decimal` resuelve miles vs decimal (`1.990`→1990,
   `1.990,50`→1990.50, `12.50`→12.50 intacto). `normalize_int` para stock/descuento.

4. **Cabeceras en español ignoradas** — el sistema previo del migrante emite
   `nombre`/`precio`/`código`/`existencia`/`categoría`… El importador sólo conocía
   inglés. → **FIJADO**: `canon_header` mapea alias ES→canónico (folding de acentos).

5. **Columna `category` inusable para CSV reales (rechazo por fila)** — `category`
   exigía un id de registro `category:xxx`; un CSV de migrante trae NOMBRES de
   categoría → `id inválido: Analgésicos`. → **FIJADO**: el handler resuelve
   nombre→id con **find-or-create** (cachea + reusa existentes; re-import no duplica).

6. **Sin preview antes de commit** — el operador importaba a ciegas; un CSV malo
   escribía basura sin chance de revisar. → **FIJADO**: paso `?dry_run=true` (valida
   + cuenta SIN escribir) cableado como **Previsualizar → Confirmar** en `importar.ts`.
   Import parcial: filas buenas entran, malas se reportan por línea (en español),
   no aborta todo. Idempotencia por `external_id` ya existía; preview respeta dedup
   within-file. Filas vacías de relleno (Excel) se ignoran, no cuentan como error.

Nota de scope: (1)-(5) tocaron `crates/api/src/v1/catalog.rs` (backend roto en caso
real → GATE workspace, no sólo cliente, por charter). Cliente: `importar.ts` (flujo
2 pasos) + `api.ts` append-only (`importProductsPreview`) + `import_products_preview`
Tauri cmd. Pendiente lane hermana: `docs/operator` no documenta migración CSV todavía.

## 2026-06-16 — Gating de módulos por rubro (no solo recetas) — ye

7. **El gate de UI era binario (recetas sí/no) pero el catálogo tiene 8 rubros**
   — `parseVertical` colapsa todo extra a `otro`, así que café (perecibles) y
   peluquería (servicio sin stock) recibían el MISMO menú. → **FIJADO**: modelo de
   capacidades por rubro en `vertical.ts` (`featuresForRubro` → `recetas`/`lotes`/
   `physicalStock`/`clinical`) sobre la clave completa del catálogo (`parseRubro`,
   NO colapsa). `visibleModulesForRubro` (first-run.ts) deriva el menú; shell.ts y
   el preview de Configuración leen la MISMA fuente → no divergen.
   - recetas/controlados + campos clínicos: SOLO farmacia.
   - lotes/vencimiento (perecibles): farmacia + minimarket + café.
   - servicios/belleza = ventas SIN stock físico → ocultan Inventario + Compras,
     pero conservan POS/caja/boletas (prueba el core agnóstico).

8. **PENDIENTE (anti-framework, no construido aún)**: `restaurant` quedó con
   `lotes:false` por seguir el brief literal (perecibles = farmacia+minimarket+café).
   Un restaurant real maneja insumos perecibles → cuando se valide ese rubro con un
   cliente, revisar a `lotes:true`. No se cambia ahora (disciplina: pack/feature por
   rubro se construye al validarlo, docs/strategy/rubro-catalog.md §Disciplina).

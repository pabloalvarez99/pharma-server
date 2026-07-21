# SP3 — Cliente web: shim invoke→fetch + build PWA de `client/src`

Ingeniero en **pharma-server**, carpeta `client/` (Tauri 2, TS vanilla + Vite).
Objetivo: el MISMO frontend del cliente desktop corriendo en browser contra
`api.rutbusiness.cl` (ADR-0015 P2). Código independiente de SP1 (probar en vivo sí
lo necesita; mientras, probar contra server local en 8090).

## Setup

- Worktree lane: `D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\saas-web`,
  branch `feature/saas-web` (base `origin/feature/erp-parity`). Checkout principal
  sucio — no tocarlo. Paths de este SP: `client/**` (disjunto de SP2 `crates/**` —
  paralelizables).
- Leer `client/package.json` primero: scripts reales de build/test (hay `*.test.ts`
  y `*.dom.test.ts` en `client/src/views/` — verificar runner y usarlo en el gate).

## Leyes

1. **Cero cambios en las 18 vistas ni en `client/src/api/*`** salvo bug real: el shim
   entra por alias de build. El barrel `client/src/api/index.ts` re-exporta módulos
   por dominio — intacto.
2. Gate de este SP = gate del client (lint/test/build según package.json) + GATE Rust
   solo si se toca Rust (no debería).
3. UI español, mismos mensajes de error que desktop.

## Hechos verificados (2026-07-21)

- **73 comandos** `#[tauri::command]` en `client/src-tauri/src/commands/*.rs`, cada
  uno proxy HTTP fino 1:1: el doc-comment de cada comando documenta método + path
  (ej. `list_products` = GET `/api/v1/products` con query `search`,`limit`, Bearer).
  **Fuente de la tabla del shim = esos doc-comments + el cuerpo del comando.**
- Token: vive en `SessionState` Rust (secrecy), comandos lo leen con `token_of`.
  En web: el shim guarda token en memoria + `sessionStorage` (NO localStorage).
  Identificar los comandos de login/setup que lo SETEAN (en `commands/auth.rs` o
  similar — leer) y replicar ese flujo en el shim.
- Errores: server envelope `{error:{code,message}}`; `http.rs` los convierte a
  `Err(String)` en español y errores de conexión pasan por `conn_error`. El shim debe
  producir los MISMOS strings (portar `error_message`/`conn_error` de
  `client/src-tauri/src/http.rs`). Timeouts: 30s API / 5s connect / 5s health
  (en fetch: `AbortSignal.timeout`).
- Server URL: `client/src/api/server-url.ts` ya existe (ADR-0015 P0) — key
  localStorage `pharma:last-server`, fallback `http://127.0.0.1:8080`. Build web:
  default `https://api.rutbusiness.cl` (inyectar vía env Vite, ej.
  `VITE_DEFAULT_SERVER_URL`, sin romper desktop).
- Frontend importa `invoke` de `@tauri-apps/api` (^2). Vite permite `resolve.alias`
  condicional por modo.

## Tareas

1. **Shim** `client/src/web-transport/` (nuevo): implementa `invoke(cmd, args)` con
   `fetch`. Tabla cmd→(método, path, query/body/headers) portada de los 73 comandos.
   Agrupar por dominio espejo de `commands/*.rs` para revisabilidad.
2. **Degradación desktop-only**: comandos sin equivalente HTTP puro (impresora
   ESC/POS `escpos.rs`, updater, diálogos de archivo nativos — identificar lista
   exacta leyendo los comandos) → el shim retorna error controlado
   `"Disponible en la app de escritorio"`; la vista NO muere.
3. **Build web**: modo Vite `web` con alias `@tauri-apps/api/core` → shim, manifest
   PWA (nombre RutBusiness, icons desde `client/src/brand/`), service worker
   shell-only (datos siempre red). Script npm `build:web`.
4. **Tests**: unit del shim (mapping de 5-6 comandos representativos con fetch
   mockeado, incl. error envelope y degradación) + smoke `build:web` en el gate.
5. **Doc**: `client/README` o doc corto: cómo buildear/deployar web (deploy real =
   estático a Vercel o Caddy de SP1 — documentar, ejecutar solo si SP1 ya está vivo).

## Verificación

Server local (binario Windows ok) en 8090 + `vite dev` modo web: login, POS venta,
inventario, cierre caja desde browser. Si SP1 vivo: mismo smoke contra
`https://api.rutbusiness.cl` y anotar resultado.

## Ship

Gate client verde → commit al lane → push (PR del lane ya abierto por SP1/SP2; si no,
crearlo contra `feature/erp-parity`).

Fin → `✅ SP3 LISTO — ERP corre en browser (shim 73 cmds) · commits en PR lane · listo para /clear`.

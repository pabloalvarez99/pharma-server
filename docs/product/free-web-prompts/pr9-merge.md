# PR9 — Merge final: cerrar el lane Free Web (RutBusiness)

Engineer on **RutBusiness**. Free Web PR1–PR8 COMPLETO y verificado
(demo end-to-end + GATE 903 tests verde en `8fe4c8f`). Esta sesión SOLO
mergea y cierra loops — cero código nuevo.

## 1. pharma-server — merge del lane

Worktree: `D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\free-web`
(branch `feature/free-web-shopify-parity`, HEAD `8fe4c8f`, pushed).

- Confirmar base: el lane salió de `feature/erp-parity` (verificar con
  `git merge-base` / `gh pr list`). CI está billing-walled — el gate local es la red.
- `gh pr create` (si no existe) base `feature/erp-parity` → `gh pr merge --merge`
  (repo usa merge commits, NO squash).
- Post-merge: checkout/pull de `feature/erp-parity` en el checkout principal
  (`D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\pharma-server`) y correr
  GATE workspace sobre el resultado mergeado:
  ```powershell
  Get-Process sccache -ErrorAction SilentlyContinue | Stop-Process -Force
  $env:CARGO_INCREMENTAL = "0"   # gotcha: sccache server hereda env sucio
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```
  Verde → push. Rojo → arreglar en la base, NUNCA debilitar asserts.
- Cleanup: `git worktree remove` del worktree free-web (la DB demo
  `data/surreal-demo-pr8` es desechable) + borrar branch local/remota del lane.

## 2. pharma-license-server — merge keys pagas web.*

Repo: `D:\Respaldo Proyectos\GitHub\01-product\rutbusiness\pharma-license-server`,
branch `feat/web-paid-features` (`62ba425`, pushed).

- `gh pr list` — si no hay PR, crearlo contra la base default del repo.
- Checks del repo (ver package.json: lint/test/build) verdes → merge + borrar branch.

## 3. Storefront — nada que mergear

`rutbusiness-storefront` ya vive en `master` pushed (`7e49a0f`). Solo verificar
`git status` limpio.

## 4. Cierre

- `bitacora.md` (en `feature/erp-parity` post-merge) — append:
  `- 2026-XX-XX: Free Web lane mergeado a erp-parity (PR #NNN) + web.* pagas mergeadas en pharma-license-server. Loop cerrado.`
- Commit `docs: bitácora merge Free Web` + push.
- Puerto 8080 lo ocupa `Desktop\RutAgent-Demo\pharma-api.exe` (instancia del
  founder — NO matar). Tests no lo necesitan.

Done → `✅ FREE WEB MERGEADO — lane cerrado, branches borradas`.

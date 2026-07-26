# SP2 — Provisioning API: `POST /admin/v1/tenants` (signup crea tenant)

Ingeniero en **pharma-server** (Rust workspace). Objetivo: endpoint que permite al
license-server (signup web) crear tenant + usuario admin. Código independiente de SP1
(solo el deploy final lo necesita).

## Setup

- Worktree lane: `D:\Respaldo Proyectos\GitHub\.worktrees\pharma-server\saas-web`,
  branch `feature/saas-web` (base `origin/feature/erp-parity`). Checkout principal
  sucio — no tocarlo. Si SP1 ya abrió PR del lane, este SP pushea commits al mismo.

## Leyes

1. GATE: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
2. Multi-tenant obligatorio: toda tabla de dominio lleva `tenant: record<tenant>` +
   índice compuesto con `tenant` (patrón `migrations/0001_init.surql`).
3. Migraciones append-only: NUNCA editar `NNNN_*.surql` aplicada; nueva = `NNNN+1_*`.
4. Errores user-facing en español, códigos en inglés. Envelope de error existente:
   `{ "error": { "code", "message", "details"? } }` (`crates/api/src/error.rs`).
5. NUNCA debilitar asserts para forzar verde.
6. sccache error → `CARGO_INCREMENTAL=0`. Puerto local: usar 8090 (8080 ocupado).

## Hechos verificados (2026-07-21)

- CLI ya crea tenants/usuarios: comandos `tenant-create`, `user-create` (argon2id,
  password vía flag/`PHARMA_PASSWORD`/prompt) en `crates/cli`. **Leer su
  implementación primero** y extraer la lógica compartible a lib (probablemente
  `crates/db` o `crates/domain` — decidir según dónde viva hoy) para llamarla desde
  CLI y handler sin duplicar.
- Config por env: `PHARMA__*` separator `__` → key nueva `PHARMA__PROVISIONING__KEY`
  (agregar al `AppConfig` en `crates/core`, opcional/None por default).
- Auth existente: JWT HS256 con claim `tenant_id` (`crates/auth`). El endpoint
  provisioning NO usa JWT — usa header `X-Provisioning-Key` comparado constante-tiempo
  contra la env.
- Rubros válidos (catálogo v1): `farmacia minimarket restaurant cafe tienda belleza
  servicios otro` (guardado como `admin_setting business.vertical` — verificar cómo lo
  setea `seed-demo`/onboarding hoy y replicar).

## Contrato del endpoint

`POST /admin/v1/tenants` — SOLO montado si `PHARMA__PROVISIONING__KEY` está seteada
(instalaciones on-prem no lo exponen; sin env → 404 como si no existiera).

Request:
```json
{
  "slug": "mi-negocio",            // opcional; derivar de business_name si falta
  "business_name": "Mi Negocio",
  "rut": "76.543.210-K",
  "vertical": "minimarket",
  "admin_email": "dueno@mail.cl",
  "admin_password": "..."
}
```
Responses: `201 {tenant_id, slug}` · `401 PROVISIONING_KEY_INVALID` (key mala) ·
`409 TENANT_EXISTS` (slug/rut duplicado) · `422` validación (rut formato, vertical
fuera de catálogo, password corta — reusar validaciones existentes si hay).
Cada creación → línea de audit log (tracing) con slug + rut, sin password.

## Tests (mínimo)

- Sin env → 404. Env seteada + key mala → 401. Key ok → 201 y login del admin
  funciona (JWT con `tenant_id` correcto).
- Duplicado → 409. Vertical inválido → 422.
- **Aislamiento cross-tenant** (test nuevo, gate de salida a público): crear 2 tenants
  vía endpoint, sembrar producto en A, verificar que listado con JWT de B NO lo ve.
  Cubrir 2-3 endpoints representativos (products, sales/POS, settings).

## Ship

GATE verde → commit al lane → push → PR del lane (crear contra `feature/erp-parity`
si SP1 aún no lo abrió).

Fin → `✅ SP2 LISTO — provisioning API + aislamiento testeado · commits en PR lane · listo para /clear`.

# bitácora — pharma-server

Registro cronológico de decisiones técnicas, cambios significativos e incidentes.
Formato: `## YYYY-MM-DD — título corto` + bullets `qué / por qué / archivos / commit`.
Espejada en vault: `C:/Users/Administrator/Documents/obsidian-mind/work/active/pharma-server/bitacora.md`.

---

## 2026-05-07 — scaffold inicial + sistema de memoria/contexto

- **Qué**: estado actual del repo tras 4 commits iniciales + setup del sistema de memoria/contexto (CLAUDE.md, esta bitácora, vault hint hook, notas Obsidian).
- **Por qué**: dejar a futuras sesiones Claude (y al dev humano) un punto de entrada claro al proyecto sin tener que reconstruir contexto desde cero.
- **Estado del scaffold**:
  - 8 crates: `core`, `db`, `api`, `auth`, `jobs`, `telemetry`, `service`, `cli`.
  - axum 0.8 API + JWT HS256 auth (`/api/me` con extractor `AuthUser` ya funciona y testeado).
  - SurrealDB embedded `kv-surrealkv` en `./data/surreal` (ns `pharma`, db `main`).
  - Migración `0001_init.surql` aplicada via CLI: tablas `tenant`, `user` (argon2id), `session` (jti UNIQUE).
  - CLI `pharma migrate` IMPLEMENTADO con tracking `_migrations` SCHEMAFULL.
  - CLI `pharma config` IMPLEMENTADO. `tenant-create` y `user-create` son **TODO stubs**.
  - Service Windows funcional (`pharma-service`, `OWN_PROCESS`, name `PharmaServer`) — embeds `api::run` en runtime tokio.
  - WiX skeleton (`installer/wix/main.wxs`) con MajorUpgrade y dirs INSTALLFOLDER/DATAFOLDER pero `ServiceComponents` **vacío** — bloqueante MSI.
  - CI windows-latest: fmt + clippy + build --release + test, sube `pharma-api.exe` como artifact (no MSI todavía).
  - Telemetry: `tracing_subscriber` JSON + EnvFilter funciona. **OTLP wiring NO implementado** (config existe, código no exporta).
  - Jobs: scheduler vacío. NATS no usado.
- **Archivos creados en este chunk**:
  - `CLAUDE.md` (raíz)
  - `bitacora.md` (raíz, este archivo)
  - `.claude/hooks/vault-hint.sh`
- **Notas Obsidian creadas** (vault, no repo): `work/active/pharma-server/{index,bitacora,decisions-log-index}.md`, `reference/pharma-server-{architecture,db,api,cli,msi,ci,env}.md`, `brain/pharma-server-{patterns,decisions,gotchas,north-star}.md`.
- **Commits relevantes pre-bitácora**:
  - `a6207c5` feat(cli): implement migrate command with _migrations tracking
  - `234ee1b` refactor(api): expose lib::run; service hosts api in-process
  - `737ad79` feat(scaffold): initial pharma-server workspace
  - `95a60aa` chore: initial commit
- **Commit de este chunk**: `8e80f62` — `chore: scaffold project memory (CLAUDE.md, bitacora, vault hooks)`. Pushed a `origin/feature/pharma-server-scaffold`.
- **Próximos pasos sugeridos** (no compromiso):
  1. Implementar `pharma tenant-create` y `pharma user-create` (CLI stubs).
  2. Llenar `installer/wix/main.wxs` `ServiceComponents` con ServiceInstall + ServiceControl + firewall rule.
  3. Wire OTLP exporter en `crates/telemetry`.
  4. `/health/ready` debería pingear SurrealDB en lugar de devolver `db: "skipped"`.

---

## 2026-05-07 — MSI installer end-to-end + Windows service smoke

- **Qué**: WiX installer completo y verificado. MSI instala servicio + abre firewall + crea data dir. Smoke directo `sc.exe` también verificado.
- **Por qué**: cerrar M3 (MSI shippeable). Antes `ServiceComponents` estaba vacío, ahora produce instalación funcional one-shot.
- **Cambios**:
  - `installer/wix/main.wxs`: `ServiceInstall` (LocalSystem, auto-start, ownProcess), `ServiceControl` (start install / stop both / remove uninstall), `fire:FirewallException` TCP 8080, `DataDirComponents` con GUID explícito (CreateFolder no permite GUID `*`), Version `$(var.Version)` (cargo-wix lo inyecta).
  - `crates/service/Cargo.toml`: `[package.metadata.wix]` con `upgrade-guid`, `path-guid`, `include = ["../../installer/wix/main.wxs"]`, `extensions = ["WixFirewallExtension"]`.
  - Comando build: `cargo wix --package service --no-build --nocapture -C -ext -C WixFirewallExtension -L -ext -L WixFirewallExtension`.
  - WiX v3.14 vía `choco install wixtoolset` (no estaba). cargo-wix 0.3.9 ya estaba.
  - README: secciones "Run as Windows service" + "MSI installer" con install/uninstall msiexec.
- **Smoke directo (sc.exe)**: `create / start / Get-Service Running / curl /health/live → 200 / curl /api/me → 401 / stop / delete` ✓.
- **Smoke MSI**: `msiexec /i pharma-server-0.1.0-x86_64.msi /qn` → service Running, `/health/live` 200, firewall rule "Pharma Server API" Inbound Allow Enabled. `msiexec /x ... /qn` → service y dir y firewall borrados ✓.
- **Gotchas (→ vault `brain/pharma-server-gotchas.md`)**:
  - WiX comments **no pueden contener `--`**. Usar texto sin double-dash.
  - cargo-wix metadata `extensions` no se honra siempre; pasar via CLI `-C -ext -C <Name>` para candle y `-L -ext -L` para light.
  - Componentes con `<CreateFolder/>` (Directory KeyPath) **requieren GUID explícito**, no `Guid="*"`.
  - msiexec: `cmd.exe /c "msiexec /i ..."` necesario en bash MSYS para evitar mangling de paths con `//`.
- **Commits**:
  - `66d0967` feat(installer): WiX ServiceInstall + firewall rule for TCP 8080
  - `db6d71d` fix(installer): MSI builds — explicit data-dir GUID, slim ext list, valid XML comments
  - `4b4d667` docs(readme): add Windows service smoke + MSI install/uninstall sections
- **CI**: run 25525854049 verde con MSI fix commit. README-only commit run en curso (no afecta build).
- **Estado scaffold tras este chunk**: M3 MSI cerrado. Pendiente: `pharma tenant-create` / `user-create`, OTLP wiring, `/health/ready` real DB ping, firmar MSI con cert (sin firma → SmartScreen warning).

---

## 2026-05-07 — CLI tenant/user create + /health/ready DB ping + OTLP wiring

- **Qué**: tres ítems pendientes del scaffold cerrados.
- **`pharma tenant-create <name> [--slug <slug>]`**:
  - `crates/cli/src/main.rs`: `CREATE tenant SET name = $name, slug = $slug RETURN AFTER`, parse `TenantRow` con `surrealdb::sql::Thing` para id.
  - Auto-slug fallback (`slugify`: lowercase + non-alnum→`-` + trim `-`).
- **`pharma user-create --tenant <slug> --email <e> [--roles a,b] [--password <p>]`**:
  - Lookup tenant por slug → record id, hash password con argon2id (`auth::password::hash`), `CREATE user SET tenant=$tenant, email=$email, password=$hash, roles=$roles`.
  - `resolve_password`: prioridad `--password` > `PHARMA_PASSWORD` env > prompt interactivo `rpassword::prompt_password` con confirmación.
  - Dep nueva: `rpassword = "7"` (workspace).
- **`/health/ready` DB ping**:
  - `crates/api/src/lib.rs`: `AppState` ahora carga `db: Option<Arc<db::Db>>`. `api::run` conecta SurrealDB en startup; si falla, log warn y arranca con `None` (ready devolverá `degraded`).
  - `crates/api/src/health.rs`: `ready` ejecuta `handle.query("RETURN 1")`. OK → 200 `{"status":"ok","checks":{"db":"ok"}}`. Err o `db: None` → 503 `degraded`.
  - Test fix: `crates/api/tests/auth.rs` AppState literal añade `db: None`.
- **OTLP wiring**:
  - `crates/telemetry/src/lib.rs`: nueva `init_with_otlp(name, &OtlpConfig)` además de `init(name)`. Si `endpoint` set y no vacío → construye `opentelemetry_otlp::SpanExporter::builder().with_tonic().with_endpoint(...)` + `TracerProvider` con `runtime::Tokio` BatchSpanProcessor + `Resource service.name`. Layer `tracing_opentelemetry::layer().with_tracer(...)` al subscriber chain (vía `Option<Layer>`).
  - `telemetry::shutdown()` llama `opentelemetry::global::shutdown_tracer_provider()`.
  - `crates/api/src/main.rs` y `crates/service/src/main.rs` ahora usan `init_with_otlp` y llaman `telemetry::shutdown()` al exit.
  - Endpoint vacío en `config/default.toml` → tratado como disabled (filter `!s.is_empty()`). Activar con `PHARMA__OTLP__ENDPOINT=http://localhost:4317`.
  - Deps wiring (workspace ya las tenía): `tracing-opentelemetry 0.28`, `opentelemetry 0.27`, `opentelemetry_sdk 0.27` (rt-tokio), `opentelemetry-otlp 0.27` (grpc-tonic).
- **Smokes locales**:
  - `pharma tenant-create "Demo Pharmacy" --slug demo` → `tenant created: id=tenant:9hd373893eo8361wntp4 slug=demo` ✓
  - `PHARMA_PASSWORD=secret123 pharma user-create --tenant demo --email admin@demo.test --roles admin,pharmacist` → user creado con record id ✓
  - `curl /health/ready` → 200 `{"status":"ok","checks":{"db":"ok"}}` ✓
- **Gotchas (→ vault `brain/pharma-server-gotchas.md`)**:
  - `opentelemetry_sdk::trace::Builder::with_config(...)` deprecated en 0.27 → usar `with_resource(resource)` directo.
  - `Layered<...>` no implementa `try_init` cuando se anida con `if let`. Solución: pasar `Option<Layer>` al chain (`tracing_subscriber::registry().with(option_layer)`); tracing-subscriber tiene impl `Layer for Option<L>`.
  - Empty string como endpoint OTLP causa `invalid URI empty string` en tonic. Filtrar `!is_empty()` antes.
- **Commits**:
  - `43d5b7a` feat(cli,api): tenant-create + user-create + /health/ready DB ping
  - `b71b6ff` feat(telemetry): wire OTLP gRPC tracing exporter
- **CI**: 25526940533 verde ✓ (commit `43d5b7a`). 25527505152 (OTLP) en curso (deps grandes ~30min build).
- **Estado scaffold tras este chunk**: cerrados `tenant-create`, `user-create`, `/health/ready` DB ping, OTLP wiring. Pendiente real: firmar MSI con cert (SmartScreen), `/health/metrics` Prometheus, login endpoint que emita JWT, integration tests con DB temporal, MIGRATE en MSI postinstall (hoy CLI manual).

## 2026-05-08 — POST /api/login (JWT issue + session row)

- **Qué**: endpoint `POST /api/login` que valida credenciales y emite JWT.
- **Request**: `{"tenant": "<slug>", "email": "<e>", "password": "<p>"}`.
- **Response 200**: `{"token": "<jwt>", "token_type": "Bearer", "expires_in": <ttl_seconds>}`.
- **Errores**: 401 `{"error":"invalid credentials"}` (tenant inexistente, user inexistente, password mismatch, `active=false`); 503 `{"error":"service unavailable"}` (db `None`, query falla, JWT issue falla).
- **Flujo**: SELECT tenant by slug → SELECT user by `tenant + email` (con `Option<bool>` para `active` para tolerar rows pre-existentes sin default aplicado) → `auth::password::verify` argon2id → `auth::issue` (HS256) → CREATE session SET user, tenant, jti=`uuid::v4`, expires_at (best-effort, log warn si falla pero token emitido).
- **Archivos**:
  - `crates/api/src/routes.rs`: handler `login`, structs `LoginRequest/Response/UserRow/TenantRow`, enum `LoginError` con `IntoResponse`.
  - `crates/api/Cargo.toml`: deps nuevas `surrealdb` + `uuid` (workspace).
  - `crates/api/tests/auth.rs`: test `login_without_db_returns_503`.
- **Smoke local**:
  - `pharma tenant-create "Smoke" --slug smoke` + `PHARMA_PASSWORD=passw0rd pharma user-create --tenant smoke --email smoke@x.cl --roles admin` ✓
  - `curl -X POST http://127.0.0.1:8080/api/login -d '{"tenant":"smoke",...}'` → 200 con token ✓
  - `curl /api/me -H "Authorization: Bearer $TOK"` → 200 con sub/tenant_id/roles ✓
  - bad password → 401 ✓ ; tenant inexistente → 401 ✓
- **Gotcha**: SurrealDB devuelve `active: None` al deserializar si la columna no estaba poblada en CREATE pre-este-deploy; serde decode `expected boolean, found None`. Fix: `Option<bool>` + `#[serde(default)]`. Treat `Some(false)` como inactivo, `None` o `Some(true)` como activo.
- **Tests**: 5 passed (4 prev + nuevo). Clippy clean. Fmt clean.
- **Pendiente**: refresh token, revocación de session (set `revoked=true`), rate limit por tenant+email, login lockout.

## 2026-05-08 — /metrics Prometheus endpoint

- **Qué**: endpoint `GET /metrics` formato exposición Prometheus, prefijo `pharma_`.
- **Implementación** (`crates/api/src/lib.rs`):
  - `PrometheusMetricLayerBuilder::new().with_prefix("pharma").with_ignore_patterns(&["/metrics","/health/live","/health/ready"]).with_default_metrics().build_pair()`.
  - Mount `/metrics` + `.layer(prom_layer)` en `run()` (NO en `build_router` para no romper tests; recorder global solo puede instalarse una vez por proceso).
  - Handler captura clone de `PrometheusHandle` y devuelve `handle.render()`.
- **Métricas expuestas**: `pharma_http_requests_total{method,status,endpoint}`, `pharma_http_requests_pending`, `pharma_http_requests_duration_seconds_{bucket,sum,count}` (default histogram buckets).
- **Smoke**: 3× `GET /` + 1× `POST /api/login` → `/metrics` muestra series correctas, sin entradas para `/metrics` ni `/health/*` (ignored).
- **Gotcha**: `metrics_exporter_prometheus::install_recorder()` panica si llamada ≥2 veces en el mismo proceso. Por eso instalación queda fuera de `build_router` (tests construyen router múltiples veces). Tests no tocan `/metrics`.
- **Builder API**: `PrometheusMetricLayerBuilder` requiere `.with_default_metrics()` (o `.with_metrics_from_fn(...)`) antes de `build_pair()` — sin esa transición de estado, error E0599 "method `build_pair` not found".
- **Tests**: 5 passed (sin cambios).
- **Pendiente**: proteger `/metrics` con auth (hoy abierto), buckets custom afinados a SLO real.

## 2026-05-08 — CLI tenant-list + user-list

- **Qué**: comandos read-only `pharma tenant-list` y `pharma user-list [--tenant <slug>]`, ambos con `--json` opcional.
- **Por qué**: completar admin surface CLI sin abrir GUI; cerrar gap obvio tras tenant-create/user-create.
- **Implementación** (`crates/cli/src/main.rs`):
  - `TenantList { json }` → `SELECT * FROM tenant ORDER BY slug`, output tabla `ID  SLUG  NAME` o JSON pretty.
  - `UserList { tenant, json }` → si `--tenant`, lookup tenant por slug + `SELECT * FROM user WHERE tenant = $tenant`; sin filtro, `SELECT * FROM user`. Output tabla `ID  EMAIL  TENANT  ROLES` o JSON pretty.
  - Reutiliza `TenantRow` / `UserRow` ya definidos.
- **Gotcha clippy**: `println!("{:<40} {}", "ID", "NAME")` con literal final dispara `clippy::print_literal` (`-D warnings` lo convierte en error). Fix: inlinear el último literal en la format string → `println!("{:<40} NAME", "ID")`.
- **Verificación**: `cargo build --workspace --release` (27m46s, OK), `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean tras fix print_literal, `cargo test --workspace` 5 passed.
- **Pendiente**: paginación / filtros adicionales (`--role`, `--limit`) si surface crece.

## 2026-05-08 — Integration tests con DB temporal (SurrealKv tempdir)

- **Qué**: 4 tests integración nuevos en `crates/api/tests/integration_db.rs` que arrancan SurrealKv real sobre `tempfile::TempDir`, corren migraciones, siembran tenant + user, e invocan handlers axum vía `tower::ServiceExt::oneshot`.
- **Por qué**: cubrir end-to-end `/health/ready`, `/api/login` y `/api/login → /api/me` con DB real; los unit tests existentes con `db: None` solo validaban paths degradados (503, 401).
- **Helper**: `spawn_test_db()` → tempdir + `db::connect` + `db::run_migrations("../../migrations")` + retiene `TempDir` en struct para cleanup auto al drop. `seed_tenant_and_user(db, slug, email, password)` ejecuta `CREATE tenant`/`CREATE user` con `auth::password::hash`.
- **Tests**:
  - `health_ready_with_db_returns_200` → 200 con `checks.db == "ok"`.
  - `login_with_valid_creds_returns_jwt` → 200 + payload con `token`, `token_type=Bearer`, `expires_in>0`.
  - `login_with_bad_password_returns_401` → 401.
  - `login_then_me_round_trip` → login → token → `Bearer <token>` en `/api/me` → 200 con `sub`, `tenant_id`, `roles[0]=admin`.
- **Dev-deps**: `tempfile = "3"` añadido a `crates/api/Cargo.toml` (ya teníamos `http-body-util` y `tower`).
- **CWD migrations**: `cargo test` corre con cwd = manifest dir (`crates/api/`), por eso `MIGRATIONS_DIR = "../../migrations"`.
- **Aislamiento**: cada test crea su propio tempdir, evita el SurrealKv file lock entre tests paralelos.
- **fmt gotcha**: tras fix `clippy::print_literal` en `cli/main.rs` (línea con `println!(...)` multilínea) `cargo fmt` re-junta a una sola línea — dejar la nueva forma single-line.
- **Verificación**: `cargo build --workspace --release` OK · `cargo fmt --check` clean · `cargo clippy --workspace --all-targets -- -D warnings` clean · `cargo test --workspace` 9 passed (5 auth + 4 integration_db).
- **Pendiente**: tests para session row creada por login, revoked path (logout), `/metrics` content-type smoke, performance smoke con N=100 logins.

## 2026-05-14 — /metrics protegido con bearer token compartido

- **Qué**: endpoint `/metrics` ahora requiere `Authorization: Bearer <token>` con token vía config. Antes: abierto, expone counters a cualquiera en LAN.
- **Por qué**: cerrar surface; producto se despliega en LAN de farmacia, scrapers (Prometheus) viven en misma red → bearer compartido suficiente (no necesitan JWT rotativo).
- **Modelo**: token-shared en `[metrics] token = ""` (`crates/core/src/config.rs` añade `MetricsConfig { token: Option<String> }` con `#[serde(default)]`). Empty string → tratado como `None` → 401. Producción inyecta vía `PHARMA__METRICS__TOKEN`.
- **Implementación** (`crates/api/src/lib.rs`):
  - Campo `metrics_token: Option<String>` en `AppState`.
  - `/metrics` movido a sub-router con `.with_state(state.clone())` para extraer `State<AppState>`.
  - Handler: `authorize_metrics(state, headers) -> Result<(), (StatusCode, &'static str)>` valida bearer.
  - Comparación con `constant_time_eq` (XOR loop, side-channel resistant) para evitar timing leak.
  - Si `metrics_token == None` → log `warn!` al arrancar + `/metrics` siempre 401 ("metrics endpoint not configured"). Cerrado-por-defecto seguro.
- **Tests nuevos** (`#[cfg(test)] mod tests` en `lib.rs`, 5 unit):
  - `metrics_no_token_configured_returns_401`
  - `metrics_missing_header_returns_401`
  - `metrics_wrong_token_returns_401`
  - `metrics_correct_token_ok`
  - `constant_time_eq_works`
- **Gotcha clippy**: primera versión devolvía `Result<(), axum::response::Response>` → `clippy::result_large_err` (`-D warnings` lo convierte en error: variant ≥128 bytes). Fix: devolver `Result<(), (StatusCode, &'static str)>` y construir Response en el handler caller.
- **Gotcha disco**: build full debug pasó de PDB limit (LNK1140) y luego ENOSPC sobre target/ (12G). `cargo clean` (-9.6GiB) + `cargo test --release`. Sustituir debug compiles por `--release` en este host hasta migrar a disco más grande / `target/` separado.
- **Verificación**: `cargo build --workspace --release` OK · `cargo fmt --check` clean · `cargo clippy --workspace --all-targets -- -D warnings` clean · `cargo test --workspace --release` 14 passed (5 unit lib + 5 auth + 4 integration_db).
- **Pendiente**: documentar token rotation flow, CLI helper `pharma metrics-token --rotate`, MSI install puede generar token aleatorio al installtime y persistirlo en config local.

---

## 2026-05-15 — Fase 1 erp-parity foundation (epic feature/erp-parity)

- **Qué**: arranque del epic ERP-parity (portar API+dominio de Tu Farmacia → pharma-server). Branch `feature/erp-parity` desde scaffold HEAD. Plan completo en `docs/erp-parity-prompt.md` (31 modelos, 87 rutas, 9 fases).
- **Por qué**: pharma-server debe alcanzar paridad funcional ERP con la app live de Tu Farmacia, vendible como MSI on-prem genérico. No se porta frontend Next.js — solo API HTTP/JSON versionada `/api/v1`.
- **Decisiones §4 (10) documentadas** en vault `brain/pharma-server-decisions.md` (sección "ERP-parity epic"): barcode_catalog global, rust_decimal money, CLP hardcode, SII stub 501, OCR stub 501, idempotency_key TTL 24h, FEFO lotes, backup `.surql.gz`, LIVE diferido, tests Surreal Mem.
- **Crate nuevo `domain`**: 10 submódulos bounded-context (catalog, inventory, sales, purchasing, finance, customers, prescriptions, operations, settings, reports) + `DomainError` (thiserror, `.code()` SCREAMING_SNAKE) + `money` (rust_decimal, CURRENCY_CLP, IVA_DEFAULT_PERCENT). Solo scaffold (cada fase llena su contexto).
- **Error envelope** (`crates/api/src/error.rs`): `{ error: { code, message, details? } }`. Códigos EN SCREAMING_SNAKE (contrato estable), mensajes ES user-facing. `ApiError` + helpers. `LoginError`/`AuthError` refactorizados sobre él (eliminado enum LoginError ad-hoc).
- **Versionado API**: rutas canónicas `/api/v1/{me,login}`; alias `/api/{me,login}` mantenido 1 release por compat.
- **RequireRole** (`middleware/role.rs`): verifica JWT + intersección de roles vs allowlist `&'static [&'static str]`. 403 FORBIDDEN envelope. Patrón `Stack<Extension<AllowedRoles>, FromFnLayer>` para no monomorfizar por call site.
- **AuditLayer** (`middleware/audit.rs`): POST/PATCH/PUT/DELETE → `tokio::spawn` insert detached en `audit_log` (nunca bloquea response path). Hash sha256 del body. Best-effort: DB caída / sin JWT → request sigue, row skip + warn.
- **Migración** `0002_audit_log.surql`: `audit_log` SCHEMAFULL multi-tenant (tenant record<tenant>, user option<record<user>>, method, path, status, payload_hash, ip, user_agent, created_at) + índices compuestos `(tenant,created_at)`, `(user,created_at)`, `(path,created_at)`. Append-only enforce a nivel app.
- **docs/parity-prisma-models.md**: inventario completo 31 modelos Prisma (campos/tipos/índices/relaciones) + cheatsheet Postgres→SurrealDB + overlay multi-tenant.
- **Fix build durable (resuelve gotcha LNK1140/disco previo)**: `[profile.dev] debug = "line-tables-only"` + `[profile.dev.package."*"] debug = false` + `[profile.test.package."*"] debug = false`. El grafo debug completo de surrealdb desbordaba el límite por-PDB de MSVC (LNK1140) y llenaba disco (C: quedó con 82MB libres). Esto reduce PDBs ~10x manteniendo backtraces en crates del workspace. Sustituye el workaround `--release-only`: ahora `cargo test --workspace` debug linkea bien.
- **Versión**: `workspace.package.version` 0.1.0 → 0.1.1 (patch por fase, regla 11).
- **Verificación**: `cargo fmt --all -- --check` clean · `cargo clippy --workspace --all-targets -- -D warnings` clean · `cargo test --workspace` verde (api lib 12, auth 5, integration_db 7 incl `mutation_writes_audit_log_row`, `bad_credentials_use_error_envelope`, `v1_alias_login_and_me_work`).
- **Commit**: `00c9ef6 feat(domain,api): Fase 1 erp-parity foundation`.
- **Pendiente**: Fase 2 Catalog (migración 0003, product/category/barcode endpoints).

---

## 2026-05-15 — Fase 2 erp-parity: Catalog (epic feature/erp-parity)

- **Qué**: catálogo productos/categorías/códigos. Migración `0003_catalog.surql`, crate `domain::catalog` (model/repo/service), endpoints `/api/v1` products+categories+etiquetas, tests integración Mem.
- **Migración 0003** (SCHEMAFULL, append-only): `category`, `product`, `product_barcode` tenant-scoped (índice compuesto líder por `tenant`); `barcode_catalog` + `therapeutic_category_mapping` GLOBALES sin tenant (catálogo Chile compartido — decisión Fase 0). Índices product: `(tenant,slug)` UNIQUE + `(tenant,active|category|laboratory|external_id|stock)`.
- **domain::catalog** dir-module: `model.rs` (DTOs/inputs `ToSchema`, money `#[serde(with="rust_decimal::serde::str")]` + `#[schema(value_type=String)]`), `repo.rs` (queries tenant-scoped puras), `service.rs` (slug auto, validación categoría, bulk-price, stock). Reemplaza `catalog.rs` flat.
- **Endpoints** `/api/v1`: products GET(filtros search/category/active/low_stock)·POST·GET/PATCH/DELETE :id·POST :id/stock·import(CSV multipart)·export(CSV)·bulk-price·stats; categories GET/POST·GET/PATCH/DELETE :id; etiquetas/search. Lecturas = AuthUser; mutaciones = `role::layer(["admin","owner"])`.
- **Decisiones §4 nuevas** (vault `brain/pharma-server-decisions.md`): (1) DELETE = soft-delete `active=false` (auditoría ISP, refs futuras order_item/stock_movement). (2) Swagger UI diferido a Fase 8; anotar `ToSchema`/`utoipa::path` ahora. (3) `POST products/:id/stock` escribe `product.stock` directo; `stock_movement` auditado llega Fase 3. (4) `POST products/update-prices` → **501** (depende `supplier_price_list`, Fase 5). (5) `stats.expired`=0 hasta `product_batch` (Fase 3).
- **Gotcha decimal binding**: `rust_decimal` con feature `serde-with-str` serializa Decimal como string → SurrealQL `decimal` schema rechaza el bind (`FieldCheck ... check:"decimal"`). Fix durable: helpers `dec_val`/`dec_opt` en `repo.rs` convierten a `surrealdb::sql::Number::from(d).into()` (Value nativo). Round-trip verificado en test `decimal_round_trips_through_db`.
- **Gotcha clippy result_large_err**: `DomainError::Db(surrealdb::Error)` infla `DomainResult` (>128B) → lint en cada fn repo. Fix: `Db(Box<surrealdb::Error>)` + `impl From<surrealdb::Error>` manual (thiserror `#[from]` no auto-boxea).
- **kv-mem test-only**: `surrealdb = { workspace=true, features=["kv-mem"] }` en `[dev-dependencies]` de `domain` (no infla binario shippeado; feature unifica solo en build de tests).
- **Tests** (`crates/domain/tests/catalog.rs`, Mem + migraciones reales): slug auto+colisión, decimal round-trip + JSON string, filtros + soft-delete, bulk-price percent/amount + round, stats agregados, category CRUD + link + validación, **aislamiento por tenant**. 7 pass + 1 unit slugify.
- **Versión**: 0.1.1 → 0.1.2 (patch por fase, regla 11).
- **Deps**: workspace `axum` += `multipart`; `csv = "1.3"`; `api` += `domain`.
- **Verificación**: `cargo fmt --all -- --check` clean · `cargo clippy --workspace --all-targets -- -D warnings` clean · `cargo test --workspace` verde (domain 7+1, api 12, auth 5, integration_db 7) · `cargo build --workspace --release` OK.
- **Pendiente**: Fase 3 Inventory (migración 0004, stock_movement/product_batch/falta, ABC, reorder, FEFO).

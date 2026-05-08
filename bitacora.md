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

# bitácora — pharma-server

Registro cronológico de decisiones técnicas, cambios significativos e incidentes.
Formato: `## YYYY-MM-DD — título corto` + bullets `qué / por qué / archivos / commit`.
Espejada en vault: `C:/Users/Administrator/Documents/obsidian-mind/work/active/pharma-server/bitacora.md`.

Estructura: **ESTADO ACTUAL** (top, se sobrescribe cada sesión — single source of
truth) → **BACKLOG** (lista priorizada única) → **log append-only** (histórico,
NO se edita). Gotchas viven en memoria + vault `brain/pharma-server-gotchas.md`,
NO acá.

---

## ESTADO ACTUAL

> Sobrescribir este bloque entero cada sesión. Es la verdad presente del proyecto.

- **Versión**: `0.1.8` (workspace `Cargo.toml`).
- **Branch**: `feature/erp-parity-po-status` (PR → `feature/erp-parity`).
- **MSI release**: https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.8
- **Funciona end-to-end**:
  - ERP local: inventario (SKU/lote/vencimiento), POS atómico single-tx con
    decremento FEFO de lotes, idempotencia por `Idempotency-Key`, loyalty.
  - **Devoluciones/refunds**: `POST /api/v1/pos/returns` atómico (devolucion +
    devolucion_item + restock opcional vía stock_movement; marca order
    `refunded`; rechaza sobre-devolución). `GET /api/v1/returns` filtrable.
  - Multi-tenant por JWT claim `tenant_id`; auth JWT HS256 + argon2id.
  - SurrealDB embedded `kv-surrealkv`, migraciones append-only con tracking.
  - **Service corre migraciones al arrancar** desde schema embebido (fix
    first-run: instalación limpia ya queda healthy sin tocar la CLI).
  - MSI instalable: ServiceInstall + ServiceControl + firewall TCP 8080.
  - Ecosistema agentes federado: identidad Ed25519 / DID, Envelope firmado
    canonical-JSON, `POST /agent/inbox` (ping, catalog.lookup, quote.request,
    po.create) — opt-in por tenant (`admin_setting federation_enabled`).
  - **`po.create` re-cotiza contra catálogo del proveedor** (no confía en
    `unit_price` del comprador; `price_adjusted` persistido en `agent_order`).
  - **`po.status`**: el comprador consulta el estado/decisión de su orden
    (`{status,total,currency,price_adjusted}`), scoped a su propio DID.
- **Falta para v1.0.0 vendible**: firma cert Authenticode (anti-SmartScreen) +
  smoke install/uninstall en VM limpia (Fase 9).
- **Tests**: workspace verde (`cargo test --workspace`), incluye 14 tests
  `sales` (devoluciones) + 11 `agent_inbox` (2 nuevos de `po.status`).

---

## BACKLOG

> Lista priorizada única. Consolidé acá todos los "Pendiente" dispersos del log.

1. **Fase 9 — MSI vendible v1.0.0**: firma Authenticode con cert + smoke
   install/uninstall en VM Windows limpia (sin firma → SmartScreen warning).
2. **Order fulfillment/settlement agente**: `po.status` ✅ (comprador consulta
   decisión). Falta: `po.accept`/`po.reject` (acción local del operador
   proveedor sobre `agent_order`) + `po.fulfill` (descuento real de stock vía
   sales/inventory) — cierre del handshake comprador↔proveedor.
3. **Multi-lot split traceability**: hoy `order_item.batch` persiste solo el
   lote primario; falta desglose por lote cuando una línea consume varios lotes.
4. **Drug-interactions ruleset port** (~370 LoC Beers + Vademécum CL).
5. **Prescription desde POS**: crear receta retenida/cheque/controlados ligada
   a la venta (modelo parcial; falta link POS).
6. **Relay offline-peer**: cola/relay para nodos federados sin conexión directa.
7. **Fase 10 — sync ERP online opt-in** entre nodos (replicación datos → v1.1.0).
8. **Fase 5-full**: PO local + recepción + costo promedio ponderado (WAC) + AP.
9. **Fase 6**: caja (apertura/cierre/arqueo) + gastos + reportes
   (ventas/márgenes/rotación/ABC/vencimientos).
10. **Fase 8**: cron jobs + backup programado SurrealKv + restore guiado +
    Swagger UI + desktop Tauri.
11. **Fase 12 — marketplace de confianza** (`docs/marketplace-master-plan.md`,
    branch `feature/marketplace-master-plan`): capa B2B sobre el protocolo
    federado firmado. Estrategia/locked decisions, sin scaffold aún.

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

## 2026-05-15 — Fase 3 erp-parity: Inventory (epic feature/erp-parity)

- **Qué**: stock_movement audit trail, product_batch (lotes/vencimiento), falta (productos a reponer), inventory summary + ABC + reorder suggestions, FEFO planner, retrofit catalog adjust_stock. Migración `0004_inventory.surql`, crate `domain::inventory` dir-module (model/repo/service), endpoints `/api/v1` stock-movements+batches+faltas+inventory+abc+reorder, tests integración Mem.
- **Migración 0004** (SCHEMAFULL, append-only): `stock_movement` (delta int ASSERT !=0, reason, admin opt<record<user>>, ref opt<string>) + índices `(tenant,product,created_at)`,`(tenant,created_at)`,`(tenant,reason)`. `product_batch` (batch_code, expiry_date datetime, stock int>=0, cost opt<decimal>, active bool DEFAULT true) + índices `(tenant,product,expiry_date)`,`(tenant,batch_code)`,`(tenant,active,expiry_date)`. `falta` (product opt<record<product>>, name, qty>0, resolved bool DEFAULT false) + índices `(tenant,resolved,created_at)`,`(tenant,product)`.
- **domain::inventory** dir-module: `model.rs` (DTOs `ToSchema`, money string-serde, FEFO `FefoAllocation`, `AbcReport`/`ReorderReport` con campo `method` documentando algoritmo); `repo.rs` (queries puras, helpers `dec_val/dec_opt/dt_val/dt_opt`); `service.rs` (gating tenant + negative-stock + reason + admin parsing). Reemplaza `inventory.rs` flat.
- **Endpoints** `/api/v1`: GET/POST stock-movements · POST stock-movements/adjust · POST stock-movements/import (CSV multipart cols `product,delta,reason,ref`) · GET/POST batches · GET/PATCH/DELETE batches/:id (soft-delete) · GET/POST faltas · PATCH faltas/:id · GET inventory (summary) · GET inventory/abc · GET inventory/reorder-suggestions. Lecturas = AuthUser; mutaciones = `role::layer(["admin","owner"])`.
- **Decisión clave — stock invariante**: `product.stock = SUM(stock_movement.delta)` materializado, mantenido vía SurrealQL multi-statement `BEGIN; CREATE stock_movement...; UPDATE product SET stock = stock + $d ...; COMMIT;` en `repo::apply_movement`. Audit trail y contador no pueden divergir. Pre-checks (tenant ownership + non-negative resultado + non-zero delta + reason no vacío) en `service::add_movement`. `repo::set_stock` eliminado del catalog — toda escritura de stock ahora pasa por inventory.
- **Retrofit `catalog::service::adjust_stock`**: ahora delega en `inventory::service::add_movement` con `reason = adj.reason ?? "manual_adjust"` y `admin = JWT.sub` (parseado vía `surrealdb::sql::thing`). API handler thread `Some(&claims.sub)`. Behavior preservado para callers (mismo `ProductDto`); ahora cada ajuste deja fila en `stock_movement`.
- **FEFO helper público** `inventory::service::plan_fefo(db, &tenant, product_id, qty) -> Vec<FefoAllocation>`: read-only, ordena `product_batch` por `expiry_date ASC, created_at ASC` filtrando `active=true AND stock>0 AND expiry_date>=now`, allocates greedy. Devuelve `Err(InsufficientStock)` si total < qty. Lo usará Fase 4 sales POS — sales escribe decrementos + emite `stock_movement(-delta, reason="sale")` en su propia tx.
- **Recepción de lote**: `POST /api/v1/batches` con `stock>0` crea `product_batch` Y emite `stock_movement(+delta, reason="batch_received", ref=batch_code, admin=JWT.sub)` Y actualiza `product.stock` en MISMA transacción multi-stmt (`repo::create_batch_atomic`). Costo promedio ponderado se posterga a Fase 5.
- **ABC + reorder stubs documentados**: `AbcReport.method = "value_stock_fallback"` (ordena por `stock*(cost_price ?? 0)`, breakpoints A≤80% / B≤95% / C resto cumulativo); switchea a `"sales_90d"` cuando exista historial Fase 4. `ReorderReport.method = "low_stock_stub"` (productos `stock<=LOW_STOCK_DEFAULT` → suggested = `2*low - stock`); switchea a `"avg_daily_sales*lead_time + safety - stock"` post-Fase 4.
- **Gotcha datetime binding**: `chrono::DateTime<Utc>` por serde default va como string ISO → SurrealQL `datetime` schema rechaza (`FieldCheck ... check:"datetime"`). Fix paralelo al de decimal: helpers `dt_val(dt)` = `surrealdb::sql::Datetime::from(dt).into()` y `dt_opt`. Aplicado en create_batch, update_batch, list_movements (from/to filters).
- **Gotcha SurrealQL ORDER BY projection**: Surreal 2.1 exige que el campo de `ORDER BY` esté en la lista de proyección del SELECT (`Missing order idiom \`expiry_date\` in statement selection`). FEFO query incluye `id, stock, expiry_date, created_at` aunque solo `id`+`stock` se deserialicen.
- **Tests** (`crates/domain/tests/inventory.rs`, Mem + migraciones reales, 10 tests): movement materializa stock atómico (positivo+negativo) · negative resulting stock blocked (estado intacto) · zero delta rejected · catalog::adjust_stock emite movement con reason custom · batch creation emite `batch_received` movement · FEFO ordena por expiry_date+created_at, devuelve plan greedy, falla `InsufficientStock` · batch soft-delete preserva fila (`active=false`) · faltas CRUD + filtro resolved · tenant isolation (movements no leak + cross-tenant mutate falla `NOT_FOUND`) · summary agrega products+batches+faltas (skus, low_stock, expiring_soon, open_faltas).
- **Versión**: workspace **NO bumpeada** (integrador subirá 0.1.2→0.1.3 al mergear los 3 PRs A→B→C).
- **Deps**: ninguna nueva (axum multipart + csv ya estaban desde Fase 2).
- **Verificación**: `cargo fmt --all -- --check` clean · `cargo clippy --workspace --all-targets -- -D warnings` clean · `cargo test --workspace` verde (domain 1 unit + 7 catalog + 10 inventory, api 12, auth 5, integration_db 7) · `cargo build --workspace --release` OK · `CARGO_TARGET_DIR=target-shared` para coexistir con worktrees B/C.
- **Handoff a integrador**: `crates/api/src/v1/mod.rs` añadió `pub mod inventory;` + `.merge(inventory::router(state))` (conflicto trivial esperado al integrar B/C). API pública para Fase 4 sales: `inventory::service::plan_fefo(&Db, &Thing, &str, i64) -> DomainResult<Vec<FefoAllocation>>` y `inventory::service::add_movement(&Db, &Thing, &str, i64, &str, Option<&str>, Option<String>) -> DomainResult<(StockMovementDto, ProductDto)>`.
- **Pendiente**: Fase 4 Sales+POS (migración 0005, order/order_item/return_doc, POST /pos/sale con FEFO + stock_movement + idempotencia, budget <50ms p99).

## 2026-05-15 — Fase 7-subset Customers/Prescriptions (epic feature/erp-parity, Agente B paralelo)

- **Qué**: cliente farmacia, scaffold loyalty, prescripciones inmutables (Ley 20.000), turnos químico farmacéutico. Migración `0005_customers.surql`, dir-modules `domain::customers` y `domain::prescriptions`, endpoints `/api/v1` clientes/loyalty/prescriptions/libro-recetas/turnos-farmaceutico, tests integración Mem.
- **Migración 0005** (SCHEMAFULL, append-only): `customer` tenant-scoped (loyalty_points int DEFAULT 0, active bool DEFAULT true, idx `(tenant,rut)`+`(tenant,name)` no-unique); `loyalty_transaction` (`tenant,customer,delta,reason,ref?,created_at`, idx `(tenant,customer,created_at)`) — append-only nivel app, sales Fase 4 escribe; `prescription` (`product?,customer?,patient_name,patient_rut,doctor_name?,doctor_rut?,controlled DEFAULT false,folio?,dispensed_at,created_at`, idx `(tenant,patient_rut,created_at)`+`(tenant,controlled,created_at)`) — INMUTABLE nivel app (sólo CREATE+SELECT); `pharmacist_shift` (`tenant,user,started_at,ended_at?,notes?`, idx `(tenant,user,started_at)`).
- **domain::customers** dir-module (model/repo/service) reemplaza `customers.rs` flat. RUT normalización (trim, drop `. -`, uppercase) + uniqueness por tenant enforced en `service::create/update_customer` (UNIQUE en idx Surreal rejected: trata múltiples NONE como duplicados con `option<string>`).
- **domain::prescriptions** dir-module reemplaza `prescriptions.rs` flat. `service::create_prescription` valida `controlled=true → doctor_name+doctor_rut obligatorios`. NO se expone `update_prescription` ni delete (Ley 20.000). Helper `list_controlled` para libro-recetas.
- **Endpoints** `/api/v1`: clientes GET(filters search/active)·POST·GET/PATCH/DELETE :id (soft-delete); loyalty GET (LoyaltyFilters)·loyalty/stats (read-only, `pending_sales_integration=true` hasta Fase 4); prescriptions GET·POST·GET :id; libro-recetas GET·export (CSV controlados); turnos-farmaceutico GET·POST·PATCH :id (cerrar turno). Lecturas = `AuthUser`; mutaciones clientes/shift = `role::layer(["admin","owner"])`; mutación prescriptions = `role::layer(["admin","owner","pharmacist"])` (química dispensa).
- **Gotcha datetime binding**: `chrono::DateTime<Utc>` serializa serde como string RFC3339 → SurrealQL `datetime` schema rechaza bind (`FieldCheck ... check:"datetime"`). Misma clase de bug que `decimal` en Fase 2. Fix durable: helpers `dt_val`/`dt_opt` en `prescriptions/repo.rs` envuelven en `surrealdb::sql::Datetime::from(d).into()` (Value nativo). Aplicado a `dispensed_at`, `from`/`to` filters, `started_at`, `ended_at` update.
- **Tests** (Mem + migraciones reales): customers (6) — create+get, RUT único por tenant, soft-delete, aislamiento por tenant (mismo RUT OK cross-tenant), loyalty_stats vacío hasta sales, update rechaza colisión RUT; prescriptions (5) — create+read, controlled exige doctor (+ aparece en `list_controlled`), aislamiento por tenant, shift open/close, close-twice → CONFLICT. Total: domain libtests 2, catalog 7, customers 6, prescriptions 5.
- **Versión**: NO bump (integrador A→B→C sube 0.1.2→0.1.3 al mergear los 3 PRs paralelos).
- **Deps**: ninguna nueva (csv ya está, axum multipart no usado aquí).
- **HANDOFF integrador**: `crates/api/src/v1/mod.rs` declara `pub mod customers; pub mod prescriptions;` y merges; `crates/domain/src/lib.rs` ya tenía `pub mod customers; pub mod prescriptions;` (sin cambios). Sales Fase 4 escribirá `loyalty_transaction` (acumulación puntos por order) y opcionalmente vincula `prescription.product` en POS sale de medicamento controlado.
- **Verificación** (worktree `..\ps-cust`, target compartido `pharma-server/target-shared`): `cargo fmt --all -- --check` clean · `cargo clippy --workspace --all-targets -- -D warnings` clean · `cargo test --workspace` verde (catalog 7, customers 6, prescriptions 5, api 12, auth 5, integration_db 7, unit 2) · `cargo build --workspace --release` OK (17m29s).
- **Pendiente**: Fase 4 sales (`order/order_item/return_doc`, POS sale, loyalty accumulation, FEFO consumption, prescription link).

---

## 2026-05-15 — Fase 5-subset Suppliers/Price-lists (epic feature/erp-parity)

- **Qué**: slice paralelo Agente C (worktree `ps-supp`, branch `feature/erp-parity-suppliers`). Suppliers + supplier_product_mapping + supplier_price_list + compare-best-cost. Migración `0006_suppliers.surql`, crate `domain::purchasing` dir-module, endpoints `/api/v1/suppliers` + `/api/v1/supplier-prices`, tests integración Mem. **Fuera de scope**: `purchase_order` / `purchase_order_item` / `purchase_payment` / `receive` (dependen de inventory Fase 3 — stock_movement/product_batch + costo promedio ponderado).
- **Migración 0006** (SCHEMAFULL, append-only): `supplier` (tenant, name, rut, contact_*, default_invoice_format, active default true; índices `(tenant,rut)`, `(tenant,name)`); `supplier_product_mapping` (tenant, supplier, product, supplier_code; índices `(tenant,supplier,supplier_code)` **UNIQUE**, `(tenant,product)`); `supplier_price_list` (tenant, supplier, product `option`, supplier_code `option`, description, unit_cost decimal ≥0, currency default 'CLP', valid_from default `time::now()`; índices `(tenant,supplier,created_at)`, `(tenant,product,created_at)`). `product` es `option` en price_list para permitir líneas solo-supplier_code antes de mapping.
- **domain::purchasing** dir-module (reemplaza flat `purchasing.rs`): `mod/model/repo/service`. Money idéntico patrón catalog: `unit_cost` DTO `#[serde(with="rust_decimal::serde::str")]` + `#[schema(value_type=String)]`; bind via helper `dec_val`. `service.rs` valida `parse_typed(id,"supplier"|"product")`, resuelve tenant scope antes de crear price/mapping. **CONFLICT mapping unique**: surfaceado como `DomainError::Conflict` mapeando `Db(surreal::Error)` cuyo mensaje contiene `unique|index|already` → 409 en lugar de 500.
- **Endpoints** `/api/v1`: GET/POST `/suppliers`, GET/PATCH/DELETE `/suppliers/{id}` (DELETE soft `active=false`), POST `/suppliers/{id}/map-product`, GET/POST `/supplier-prices`, POST `/supplier-prices/compare`, POST `/supplier-prices/import` (CSV multipart). Lecturas = `AuthUser`; mutaciones = `role::layer(["admin","owner"])`. `compare`: por item con `product` → busca min `unit_cost` en `supplier_price_list` del tenant + computa `savings = product.cost_price − best.unit_cost` (si `cost_price` existe); con `supplier_code` → min `unit_cost` cross-supplier, savings `None`. CSV import (header-based, columnas flexibles): `supplier|supplier_code|product|description|unit_cost|currency|valid_from`; `?supplier=...` query como default si CSV no trae columna. Resumen `{created,failed,errors[]}` patrón idéntico a `import_products`.
- **Decisiones nuevas** (vault `brain/pharma-server-decisions.md`): (1) Subset Fase 5 sin OC: lo dependiente de inventory (stock_movement/product_batch + WAC) se difiere; entregar valor inmediato (catálogo proveedores + comparador precios + import) sin bloquear por Fase 3. (2) `supplier_price_list.product` opcional: el comprador suele recibir listas con supplier_code antes de mapearlas. (3) Mapping unique on `(tenant, supplier, supplier_code)` mapea Db-error → `Conflict` (409) por DX en handler/UX.
- **Gotcha datetime binding**: chrono `DateTime<Utc>` serializa RFC3339 string → `FieldCheck check:"datetime"` al bind. Fix durable simétrico al decimal: `surrealdb::sql::Datetime::from(dt)` antes del `.bind`. Documentado en `purchasing/repo.rs::create_price`.
- **Gotcha SurrealDB Response::take + serde flatten**: `SELECT *, supplier.name AS supplier_name FROM ... LIMIT 1` con `Joined { #[serde(flatten)] row: PriceRow, supplier_name }` falla con `Serialization("untagged and internally tagged enums do not support enum input")` por `Option<Thing>` en `PriceRow`. Workaround estable: dos queries — `SELECT * FROM supplier_price_list ...` → `PriceRow`, luego `SELECT name FROM $supplier_thing` → `String`. Helper privado `supplier_name(db, &Thing)`. Costo: +1 roundtrip por compare-item, aceptable en hot path no-POS.
- **Tests** (`crates/domain/tests/purchasing.rs`, Mem + migraciones reales): supplier CRUD + soft-delete; price_list decimal round-trip (12345.67) + JSON string; compare elige menor `unit_cost` + computa savings (cheap 700 vs current cost 900 → savings 200); compare por supplier_code sin product (savings `None`); mapping unique `(tenant,supplier,supplier_code)` → `CONFLICT`; **aislamiento por tenant**. 6 pass.
- **Versión**: NO bumpeada en este slice; integrador A→B→C subirá 0.1.2→0.1.3 al mergear los tres PRs.
- **Deps**: NO se tocó workspace `[dependencies]` (multipart axum + csv ya presentes desde Fase 2). NO se tocó `api/Cargo.toml`.
- **Verificación** (CARGO_TARGET_DIR shared `pharma-server/target-shared`): `cargo fmt --all -- --check` clean · `cargo clippy --workspace --all-targets -- -D warnings` clean · `cargo test --workspace` verde (domain 13+1, api 12, auth 5, integration_db 7) · `cargo build --workspace --release` OK.
- **Handoff integrador**: conflicto trivial esperado en `crates/api/src/v1/mod.rs` (este slice añade `pub mod purchasing;` + `.merge(purchasing::router(state))` sobre el `pub mod inventory;` de Agente A y `pub mod customers;` de Agente B). `migrations/0006_*` no colisiona (Agente A usa 0004, Agente B 0005).
- **Pendiente**: purchase_order + purchase_order_item + receive + AP (`purchase_payment`) + OCR `scan-invoice` (501) — todos requieren inventory Fase 3 entregada antes.

---

## 2026-05-16 — Integración Fases 3 + 7-subset + 5-subset (epic feature/erp-parity)

- **Qué**: merge A→B→C de los 3 PRs paralelos (#5 inventory, #3 customers, #4 suppliers) en branch integradora `feature/erp-parity-merge` y fast-forward a `feature/erp-parity`. Branches slice borradas tras consolidación.
- **Por qué**: 3 agentes paralelos cerraron Fase 3 + 7-subset + 5-subset sobre el mismo base (`feature/erp-parity` post-Fase 2). Orden de merge dicta lineage limpio y minimiza conflictos: Fase 3 (Inventory) primero porque Fase 4 sales depende; B y C son independientes entre sí.
- **Conflictos resueltos**:
  - `crates/api/src/v1/mod.rs` (2 conflicts esperados): consolidar a 5 `pub mod` (catalog, inventory, customers, prescriptions, purchasing) + `.merge(...)` encadenados con `state.clone()`.
  - `bitacora.md` (2 conflicts): concat de las 3 secciones agente en orden cronológico, preservando contenido íntegro.
  - `Cargo.lock`: regen automático al `cargo build` post-merge (no conflicto real, solo CRLF stash).
- **Versión**: workspace `0.1.2 → 0.1.3` (patch por epic, regla 11). Commit aparte `54aaafb`.
- **Pre-commit verde** (CARGO_TARGET_DIR `pharma-server/target-shared`): `cargo fmt --all --check` clean · `cargo clippy --workspace --all-targets -- -D warnings` clean · `cargo test --workspace` verde (domain unit 3, catalog 7, inventory 10, customers 6, prescriptions 5, purchasing 6, api 12, auth 5, integration_db 7 = 61 tests) · `cargo build --workspace --release` OK (6m09s).
- **Commits merge**: `750e2a6` Merge A · `ffc6891` Merge B · `1a55e66` Merge C · `54aaafb` bump 0.1.3.
- **PRs**: #3 #4 #5 marcados auto-merged por GitHub al ff `feature/erp-parity`. Branches remotas + locales `feature/erp-parity-{inventory,customers,suppliers,merge}` borradas. Worktrees `ps-merge`/`ps-cust`/`ps-supp` removidos.
- **Gotchas nuevos confirmados durales** (registrar en vault `brain/pharma-server-gotchas.md`):
  - **datetime binding** (espejo del decimal Fase 2): `chrono::DateTime<Utc>` serializa serde como string RFC3339 → SurrealQL `datetime` schema rechaza (`FieldCheck check:"datetime"`). Fix: helpers `dt_val(dt)` = `surrealdb::sql::Datetime::from(dt).into()` y `dt_opt`. Aparece independientemente en Fase 3 (inventory: batch expiry, movement filters), Fase 7-subset (prescriptions dispensed_at, shift started_at), Fase 5-subset (price_list valid_from). Patrón obligatorio para todo bind de `datetime`.
  - **serde-flatten + Option<Thing>**: `#[serde(flatten)]` sobre struct que contiene `Option<surrealdb::sql::Thing>` rompe deserialización (`untagged and internally tagged enums do not support enum input`). Workaround: dos queries separadas + join en código, NO flatten. Documentado en `purchasing/repo.rs::supplier_name`.
- **Pendiente**: Fase 4 Sales/POS (usa FEFO de A + customer/loyalty de B + supplier-prices de C). Fase 5 completa (purchase_orders + receive + WAC + AP). Fase 6 Finance/Reports. Fase 8 cron+backup+swagger. Fase 9 hardening+MSI.

---

## 2026-05-16 — Visión extendida: ecosistema federado de agentes ERP

- **Qué**: documento `docs/ecosystem-roadmap.md` formaliza ampliación visión. Pharma-server deja de ser solo ERP on-prem vendible — pasa a ser **nodo de malla federada de agentes** (farmacia, proveedor, droguería, lab) donde humanos reales transan vía protocolo común. Fases 10 (sync online opt-in) y 11 (agent protocol) añadidas al roadmap.
- **Por qué**: usuario expresó objetivo dual: (a) ERP descargable Windows offline+online, (b) "ecosistema de agentes con dueños humanos reales comerciando". Necesario alinear desde ya — decisiones de arquitectura cambian (identidad criptográfica per-nodo, schema compartido, outbox sync) si se diseña con la mira larga, vs si se posterga y rompe compat después.
- **Lecciones rescatadas de Tu Farmacia** (`build-and-deploy-webdev-asap/pharmacy-ecommerce/`):
  - POS sale flow atómico (`apps/web/src/app/api/admin/pos/sale/route.ts`) → blueprint Fase 4.
  - Cierre-dia agregaciones → blueprint Fase 6.
  - `drug-interactions.ts` (Beers+Vademécum CL) y `controlled-substances.ts` (Decreto 404) → port literal a `domain::sales`.
  - Loyalty (`lib/loyalty.ts`), Transbank (`lib/transbank.ts`), OCR Cloud Vision (`scan-invoice/route.ts`), cron jobs (`api/cron/*`), Electron wrapper desktop POS (`apps/desktop/main.js`) → patrones reusables.
  - Tu Farmacia es single-tenant cloud-first; pharma-server es multi-tenant offline-first. Reglas negocio + UI reusan; stack no.
- **Decisiones nuevas (locked-in)**:
  - Online sync ON por defecto = **OFF**. Opt-in por tenant. Datos sensibles (PII, recetas, ventas) NUNCA salen del nodo sin opt-in explícito.
  - Protocolo agente: Ed25519 + HTTP push + relay opcional + JSON canónico firmado. DID-style `did:pharma:<pubkey>`.
  - Reputación local-only por nodo. Sin scoring centralizado.
  - Desktop wrapper preferred: **Tauri** (Rust nativo, más liviano que Electron). Decisión abierta pero leaning.
  - Catálogo global (`barcode_catalog`, `therapeutic_category_mapping`) ya existente desde Fase 2 = vocabulario producto canónico cross-nodo. Foundation correcta.
- **Decisiones abiertas**: marketplace cross-tenant alcance (read-only fase 1 vs bidireccional fase 2), identity verifiable (SII/ISP attestation) post-Fase 11, hub federado oficial vs solo self-host.
- **Orden propuesto**: F4 Sales→F5full→F6→F8→F9 (v1.0.0 vendible)→F10 sync (v1.1.0)→F11 agentes MVP (v1.2.0 "agent-ready").
- **Archivos**: `docs/ecosystem-roadmap.md` (nuevo); `CLAUDE.md` (header actualizado con visión extendida).
- **Pendiente inmediato**: arrancar Fase 4 Sales/POS (migración `0007_sales.surql`, `domain::sales` dir-module, `POST /pos/sale` atómico con FEFO+loyalty+prescription+stock_movement).

---

## 2026-05-16 — Fase 4 erp-parity Sales/POS (branch feature/erp-parity-sales, v0.1.4)

- **Qué**: POS sale end-to-end + orders read + admin_setting + idempotency + loyalty award. Migración `0007_sales.surql`, dir-module `domain::sales`, endpoints `/api/v1/{pos/sale, orders, settings/{key}}`. 7 integration tests verde.
- **Migración 0007** (SCHEMAFULL, multi-tenant, append-only): `order` (status enum 6 estados, payment_method enum 7, discount default 0, customer opt<record>, sold_by opt<record<user>>, external_ref para boleta SII), `order_item` (FEFO batch opt<record>), `devolucion` + `devolucion_item`, `admin_setting` (tenant key/value UNIQUE), `idempotency_key` (key per tenant UNIQUE, expires_at TTL).
- **`domain::sales` dir-module**: `controlled.rs` port literal Tu Farmacia `lib/controlled-substances.ts` (Decreto 404 set 24 sustancias); `interactions.rs` scaffold types-stable + `check()` stub (Beers ruleset port pending); `model.rs` 13 DTOs (decimal str/str_option serde); `repo.rs` 450 LoC; `service.rs` con validaciones + idempotency + loyalty.
- **`repo::apply_sale` (two-call atomic pattern)**: paso 1 = `CREATE order RETURN AFTER` (single stmt, captura Thing); paso 2 = `BEGIN; per-item {CREATE order_item, UPDATE product SET stock-=qty, CREATE stock_movement reason='sale' ref=<order.id>}; COMMIT;`. Razón two-call: SurrealDB 2.x LET slot semantics inconsistente entre versiones + `SELECT VALUE id ... ORDER BY created_at` rechazado por gotcha ORDER-BY-projection ya documentado. Atómico donde importa (items+stock+movements).
- **`service::post_sale`** flow: validate (non-empty, payment_method ∈ POS_METHODS, qty>0, price≥0) → idempotency lookup (sentinel `Conflict("IDEMPOTENCY_CACHED:<json>")`) → stock pre-check (single SELECT IN $ids tenant-scoped) → money totals (subtotal, clamp discount, total) → mixed-payment cross-check (cash+card≥total) → tenant parse_typed → `apply_sale` → loyalty award si customer → `store_idempotency` 24h TTL.
- **Loyalty integrado**: `repo::award_loyalty` (atomic tx append `loyalty_transaction` + bump `customer.loyalty_points`). Conversión configurable via `admin_setting.loyalty_points_per_clp` (default 1000 = 1 punto/$1000 CLP). Cliente opcional — sale sin customer no afecta loyalty.
- **`api/v1/sales.rs`**: POST `/pos/sale` role admin/owner/**cashier** (rol nuevo introducido para mostrador) + honra `Idempotency-Key` header → replay cached 200; GET `/orders` + filtros tenant-scoped (status, payment_method, customer, from/to, limit/offset); GET `/orders/{id}` (detalle con items); GET `/settings/{key}` bearer; PUT `/settings/{key}` admin/owner.
- **Decisiones nuevas**: (1) Rol `cashier` solo para `/pos/sale` (no para CRUD productos/precios). (2) Sentinel error `Conflict("IDEMPOTENCY_CACHED:<json>")` para señalizar cache hit desde service → handler controla status; trade-off cleaner que pasar `Result<T, Either<DomainError, CachedJson>>`. (3) Two-call sale (CREATE order + tx items): rompe pure atomicity del order row, pero el order standalone es side-effect-free (no toca stock), y la tx de items SÍ es atómica.
- **Out of scope deferred a slice next** (mantenido sentinel stubs en code): FEFO batch decrement (plan_fefo + UPDATE product_batch), prescription create desde POS, full interactions ruleset port (~370 LoC Beers + Vademécum CL), devolucion endpoints (model + migración ya listos, falta service+api).
- **Tests** (`crates/domain/tests/sales.rs`, Mem + migraciones reales, 7 tests): atomic decrement (50→47), insufficient stock blocked (estado intacto), invalid payment method, tenant isolation cross-tenant NOT_FOUND, admin_setting upsert idempotente, loyalty award default (10 puntos por $10000), loyalty rate setting override (6 puntos por $3000 con setting=500).
- **Versión**: workspace `0.1.3 → 0.1.4` (patch por fase).
- **Verificación**: fmt clean · clippy `-D warnings` clean (1 fix `clippy::doc_lazy_continuation` en doc comment) · `cargo test --workspace` 68 tests verde (7 nuevos sales + 61 previos).
- **Commits branch `feature/erp-parity-sales`**: `b4b086d` scaffold+migration · `bb284a5` service+repo+tests · `fb181ae` api router · pendiente bump+merge.
- **Pendiente**: full interactions port, prescription POS link, FEFO batch decrement, devolucion endpoints, Fase 5-full (PO+receive+WAC+AP), Fase 6 (caja+gastos+reportes), Fase 8 (cron+backup+swagger+Tauri desktop), Fase 9 (hardening+MSI vendible v1.0.0), Fase 10 (sync online opt-in), Fase 11 (agent protocol MVP).

---

## 2026-05-16 — Fase 11 step 1 (agent identity) + MSI downloadable verificado (v0.1.4)

- **Qué**: (1) crate nuevo `agent` — foundation ecosistema federado: identidad Ed25519, DID `did:pharma:<bs58(pubkey)>`, AgentCard self-signed, Envelope firmado canonical-JSON. CLI `pharma agent {init,did,card,verify}`. (2) **MSI buildeable verificado end-to-end**: `pharma-server-0.1.4-x86_64.msi` (11.2 MB) generado con WiX v3.14 + cargo-wix 0.3.9.
- **Por qué**: el goal exige (a) ERP descargable Windows offline+online, (b) ecosistema de agentes con dueños humanos transando. Ambos avanzados este bloque: MSI prueba el "descargable Windows" real; `agent` crate es la base criptográfica del mesh.
- **crate `agent`** (offline-pure, sin networking):
  - `identity.rs`: `Identity` keypair Ed25519, seed hex persistido (0600 Unix), `generate/save/load/load_or_init` idempotente, `did()`, `verify_with_did()`. 4 tests.
  - `canonical.rs`: JSON determinista (keys ordenadas, sin whitespace) — dos nodos hashean idénticos bytes para verificar firma. 1 test.
  - `card.rs`: `AgentCard` self-signed (did, name, kind pharmacy|supplier|distributor|lab, region, endpoint). Tamper en cualquier campo invalida sig. 3 tests.
  - `envelope.rs`: `Envelope` (from/to/msg_id/ts/topic/body/sig) firmado sobre canonical sans sig. Detecta body tampered + `from` forjado. Topics MVP documentados (catalog.lookup, quote.request, po.create, shipment.notify, payment.confirm). 4 tests.
- **CLI agent**: `init` (keygen idempotente, default path = sibling del data dir SurrealKv `<db dir>/agent.key`), `did`, `card --name --kind --region --endpoint`, `verify <file>` (card o envelope, exit code para scripts/CI).
- **Bug real fixeado**: telemetry escribía a stdout → contaminaba `pharma agent card > card.json`. Nuevo `telemetry::init_cli` → logs a stderr, stdout limpio para piping. Smoke confirmó pipe limpio + tamper rejection.
- **MSI verificación**: `cargo wix --package service --no-build --nocapture -C -ext -C WixFirewallExtension -L -ext -L WixFirewallExtension` ejecutado desde `crates/service/` (CWD relativa al include `../../installer/wix/main.wxs`). Requiere release `pharma-service.exe` pre-built (8m03s) + WiX bin en PATH (`C:/Program Files (x86)/WiX Toolset v3.14/bin`). Artefacto: `target-shared/wix/pharma-server-0.1.4-x86_64.msi`. wxs ya completo (ServiceInstall LocalSystem auto-start + ServiceControl + firewall TCP 8080 + DataDir). **Gotcha**: cargo-wix resuelve `include` relativo a CWD, NO al crate — ejecutar desde `crates/service/` o el path rompe.
- **Decisiones nuevas (locked)**: (1) agent key default = sibling del SurrealKv data dir (backup conjunto). (2) CLI telemetry → stderr (output piping limpio, contrato para tooling). (3) Canonical JSON propio (sorted keys) en lugar de depender de serde_json map order — necesario para firma cross-nodo determinista.
- **Verificación**: fmt clean · clippy `-D warnings` clean · `cargo test --workspace` 80 verde (12 agent + 68 previos) · MSI 11.2 MB generado OK.
- **PRs**: #6 Sales mergeado (`b12bfa5`), #7 agent mergeado (`80de3a2`). Branch `feature/erp-parity` al día.
- **Estado vs goal**: "descargable Windows" = **MSI v0.1.4 buildeable confirmado** (falta: firma cert Authenticode anti-SmartScreen, smoke install/uninstall en VM limpia → Fase 9). "offline" = SurrealKv embedded ya. "online + ecosistema agentes" = `agent` crate identity+envelope listo (falta: transport HTTP push/relay, topic handlers, reputación local — Fase 11 steps 2-4). "online sync ERP" = Fase 10 pendiente.
- **Pendiente** (orden): Fase 11 step 2 transport (HTTP push endpoint `/agent/inbox` + verify middleware) + topic handler `catalog.lookup` (usa `barcode_catalog` global), step 3 reputación local (`agent_interaction`), step 4 relay opcional. Paralelo: Fase 5-full (PO+receive+WAC+AP), Fase 6 (caja+gastos+reportes), Fase 8 (cron+backup+swagger+Tauri), Fase 9 (MSI firmado + smoke VM → v1.0.0 vendible), Fase 10 (sync online opt-in → v1.1.0).

---

## 2026-05-16 — Fase 11 step 2 (transport `/agent/inbox`) + release v0.1.4 descargable publicado

- **Qué**: (1) transport agente funcional node-to-node — `POST /agent/inbox` verifica firma Ed25519 del Envelope, despacha topic, responde Envelope firmado por el nodo. (2) **GitHub release `v0.1.4` con MSI adjunto** → ERP literalmente descargable desde URL.
- **Por qué**: el goal exige "descargable Windows" + "ecosistema agentes comerciando". El Stop hook marcó (correctamente) que MSI sólo era buildeable-from-source y el transport agente no existía. Ambos cerrados este bloque.
- **Transport** (`crates/api/src/v1/agent.rs`): `AppState.node_identity` (Ed25519 cargada en `api::run` desde `<db dir>/agent.key` load_or_init idempotente). `POST /agent/inbox` — SIN JWT/tenant: la autenticidad ES la firma del Envelope. Verifica sig sobre canonical bytes → 401 si tampered, 421 si misdirected (`to` ≠ DID nodo). Topics: `ping`→`pong`, `catalog.lookup`→`catalog.match` (resuelve contra `barcode_catalog` GLOBAL, cero data de tenant cruza el borde). `GET /agent/did` para reachability. Respuesta = Envelope nuevo firmado por el nodo.
- **Migración 0008_agent.surql**: `agent_interaction` NODE-LEVEL (NO tenant-scoped — federación es entre instalaciones soberanas, no entre tenants). Cada envelope entrante registrado con outcome (ok/rejected/error) → grafo de confianza local-only. Reputación NUNCA centralizada (decisión locked ecosystem-roadmap).
- **Decisión nueva (locked)**: `/agent/*` es node-level, fuera del modelo JWT/tenant. Auth = firma criptográfica del mensaje, no bearer token. Catálogo global (`barcode_catalog`) es el único dato que un nodo expone vía `catalog.lookup` — PII/ventas/stock por tenant jamás salen sin opt-in (Fase 10).
- **Release**: `gh release create v0.1.4 --target feature/erp-parity` con `pharma-server-0.1.4-x86_64.msi` (11,362,304 bytes, sha256 `67ca1e32382dae3dd3d65217ceb3710d011a148ce7eba9f444e10099527913a6`). URL: https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.4 . MSI rebuildeado post-merge (incluye agent transport). Notas con instrucciones msiexec + estado pre-prod (sin firma → SmartScreen).
- **Tests** (`crates/api/tests/agent_inbox.rs`, 4): signed pong (reply firmada por nodo verifica, echo msg_id, from=nodo to=peer), tampered envelope→401, catalog.lookup matchea barcode_catalog global + registra agent_interaction, unknown topic→400. Workspace 84/84 verde. clippy `-D warnings` clean, fmt clean.
- **PRs**: #8 mergeado (`a4cc530`). Branch `feature/erp-parity` al día.
- **Estado vs goal**: ✅ descargable Windows (release URL real, no sólo buildeable). ✅ offline (SurrealKv embebido). ✅ ecosistema agentes **funcional online** (exchange firmado node-to-node testeado e2e). ⏳ sync ERP online opt-in entre nodos = Fase 10 pendiente. ⏳ trading completo (quote.request/response, po.create inter-nodo) = Fase 11 steps 3-4. ⏳ firma cert MSI + smoke VM = Fase 9 (v1.0.0 vendible).
- **Pendiente** (orden): Fase 11 step 3 (`quote.request`/`quote.response` precio entre nodos), step 4 (`po.create` OC inter-nodo + relay opcional offline-peer). Paralelo: Fase 5-full (PO+receive+WAC+AP local), Fase 6 (caja+gastos+reportes), Fase 8 (cron+backup+swagger+Tauri desktop), Fase 9 (MSI firmado Authenticode + smoke VM limpia → v1.0.0), Fase 10 (sync ERP online opt-in → v1.1.0).

---

## 2026-05-16 — Fase 11 steps 3-4: comercio inter-nodo + release v0.1.5

- **Qué**: ciclo de comercio entre nodos cerrado. `/agent/inbox` topics nuevos `quote.request`→`quote.response` y `po.create`→`po.ack`. Migración `0009_agent_order.surql`. GitHub release `v0.1.5` con MSI (11.38 MB) incluyendo la capa de trading. Cierra el gap que el Stop hook marcó: "comerciando" ahora funcional + testeado e2e.
- **Por qué**: el goal exige "ecosistema de agentes con dueños humanos reales comerciando". Identity+transport (steps 1-2) no era comercio; faltaban cotización y orden de compra inter-nodo. Steps 3-4 lo completan: descubrir (`catalog.lookup`) → cotizar (`quote.request`) → ordenar (`po.create`), todo firmado entre nodos soberanos.
- **`quote.request`→`quote.response`**: body `{tenant, items:[{barcode,qty}]}`. Resuelve producto del tenant proveedor vía `product_barcode` join; responde `{tenant, currency:CLP, lines:[{barcode,product_name,unit_price,qty,available,line_total,in_stock}], total}`.
- **`po.create`→`po.ack`**: body `{tenant, lines:[{barcode,qty,unit_price}], buyer_note?}`. Persiste `agent_order` (tenant-scoped al proveedor + `peer_did` del comprador, `lines_json`, total, status='received'); responde `{order_id, status, currency, total}`.
- **Gate opt-in federación (decisión locked aplicada)**: `quote.request`/`po.create` solo responden si el tenant tiene `admin_setting` key `federation_enabled` == "true". Si no → 403. Precios/stock por tenant privados por defecto; nada sensible cruza el borde del nodo sin opt-in explícito del operador. Helper `resolve_federation_tenant`.
- **Migración 0009**: `agent_order` SCHEMAFULL tenant-scoped (proveedor que cumple) + `peer_did` (comprador federado), `lines_json` string (evita gotcha nested-schema/Option<Thing>), status enum lifecycle (received/accepted/rejected/fulfilled/cancelled).
- **Gotchas nuevos**: (1) `value` es palabra reservada SurrealQL → `SELECT value FROM admin_setting` no parsea; usar `SELECT *` + campo struct. (2) `product.price` es schema `decimal` → deserializar a `f64` falla silencioso (take→None); castear en query `<float> price AS price`. CLP entero (sin centavos) → sin pérdida de precisión.
- **Tests** (`crates/api/tests/agent_inbox.rs` +3, total 7): quote priced lines con federación ON (total 14900 por 10×1490, in_stock), quote bloqueado 403 con federación OFF, po.create persiste agent_order + ack (total 35760 por 24×1490, peer_did=comprador, status=received). Workspace **87/87 verde**, clippy `-D warnings` clean, fmt clean.
- **Release**: `gh release create v0.1.5 --target feature/erp-parity` con `pharma-server-0.1.5-x86_64.msi` (11,382,784 bytes, sha256 `be4607fc6d1ae08af435d3620a84f474354e7ea8900c5306079cf343192ca5b6`). https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.5
- **PRs**: #9 mergeado (`4df172c`). Versión 0.1.4 → 0.1.5.
- **Estado vs goal**: ✅ descargable Windows (release v0.1.5) · ✅ offline (SurrealKv) · ✅ ecosistema agentes **comerciando** (catalog.lookup→quote→po firmado, opt-in, e2e testeado) · ⏳ sync replicación datos ERP entre nodos = Fase 10 · ⏳ fulfillment/settlement de agent_order + relay offline-peer = siguiente · ⏳ MSI firmado cert + smoke VM = Fase 9 (v1.0.0 vendible).
- **Pendiente**: order fulfillment flow (`po.accept`/`po.fulfill` + descuento stock real vía sales/inventory), relay opcional para peers offline, Fase 5-full (PO local+WAC+AP), Fase 6 (caja+reportes), Fase 8 (cron+backup+swagger+Tauri), Fase 9 (MSI firmado → v1.0.0), Fase 10 (sync online opt-in → v1.1.0).

---

## 2026-05-16 — v0.1.6: first-run fix + venta atómica + endurecimiento seguridad `po.create` + reestructura bitácora

- **Qué**: release v0.1.6. Tres fixes (dos pre-existentes en la branch + uno nuevo de seguridad) + reestructura de esta bitácora. MSI rebuildeado, smoke install limpio real, release publicado, PR #10 mergeado.
- **Por qué**: el goal exige ERP descargable que instale y quede funcional sin fricción, y un ecosistema federado donde nodos soberanos comercian sin poder estafarse. El `po.create` confiaba en el `unit_price` del comprador → un peer malicioso podía persistir una orden a precio arbitrario en el nodo proveedor.
- **fix(service) first-run** (commit pre-branch `244427f`): el servicio corre migraciones al arrancar desde schema embebido. Instalación MSI limpia queda healthy sin invocar la CLI (`/health/ready` → db ok recién instalado, verificado en smoke).
- **fix(sales) venta atómica** (commit pre-branch `1b67590`): venta POS single-tx con decremento FEFO de lotes.
- **fix(agent) SEGURIDAD `po.create`** (`82f0f7c`): el nodo proveedor re-cotiza cada línea contra su propio catálogo (mismo path que `quote.request`: `product_barcode` join → `product` con `<float> price AS price`, gate `federation_enabled` intacto). El precio canónico manda; la línea persistida lleva `unit_price_canonical` + `unit_price_sent`; el `po.ack` devuelve `price_adjusted: true` si alguna línea fue reescrita (precio divergente, producto desconocido o inactivo). `agent_order.total` ahora es el canónico, nunca el del comprador. NO rompe compat con peers ya releasados: el body de entrada es idéntico, sólo cambian campos del ack (additive) y el contenido de `lines_json`.
- **docs(bitacora)** (`dee2875`): bloque `## ESTADO ACTUAL` al tope (se sobrescribe cada sesión, single source of truth) + `## BACKLOG` único priorizado (consolidé todos los "Pendiente" dispersos). Log append-only histórico intacto debajo. Gotchas NO duplicados: viven en memoria + vault `brain/pharma-server-gotchas.md`, sólo referenciados.
- **Tests** (`crates/api/tests/agent_inbox.rs`): +2 de seguridad — `po_create_rejects_buyer_supplied_price_and_uses_canonical` (envía unit_price=1, producto vale 1490 → total ack 14900, `price_adjusted=true`, `agent_order.total`=14900 persistido) y `po_create_marks_adjusted_when_product_unknown` (producto desconocido → total 0, `found:false`, flag true). `po_create_records_order_and_acks` actualizado para asertar `price_adjusted=false` cuando el comprador manda el precio correcto. agent_inbox 9/9, workspace verde, clippy `-D warnings` clean, fmt clean.
- **Build/MSI**: `cargo build --workspace --release` (7m32s, CARGO_TARGET_DIR=target-shared). MSI `pharma-server-0.1.6-x86_64.msi` 11,710,464 bytes, sha256 `fe8c496387c7fbb4a8cc3856b177c080581f729c3a86db5b9b9f42423678a66d`. **Gotcha confirmado**: con `cargo wix` corriendo desde `crates/service/`, `CARGO_TARGET_DIR` relativo (`target-shared`) lo resuelve cargo-wix relativo a SU CWD → busca el exe en `crates/service/target-shared/release` y falla `LGHT0103`. Fix: exportar `CARGO_TARGET_DIR` como ruta **absoluta** antes de `cargo wix`. (Actualizado en memoria `project_wix_msi_gotchas`.)
- **Smoke install REAL** (este Windows): `Stop-Service PharmaServer` → `msiexec /i pharma-server-0.1.6-x86_64.msi /qn` (MajorUpgrade removió 0.1.5, exit 0) → `Get-Service PharmaServer` Running → `curl /` `{"name":"pharma-server","version":"0.1.6"}` → `curl /health/ready` 200 `{"status":"ok","checks":{"db":"ok"}}`. El `db:ok` recién instalado confirma el fix first-run end-to-end.
- **Release**: `gh release create v0.1.6 --target feature/erp-parity` con el MSI adjunto. https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.6
- **PRs**: #10 mergeado (merge commit `2eed7d5`). Versión 0.1.5 → 0.1.6.
- **Estado vs goal**: ✅ descargable Windows (release v0.1.6, smoke limpio) · ✅ offline (SurrealKv) · ✅ instala healthy sin CLI (first-run) · ✅ ecosistema agentes comerciando **con integridad de precio** (el proveedor no puede ser estafado vía `unit_price`) · ⏳ fulfillment/settlement + relay offline = siguiente · ⏳ MSI firmado cert + smoke VM limpia = Fase 9 (v1.0.0 vendible) · ⏳ Fase 10 sync online.
- **Pendiente**: ver `## BACKLOG` al tope (lista priorizada única).

---

## 2026-05-16 — v0.1.7: devoluciones/refunds (cierra ítem BACKLOG, Fase 4 completa POS↔return)

- **Qué**: feature devoluciones end-to-end. `POST /api/v1/pos/returns` + `GET /api/v1/returns`. Modelo + migración `0007` (`devolucion`/`devolucion_item`) ya existían desde Fase 4; faltaban repo+service+API+tests. Release v0.1.7, smoke install limpio.
- **Por qué**: el ítem "Devolución endpoints" estaba en BACKLOG con modelo/migración listos pero sin lógica. Una farmacia real necesita devoluciones (producto vencido, error de venta, garantía) — sin esto el POS está incompleto. Usuario pidió seguir trabajando autónomamente sobre BACKLOG; este era el ítem de menor riesgo y mayor cierre (no estaba en la exclusión de la sesión v0.1.6).
- **`repo::apply_refund`** (`crates/domain/src/sales/repo.rs`): un solo `BEGIN; … COMMIT;` — CREATE `devolucion` (id client-gen para vivir dentro de la tx) + N `devolucion_item`; por línea con `restock=true` además `UPDATE product SET stock += qty` + `CREATE stock_movement(reason='return')`; si hay `order` referenciada → `UPDATE order SET status='refunded'` en la misma tx. Índices de statement calculados dinámicamente (restock = +2 statements) para `take(idx)` correcto. **Invariante mantenido**: stock nunca se escribe fuera del audit trail (mismo principio que `apply_sale`).
- **`service::create_refund`**: valida items no vacío, `qty>0`, `unit_price>=0`, `restock` exige `product` (no se puede reponer un ítem sin SKU — `stock_movement.product` es obligatorio), y si hay `order` rechaza **sobre-devolución** (qty devuelta por producto ≤ qty vendida en esa orden; producto debe pertenecer a la orden). `list_refunds` filtrable por order/tipo, paginado.
- **API** (`crates/api/src/v1/sales.rs`): `POST /api/v1/pos/returns` roles `admin/owner/cashier` (mostrador procesa devoluciones), `GET /api/v1/returns` bearer. Mismo patrón tenant-scoped que el resto de sales.
- **Tests** (`crates/domain/tests/sales.rs` +4, total 14): restock devuelve stock + marca order `refunded` + registra movement; no-restock no toca stock ni movements; sobre-devolución (qty>vendido) → `INVALID_INPUT` con stock intacto; restock sin product → `INVALID_INPUT`. Workspace verde, clippy `-D warnings` clean, fmt clean.
- **Build/MSI/Smoke**: release build (CARGO_TARGET_DIR absoluto). MSI `pharma-server-0.1.7-x86_64.msi` 11,743,232 bytes, sha256 `6ad21a16b248802d4337f2f2938c0175ac6ba371d85fcc5d7e45ac2997536455`. Smoke real: stop→`msiexec /i` (MajorUpgrade quitó 0.1.6, exit 0)→Running→`/`=`{"version":"0.1.7"}`→`/health/ready` 200 `db:ok`.
- **Release**: `gh release create v0.1.7 --target feature/erp-parity`. PR `feature/erp-parity-returns` → `feature/erp-parity`. Versión 0.1.6 → 0.1.7. Cargo.lock sincronizado en el mismo commit del bump (lección de v0.1.6: no dejar drift toml/lock).
- **Estado vs goal**: ✅ POS completo (venta + devolución atómicas) · ✅ descargable/offline/first-run/agentes (sin cambios) · ⏳ fulfillment/settlement + relay = siguiente · ⏳ Fase 9 MSI firmado · ⏳ Fase 10 sync · ⏳ Fase 12 marketplace (plan en branch separada).
- **Pendiente**: ver `## BACKLOG` al tope.

---

## 2026-05-16 — v0.1.8: `po.status` (comprador consulta decisión) + price_adjusted durable

- **Qué**: topic federado nuevo `po.status` → `po.status.result`. Migración `0010` persiste `price_adjusted` en `agent_order`. Release v0.1.8, smoke install limpio.
- **Por qué**: tras `po.create` el comprador no tenía forma protocolar de saber la decisión del proveedor ni si su orden fue re-cotizada. Sin esto el "comercio" inter-nodo es ciego del lado comprador. Es el primer paso (de menor riesgo, additive) del ítem #2 del BACKLOG (fulfillment/settlement) — los siguientes (`po.accept`/`po.reject`/`po.fulfill`) implican acción local del operador + descuento real de stock y se dejan para después.
- **`po.status`** (`crates/api/src/v1/agent.rs`): body `{order_id}` → `{order_id,status,total,currency,price_adjusted}`. **Autorización = propiedad del DID**: la query filtra `agent_order WHERE id=$id AND peer_did=$from` (el `from` del Envelope firmado). Un peer no puede leer órdenes de otro comprador aunque conozca el id. Autenticidad = firma Ed25519; autorización = DID ownership. Decimal→f64 con cast `<float> total` (gotcha conocido). order_id inválido→400, no-agent_order→400, no encontrado/otro DID→404.
- **Migración 0010**: `DEFINE FIELD price_adjusted ON agent_order TYPE bool DEFAULT false`. `po.create` ahora bindea `price_adjusted=$pa` en el CREATE (antes solo se devolvía en el ack y se perdía). Append-only; embebida vía `include_dir!` (compile-time) → el service la aplica en first-run sin tocar CLI (verificado en smoke: upgrade 0.1.7→0.1.8 aplicó 0010, `/health/ready` db:ok).
- **Tests** (`crates/api/tests/agent_inbox.rs` +2, total 11): po.create con precio malo → po.status del mismo comprador devuelve `status=received`, `total` canónico (3×1490=4470), `price_adjusted=true`; otro DID → 404; id inexistente → 404. Workspace verde, clippy `-D warnings` clean, fmt clean.
- **Build/MSI/Smoke**: release build (CARGO_TARGET_DIR absoluto). MSI `pharma-server-0.1.8-x86_64.msi` 11,747,328 bytes, sha256 `8e172b3454fb68ba9961050d5f32af61fce1fb573b17228eba146a36b4d6b2f2`. Smoke real: stop→`msiexec /i` (MajorUpgrade quitó 0.1.7, exit 0)→Running→`/`=`{"version":"0.1.8"}`→`/health/ready` 200 `db:ok`.
- **Release**: `gh release create v0.1.8 --target feature/erp-parity`. PR `feature/erp-parity-po-status` → `feature/erp-parity`. Versión 0.1.7 → 0.1.8 (bump + Cargo.lock mismo commit).
- **Compat federada**: topic nuevo additive — no cambia `po.create`/`quote.request`/peers ya releasados. Migración additive con DEFAULT (órdenes viejas → `price_adjusted=false`).
- **Pendiente**: ver `## BACKLOG` al tope.

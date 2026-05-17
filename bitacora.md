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

- **Versión**: `0.1.16` (workspace `Cargo.toml`).
- **Branch**: `feature/erp-parity-backup` (PR → `feature/erp-parity`).
- **MSI release**: https://github.com/pabloalvarez99/pharma-server/releases/tag/v0.1.16
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
  - **Operador acepta/rechaza/despacha órdenes entrantes**:
    `GET /api/v1/agent-orders`, `POST /{id}/accept|reject|fulfill` (JWT,
    role admin/owner, tenant-scoped). **`fulfill` decrementa stock real**
    (`product.stock -= qty` + `stock_movement(reason='agent_fulfill')` por
    línea + `agent_order.status='fulfilled'`, todo en un BEGIN/COMMIT;
    invariante `stock = SUM(stock_movement.delta)` se mantiene). Transiciones:
    `received → accepted|rejected`, `accepted → fulfilled`. Cualquier otra = CONFLICT.
  - **Receta desde POS**: `PosSaleRequest.prescriptions` persiste `prescription`
    rows ligadas al cliente; `controlled` se autodetecta vía
    `product.active_ingredient` si el POS no lo manda. IDs vuelven en
    `PosSaleResponse.prescriptions`.
  - **Alertas de interacciones medicamentosas** (Beers + Vademécum CL, 31
    reglas, 12 grupos): cada venta tokeniza `product.active_ingredient` de
    cada item y devuelve `interaction_warnings` ordenados por severidad. No
    bloquea la venta (caveat clínico).
- **Falta para v1.0.0 vendible**: firma cert Authenticode (anti-SmartScreen) +
  smoke install/uninstall en VM limpia (Fase 9).
- **Tests**: workspace verde (`cargo test --workspace`), incluye 14 `sales`
  (devoluciones) + 11 `agent_inbox` (`po.status`) + 8 `agent_orders` (3 nuevos
  de `fulfill`).

---

## BACKLOG

> Lista priorizada única. Consolidé acá todos los "Pendiente" dispersos del log.

1. **Fase 9 — MSI vendible v1.0.0**: firma Authenticode con cert + smoke
   install/uninstall en VM Windows limpia (sin firma → SmartScreen warning).
2. **~~Order fulfillment/settlement agente~~** ✅: `po.status` (comprador
   consulta), operador accept/reject/fulfill (`/api/v1/agent-orders/{id}/...`)
   con stock decrement atómico + audit trail. Pendiente menor: multi-lot/FEFO
   split en path federado (sales ya lo tiene).
3. **~~Multi-lot split traceability~~** ✅ (sales path): migración 0013 +
   `order_item.batches_json` + `OrderItemDto.batches`. Pendiente: replicar en
   `agent_fulfill` (path federado) — ver BACKLOG #2.
4. **~~Drug-interactions ruleset port~~** ✅ (Beers + Vademécum CL, 31 reglas).
5. **~~Prescription desde POS~~** ✅: receta(s) ligada(s) a la venta + cliente,
   `controlled` autodetectado vía `product.active_ingredient`.
6. **Relay offline-peer**: cola/relay para nodos federados sin conexión directa.
7. **Fase 10 — sync ERP online opt-in** entre nodos (replicación datos → v1.1.0).
8. **Fase 5-full**: PO local + recepción + costo promedio ponderado (WAC) + AP.
9. **Fase 6** (mayormente cerrada): ~~caja~~ ✅ v0.1.14 · ~~gastos + report
   sales-daily~~ ✅ v0.1.15. Falta: reportes avanzados (márgenes/rotación/ABC/
   vencimientos) — extensiones sobre el mismo patrón.
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

---

## 2026-05-16 — v0.1.9: operador acepta/rechaza órdenes federadas entrantes

- **Qué**: superficie HTTP JWT/tenant-scoped para que el operador del proveedor actúe sobre `agent_order`s entrantes. Nuevo `domain::agent_orders` (model+service) + `crates/api/src/v1/agent_orders.rs`. Release v0.1.9, smoke limpio.
- **Por qué**: tras `po.create` la orden quedaba `received` para siempre — nadie del lado proveedor podía decidir, y el `po.status` del comprador nunca cambiaba. Esto abre el lazo: `po.create` (firmado) → operador accept/reject (JWT) → comprador lo ve vía `po.status` (firmado). Es el segundo paso del ítem #2 del BACKLOG; falta solo `po.fulfill` (descuento real de stock).
- **Separación de planos (decisión)**: la *creación* de `agent_order` es federada (autenticidad = firma Ed25519, sin JWT). La *decisión* es acción humana local del operador → endpoint JWT/tenant-scoped normal (role admin/owner), NUNCA un topic federado (el peer no decide su propia orden). `agent_order.tenant` ya existía → filtrado por `claims.tenant_id`; un tenant jamás ve órdenes de otro.
- **`domain::agent_orders::service`**: `list` (tenant-scoped, filtro status, paginado), `get`, `decide` (transición **solo** `received → accepted|rejected`; re-decidir una orden ya resuelta = `CONFLICT`, no idempotente — decisión deliberada: aceptar dos veces o flip-flop es un error operativo, no un no-op). `lines_json` se decodifica de vuelta a array JSON para la UI del operador. `<float> total` cast (gotcha decimal→f64 conocido).
- **API** (`/api/v1/agent-orders`): `GET` (lista, `?status`), `GET /{id}`, `POST /{id}/accept`, `POST /{id}/reject` — todos `route_layer(role admin/owner)`.
- **Tests** (`crates/domain/tests/agent_orders.rs`, 5): list tenant-scoped + filtro status; accept luego re-decidir → CONFLICT; reject desde received; target inválido (`fulfilled`) → INVALID_INPUT con estado intacto; get cross-tenant → NOT_FOUND. Workspace verde, clippy `-D warnings` clean, fmt clean.
- **Build/MSI/Smoke**: release build. MSI `pharma-server-0.1.9-x86_64.msi` 11,763,712 bytes, sha256 `8a507d6a3a3bafbb405451542b7b2ff9e954c758c89102a2a52626f51e9ea992`. Smoke real: stop→`msiexec /i` (MajorUpgrade quitó 0.1.8, exit 0)→Running→`/`=`{"version":"0.1.9"}`→`/health/ready` 200 `db:ok`.
- **Release**: `gh release create v0.1.9 --target feature/erp-parity`. PR `feature/erp-parity-agent-orders-admin` → `feature/erp-parity`. Versión 0.1.8 → 0.1.9 (bump + Cargo.lock mismo commit).
- **Compat**: endpoints nuevos additive, sin migración (reusa `agent_order` + campos existentes). No toca path federado.
- **Pendiente**: ver `## BACKLOG` al tope.

---

## 2026-05-16 — v0.1.10: `fulfill` cierra el lazo comprador↔proveedor con stock real

- **Qué**: `POST /api/v1/agent-orders/{id}/fulfill` despacha la orden aceptada — decrementa `product.stock` real y deja audit trail. Es el paso que faltaba del ítem #2 del BACKLOG (fulfillment/settlement). Release v0.1.10, smoke limpio.
- **Por qué**: aceptar una orden y no descontar stock dejaba la decisión sin efecto físico. Ahora el lazo comercial inter-nodo es completo y consistente con el invariante de inventario: el comprador puede polear `po.status=fulfilled`, y el proveedor tiene la trazabilidad del stock que salió por cada orden federada.
- **`service::fulfill`** (`crates/domain/src/agent_orders/service.rs`): solo legal desde `accepted` (received/rejected/fulfilled → `CONFLICT`). Pre-resuelve cada línea catalogada vía `product_barcode` (tenant-scoped, `active=true`); si alguna falta producto o tiene stock insuficiente rechaza la orden ENTERA antes de cualquier escritura — no hay fulfillment parcial. Un único `BEGIN/COMMIT`: `UPDATE product SET stock = stock - $q` + `CREATE stock_movement(reason='agent_fulfill', ref=order_id)` por línea + `UPDATE agent_order SET status='fulfilled'`. Mantiene el invariante `product.stock = SUM(stock_movement.delta)` y la regla "stock NUNCA fuera del audit trail" (igual que `apply_sale` y `apply_refund`). Líneas con `found:false` del re-quote de `po.create` se saltan (no son catálogo del proveedor).
- **Decisión**: agent_fulfill NO usa FEFO/batch split todavía. El path sales sí (Fase 4), pero acá la complejidad cosmética de mostrar lotes a un peer federado no justifica bloquear el cierre del lazo — queda en BACKLOG como mejora.
- **API** (`crates/api/src/v1/agent_orders.rs`): `POST /api/v1/agent-orders/{id}/fulfill` (role admin/owner). Transiciones legales: `received → accepted|rejected`, `accepted → fulfilled`.
- **Tests** (`crates/domain/tests/agent_orders.rs` +3, total 8): happy path (stock 50→43, movement -7 con `reason=agent_fulfill` + `ref` correcto, status=fulfilled); fulfill desde received → `CONFLICT`; fulfill con stock insuficiente (stock=3, qty=10) → `INSUFFICIENT_STOCK`, orden queda `accepted` y stock intacto. agent_orders 8/8, workspace verde, clippy `-D warnings` clean, fmt clean.
- **Build/MSI/Smoke**: release build (7m). MSI `pharma-server-0.1.10-x86_64.msi` 11,780,096 bytes, sha256 `79410ce2393767a954f076c4b52d426a62581b4690d968e946fcf861760339eb`. Smoke real: stop→`msiexec /i` (MajorUpgrade quitó 0.1.9)→Running→`/`=`{"version":"0.1.10"}`→`/health/ready` 200 `db:ok`.
- **Release**: `gh release create v0.1.10 --target feature/erp-parity`. PR `feature/erp-parity-po-fulfill` → `feature/erp-parity`. Versión 0.1.9 → 0.1.10 (bump + Cargo.lock mismo commit).
- **Compat**: endpoint additive, sin migración. Path federado intacto.
- **Estado vs goal**: ✅ POS completo · ✅ descargable/offline/first-run · ✅ **lazo federado completo** (create→accept/reject→fulfill+stock, con po.status del lado comprador) · ⏳ Fase 9 MSI firmado · ⏳ Fase 10 sync · ⏳ Fase 12 marketplace.
- **Pendiente**: ver `## BACKLOG` al tope.
---

## 2026-05-16 — Fase 12 plan maestro marketplace de confianza (estrategia, no scaffolding)

- **Qué**: `docs/marketplace-master-plan.md` (449 líneas, 10 secciones extendidas + Mermaid §4) — análisis fundador/VC/arquitecto de marketplace antifraude identidad verificable + reputación portable CL→LATAM. PR #11 (commit `cec33c2`) mergeado a `feature/erp-parity`. Pointer en `CLAUDE.md` L6 + sección "## 3bis. Fase 12" en `docs/ecosystem-roadmap.md` (mismo PR).
- **Por qué**: el usuario pidió análisis ultradeep founder/VC/arquitecto de la idea marketplace. Conclusión reordena la tesis: el activo diferencial NO es "otro Yapo/Wallapop" sino el **protocolo federado firmado ya construido** (`crates/agent/{identity,envelope,card,canonical}.rs`, `crates/api/src/v1/agent.rs` con `po.create` re-cotizando precio canónico server-side L404-526, `migrations/0008_agent.surql`+`0009_agent_order.surql`, opt-in por tenant `federation_enabled`) anclado a un ERP que ya se vende single-player (MSI v0.1.x). Documento = marco estratégico acordado para cualquier trabajo futuro de marketplace/Hub.
- **Decisiones estratégicas locked** (no son opinión de una sesión, son el frame del proyecto Fase 12):
  1. Entrada = **B2B vertical** farmacia indep. ↔ droguería/distribuidor sobre el protocolo `agent` existente. ERP/POS MSI = anzuelo de adquisición + ancla de identidad. **NO** C2C general (inwinneable vs Facebook Marketplace).
  2. **Densidad geográfica primero**: Coquimbo/La Serena (Tu Farmacia = nodo #1 / design partner). Expansión horizontal sólo post-PMF.
  3. Palanca real = **verificación de transferencia** (Khipu/Fintoc open-banking confirma plata movida + titular == KYC) que mata el "comprobante falso" — la estafa #1 CL. Reputación = complemento, no núcleo.
  4. Monetización **3 capas**: (a) ERP SaaS on-prem (cash hoy + lock-in), (b) take-rate sobre GMV inter-nodo escrowed (upside, necesita 100s de nodos — nunca el runway base), (c) identity / verified-settlement-as-a-service (moat / opcionalidad unicornio).
  5. **NO custodiar fondos**: orquestar escrow vía partner licenciado CMF + Khipu/Fintoc; cobrar fee de orquestación. DIY custody = muerte regulatoria (Ley Fintech 21.521 / CMF / UAF).
  6. Arquitectura = Hub centralizado online (Postgres administrado, KYC, escrow, discovery, disputa) **sobre** el protocolo federado por debajo. **NO** malla leaderless en v1. Rust para node + protocolo + núcleo del Hub (Hub importa `crates/agent` *verbatim* → cero divergencia de sig-verify); TS (Next.js/Expo) para clientes. **Sin CRDTs** (modelo tenant-owned, sin multi-writer concurrente; outbox + LWW basta — patrón Fase 10).
  7. Riesgo existencial **#1** = el fundador construye el protocolo elegante en vez del producto aburrido (escrow + identidad verificada + reorden) que el mercado paga. Cripto = plomería, nunca un feature de cara al cliente.
  8. Techo realista vertical-pharma = PE/lifestyle, **no unicornio**. Ruta unicornio = generalizar a riel de confianza/liquidación SMB LATAM (`did:pharma`→`did:trade`), Fase-N infra, NO marketing de v1.
- **Scope (LOCKED, hard rule para sesiones futuras)**: estrategia/arquitectura **SOLO** — el diseño técnico del Trust Hub (registry + KYC + orquestador escrow + emisor Verifiable Credentials + scoring antifraude) es un **plan separado posterior** y **NO se inicia** hasta validar §2 con design partners reales (Coquimbo/La Serena, §6 del doc). Cero código de Hub especulativo.
- **Discoverability**: `CLAUDE.md` L6 (pointer Fase 12 → `docs/marketplace-master-plan.md`); `docs/ecosystem-roadmap.md` §3bis (cross-ref con resumen ejecutivo); memoria `project_marketplace_master_plan` + espejo vault.
- **Diff**: 3 docs, +473/-1. Cero código, cero deps, cero migraciones. Verificado contra evidencia citada en el doc (rutas reales de la rama).
- **Estado vs goal**: ✅ estrategia documentada con evidencia de código real · ⏳ validación con design partners → recién ahí inicia el plan técnico del Hub/escrow.
- **Pendiente**: ver `## BACKLOG` al tope.

---

## 2026-05-16 — v0.1.11: receta(s) desde POS (cierra BACKLOG #5)

- **Qué**: `PosSaleRequest.prescriptions` ya no se descarta — cada entry persiste una `prescription` row después del commit de la venta y los IDs vuelven en `PosSaleResponse.prescriptions`. Cierra el ítem #5 del BACKLOG. Release v0.1.11.
- **Por qué**: el modelo `PosPrescriptionInput`, la tabla `prescription`, y `prescriptions::service::create_prescription` ya existían pero `post_sale` los ignoraba (`prescriptions: Vec::new()`). Una farmacia real necesita la receta ligada a la venta para Ley 20.000 (controlados) y para recetas retenidas/cheque.
- **`detect_controlled`** (helper en `sales/service.rs`): si el POS deja `controlled = None`, consulta `product.active_ingredient` y delega a `sales::controlled::is_controlled` (Decreto 404 CL). Si el POS manda `Some(true)` explícito y faltan datos del médico, el repo de prescriptions ya rechaza con `INVALID_INPUT` (guard existente).
- **Compat**: aditivo. Llamadas viejas con `prescriptions: vec![]` siguen igual. No migración.
- **Tests** (`crates/domain/tests/sales.rs` +2, total 16): venta con prescription persiste `prescription:xxx` linked; controlled=true sin doctor → INVALID_INPUT. Workspace verde, clippy `-D warnings` clean, fmt clean.
- **Build/MSI/Smoke**: release build (7m). MSI `pharma-server-0.1.11-x86_64.msi` 11,780,096 bytes (idéntico tamaño a 0.1.10 — solo wiring), sha256 `63556fdc4fd760f258e3549648cb4ef4fdd753783282594d58bc516af276cda3`. Smoke real: stop→`msiexec /i` (MajorUpgrade quitó 0.1.10)→Running→`/`=`{"version":"0.1.11"}`→`/health/ready` 200 `db:ok`.
- **Release**: `gh release create v0.1.11 --target feature/erp-parity`. PR `feature/erp-parity-prescription-pos` → `feature/erp-parity`. Versión 0.1.10 → 0.1.11 (bump + Cargo.lock mismo commit).
- **Pendiente**: ver `## BACKLOG` al tope.

---

## 2026-05-16 — v0.1.12: drug-interactions ruleset port (Beers + Vademécum CL) — cierra BACKLOG #4

- **Qué**: `sales::interactions::check` ya no devuelve `Vec::new()` — port completo del ruleset clínico desde Tu Farmacia (`apps/web/src/lib/drug-interactions.ts`, ~370 LoC). `post_sale` ahora carga `active_ingredient` de cada producto del carrito (tenant-scoped) y devuelve `interaction_warnings` ordenados por severidad en la respuesta. La venta NUNCA se bloquea — caveat clínico mirrored: las reglas son referenciales, no sustituyen criterio farmacéutico.
- **Por qué**: el goal "ERP profesional para farmacias" incluye seguridad del paciente. Una venta de WARFARINA + IBUPROFENO o SILDENAFIL + NITRATO sin warning visible es un riesgo evitable. El ruleset upstream ya está validado clínicamente (Beers Criteria 2023, Vademécum Chileno, FNM ISP) y portarlo cierra el último ítem clínico-funcional pendiente del POS.
- **Ruleset** (`crates/domain/src/sales/interactions.rs`, 565 LoC): 12 grupos (AINE, ANTICOAGULANTE, IBP, BENZODIAZEPINA, IECA, ARA2, ISRS, NITRATO, ESTATINA_3A4, MACROLIDO_3A4, PDE5, HIPOGLICEMIANTE), 31 reglas (grupo|fármaco × grupo|fármaco con severidad Crítica|Mayor|Moderada). PAIR_MAP build-once con `OnceLock` — reglas específicas de mayor severidad ganan sobre reglas de grupo (ej: SIMVASTATINA+CLARITROMICINA = Crítica overridea ESTATINA_3A4×MACROLIDO_3A4 = Mayor). Una exclusión explícita: CLOPIDOGREL+PANTOPRAZOL (otros IBPs sí disparan; PANTOPRAZOL es el alternativa segura clínica).
- **Tokenizador**: uppercase + strip acentos castellanos (Á/É/Í/Ó/Ú/Ñ) + match **greedy longest-first** contra el set conocido de nombres de fármacos. Importante: "Mononitrato de isosorbida" contiene literal "ISOSORBIDA" Y "MONONITRATO DE ISOSORBIDA"; el matcher consume el más largo y rellena con espacios el span para evitar doble-match (ambos están en el grupo NITRATO y dispararían dos warnings idénticos contra PDE5). Caso real cubierto por test `pde5_plus_nitrato_is_critica`.
- **Wiring en `post_sale`**: nueva helper `load_active_ingredients` (single SELECT IN $ids, tenant-scoped). El resultado se pasa a `check()` y se serializa en `PosSaleResponse.interaction_warnings` (vacío serializa skip vía `serde(skip_serializing_if)`).
- **Compat**: aditivo. Sin migración. Productos sin `active_ingredient` simplemente no aportan tokens al check. Sales 16/16 y resto del workspace sin cambios.
- **Tests** (6 unit + workspace verde): pde5+nitrato Crítica con un solo hit (no doble-match); anticoagulante+aine Crítica; clopidogrel+pantoprazol excluido pero otros IBPs disparan Mayor; simvastatina+claritromicina override a Crítica; sort por severidad descendente; vacío/desconocido devuelve vacío. clippy `-D warnings` clean (1 fix `clippy::unnecessary_sort_by` → `sort_by_key(Reverse(...))`), fmt clean.
- **Build/MSI/Smoke**: release build (7m16s). MSI `pharma-server-0.1.12-x86_64.msi` 11,808,768 bytes, sha256 `a94c0b8882c26ce1cc81a0e9fb7c81e43ac271c38ca0663e5e7bd3425deac9cf`. Smoke real: stop→`msiexec /i` (MajorUpgrade quitó 0.1.11)→Running→`/`=`{"version":"0.1.12"}`→`/health/ready` 200 `db:ok`.
- **Release**: `gh release create v0.1.12 --target feature/erp-parity`. PR `feature/erp-parity-interactions` → `feature/erp-parity`. Versión 0.1.11 → 0.1.12 (bump + Cargo.lock mismo commit).
- **Estado vs goal**: ✅ POS completo (venta + devolución + receta + alertas de interacción) · ✅ descargable/offline/first-run · ✅ lazo federado completo · ⏳ Fase 9 MSI firmado · ⏳ Fase 10 sync · ⏳ Fase 12 marketplace.
- **Pendiente**: ver `## BACKLOG` al tope.

---

## 2026-05-16 — v0.1.13: `POST /api/v1/interactions/check` — preview live de interacciones

- **Qué**: endpoint pre-check para que el POS muestre warnings de interacción **antes** de commitear la venta (badge live mientras el cajero arma el carrito). v0.1.12 dejó las alertas activas pero solo on commit — demasiado tarde para UX. Body `{products:[product:xxx], extra_ingredients:[free-text]}` → mismo `interaction_warnings` que `post_sale` devolvería.
- **Por qué**: warnings clínicos solo en `PosSaleResponse` exigen ejecutar la venta para ver la alerta. El flujo real del cajero: agrega un ítem → quiere ver si interactúa con lo ya en el carrito. Sin pre-check, la única forma sería commitear y devolver, que es contra el invariante de stock.
- **Wiring** (`crates/api/src/v1/sales.rs`): `route_layer(reads)` (bearer, NO write-roles — pre-check es read-only). Tenant-scoped: product ids de otros tenants se filtran silenciosamente. `extra_ingredients` permite al POS pre-cargar líneas todavía no linked a un `product` (ej: ítems custom del cajero).
- **Refactor mínimo**: `domain::sales::service::load_active_ingredients` pasa de `async fn` privada a `pub` para reuso del api crate. Cero cambio en behaviour.
- **Compat**: aditivo. Sin migración. Cero impacto en `post_sale`, sales tests 16/16 verde.
- **Build/MSI/Smoke**: release build (7m10s). MSI `pharma-server-0.1.13-x86_64.msi` 11,821,056 bytes, sha256 `3643539a4d291eaf5881b5de0c103a1ac3da93bae808f56e76eaa5e4e106cbf3`. Smoke real: stop→`msiexec /i` (MajorUpgrade quitó 0.1.12)→Running→`/`=`{"version":"0.1.13"}`→`/health/ready` 200 `db:ok`.
- **Release**: `gh release create v0.1.13 --target feature/erp-parity`. PR `feature/erp-parity-interactions-check` → `feature/erp-parity`. Versión 0.1.12 → 0.1.13 (bump + Cargo.lock mismo commit).
- **Pendiente**: ver `## BACKLOG` al tope.

---

## 2026-05-16 — v0.1.14: caja (apertura/cierre/arqueo + movimientos) — Fase 6 parcial

- **Qué**: cash register completo. Nueva migración `0011`, crate `domain::cash_register`, API `/api/v1/cash-sessions[...]`. Release v0.1.14, smoke install limpio.
- **Por qué**: una farmacia real abre y cierra caja todos los días — sin esto no hay control de diferencias, no hay arqueo, no se puede reconciliar Z (cierre) con efectivo físico. Cierra el componente "caja" del ítem Fase 6 del BACKLOG (gastos + reportes quedan pendientes).
- **Migración 0011**: `cash_register_session` (status `open|closed`, `opening_cash` decimal, `closing_cash_counted/_expected`, `discrepancia`, `opened_at`/`closed_at`) + `cash_movement` (`tipo ingreso|retiro`, `amount > 0`, `reason`, `admin`, FK `session`). Índices `(tenant,opened_at)`, `(tenant,status,opened_at)`, `(tenant,user,status)`. Additive — migración append-only.
- **`domain::cash_register::service`**: invariantes principales:
  1. **Una caja abierta por (tenant, user)** — segundo open = `CONFLICT` (chequeo `SELECT count() GROUP ALL` antes de CREATE).
  2. Cualquier `add_movement` exige `session.status='open'` — si está cerrada, `CONFLICT`.
  3. `close_session` desde `closed` = `CONFLICT` (no idempotente — re-cerrar una caja resuelta es error operativo).
  4. `tipo` válido `ingreso|retiro`, `amount > 0`, `reason` no vacío — todo `INVALID_INPUT` ante violación.
  5. Tenant isolation: get/list/decide filtran por `tenant=$t`; otra organización no ve caja de la primera.
- **Math del arqueo** (`compute_summary`): expected = `opening_cash + cash_sales + Σ ingreso − Σ retiro`. `cash_sales` = `math::sum(order.cash_amount) WHERE tenant=$t AND payment_method IN ['pos_cash','pos_mixed'] AND status NOT IN ['refunded','cancelled'] AND created_at BETWEEN opened_at..close_time`. Sin denormalizar el link sale→session: el rango temporal hace el join. `discrepancia = counted - expected` (negativo = falta, positivo = sobra).
- **`arqueo` live**: misma fórmula que `close` pero sin freezear — el operador app pinta el expected en vivo mientras la caja sigue abierta (`closing_cash_expected` surface en el DTO, `counted/discrepancia` siguen `None`).
- **API**: roles `admin/owner/cashier` para writes; reads bearer. Endpoints: `POST /cash-sessions` (open), `GET /cash-sessions[?status&user]`, `GET /{id}`, `GET /{id}/arqueo` (live), `GET /{id}/movements`, `POST /{id}/movements`, `POST /{id}/close`.
- **Tests** (6 kv-mem): open+2 ventas pos_cash+ingreso 2000+retiro 500 → expected 14500, counted 14450 → discrepancia -50 (short, registrada sin error); segundo open mismo user = CONFLICT; movimiento en caja cerrada = CONFLICT; close-already-closed = CONFLICT; tipo "fuga" o amount=0 → INVALID_INPUT; cross-tenant get → NOT_FOUND. **Gotcha confirmado**: `rust_decimal::Decimal::is_sign_positive()` retorna `true` para `ZERO` — para validar "estrictamente positivo" usar `value <= Decimal::ZERO`, no `!is_sign_positive()`. Costó un test rojo al primer run.
- **Compat**: aditivo. Sin tocar `order` schema ni POS. Migración 0011 embebida (`include_dir!`) → first-run de upgrade aplica sola en el smoke real (verificado: `/health/ready` db:ok tras `msiexec /i` sobre instalación 0.1.13).
- **Build/MSI/Smoke**: release build (7m19s). MSI `pharma-server-0.1.14-x86_64.msi` 11,943,936 bytes (~120 KB más que 0.1.13 por el binario más grande), sha256 `6f75d3cfe87b6bb2e09c9d3c6a960219b9a7ce99755244ab9bddb204107c7fc5`. Smoke real: stop→`msiexec /i` (MajorUpgrade quitó 0.1.13, exit 0)→Running→`/`=`{"version":"0.1.14"}`→`/health/ready` 200 `db:ok`.
- **Release**: `gh release create v0.1.14 --target feature/erp-parity`. PR `feature/erp-parity-cash-register` → `feature/erp-parity`. Versión 0.1.13 → 0.1.14 (bump + Cargo.lock mismo commit).
- **Estado vs goal**: ✅ POS completo · ✅ devoluciones · ✅ receta · ✅ alertas interacciones (commit + live) · ✅ caja apertura/cierre/arqueo · ✅ lazo federado completo · ⏳ Fase 6 restante (gastos + reportes) · ⏳ Fase 9 firma MSI · ⏳ Fase 10 sync · ⏳ Fase 12 marketplace.
- **Pendiente**: ver `## BACKLOG` al tope.

---

## 2026-05-17 — v0.1.15: gastos + reporte sales-daily (Fase 6 mayormente cerrada)

- **Qué**: dos slices para cerrar Fase 6 contable básica. Migración `0012` agrega `expense`. Nuevo `domain::expenses` (model+service) y API `/api/v1/expenses` + `/api/v1/reports/sales-daily`. Release v0.1.15, smoke install limpio.
- **Por qué**: caja sola no cierra el ciclo financiero del día — el operador necesita registrar gastos (arriendo, luz, sueldos, facturas) y ver ingresos por día para evaluar rentabilidad. Sin esto la app pinta solo el efectivo del cajón, no el negocio.
- **`expense`** (`migrations/0012_expenses_and_reports.surql`): `category`, `description`, `amount > 0`, `payment_method ∈ {cash, bank, card, transfer}`, opcional `cash_session` (FK a `cash_register_session` — un gasto en efectivo durante un turno cierra naturalmente contra el arqueo), opcional `supplier` (FK), `note`, `created_by`, `incurred_at`, `created_at`. Tres índices: `(tenant, incurred_at)`, `(tenant, cash_session)`, `(tenant, category, incurred_at)`.
- **`sales_daily`**: rollup `revenue/cash/card/orders` por fecha UTC sobre `order`. Inicialmente intenté `GROUP BY string::slice(<string> created_at, 0, 10)` directo en SurrealQL — falló con `Serialization("expected a string, found 0i64")`. **Gotcha**: en SurrealKv 2.1, el cast `<string> created_at` dentro de un `string::slice` no devuelve un string utilizable como group key — el slice termina re-serializado como int. **Fix**: pull rows + bucket en Rust con `chrono::format("%Y-%m-%d")` + `BTreeMap`. Para datasets single-shop esto es trivial (<10K orders/día); cuando el volumen lo justifique, usar `time::format` directamente.
- **API**: writes role `admin/owner`, reads bearer. `POST /api/v1/expenses`, `GET /api/v1/expenses[?category&payment_method&from&to&limit&offset]`, `GET /api/v1/reports/sales-daily[?from&to]` (tenant-scoped; `refunded/cancelled` excluidos del reporte).
- **Tests** (4 kv-mem): create+list filtrable por category (rent) y payment_method (cash); INVALID_INPUT para `amount=0` y `payment_method='bitcoin'`; sales_daily con 3 ventas pos_cash en la misma fecha UTC agrega `orders=3, revenue=3000, cash=3000`, `date` formato `YYYY-MM-DD`; tenant isolation (otro tenant ve lista vacía y reporte vacío). Workspace verde (118 tests totales), clippy `-D warnings` clean, fmt clean.
- **Compat**: aditivo. Migración 0012 embebida (`include_dir!`) → first-run de upgrade aplica sola. Cero cambio al schema de `order`.
- **Build/MSI/Smoke**: release build (7m15s). MSI `pharma-server-0.1.15-x86_64.msi` 11,984,896 bytes, sha256 `6055ca2a70da2ec114b6ee18ef18d0ce135c7c9512896f152d4bea813c596daf`. Smoke real: stop→`msiexec /i` (MajorUpgrade quitó 0.1.14, exit 0)→Running→`/`=`{"version":"0.1.15"}`→`/health/ready` 200 `db:ok` (migración 0012 aplicada en first-run del upgrade).
- **Release**: `gh release create v0.1.15 --target feature/erp-parity`. PR `feature/erp-parity-expenses-reports` → `feature/erp-parity`. Versión 0.1.14 → 0.1.15 (bump + Cargo.lock mismo commit).
- **Estado vs goal**: ✅ POS clínicamente completo · ✅ caja + gastos + reporte ventas/día · ✅ lazo federado completo · ⏳ reportes avanzados (márgenes/rotación/ABC/vencimientos) — extensiones del mismo patrón, no urgentes · ⏳ Fase 9 firma MSI · ⏳ Fase 10 sync · ⏳ Fase 12 marketplace.
- **Pendiente**: ver `## BACKLOG` al tope.

---

## 2026-05-17 — v0.1.16: backup on-demand (`POST /api/v1/admin/backup`)

- **Qué**: nuevo endpoint que empaqueta el data dir SurrealKv + `agent.key` en un `tar.gz` timestamped bajo `<data_dir>/backups/`. Devuelve `{path, bytes, sha256, started_at, duration_ms}`. Role `admin/owner`. Release v0.1.16, smoke install limpio.
- **Por qué**: Fase 9 vendible v1.0.0 no shippea sin backup confiable. Un cliente real necesita poder respaldar y restaurar — sin esto la única estrategia es "que el VSS de Windows haga snapshot del data dir", lo cual no es accionable desde la app. El endpoint da una salida vendible: un único `.tar.gz` portable que contiene SurrealKv + identidad federada juntos.
- **Implementación** (`crates/api/src/v1/backup.rs`): `AppState.data_dir: Option<PathBuf>` agregado (None en kv-mem tests). `backup_now()` sincrónico (no `spawn_blocking` — datasets single-shop son chicos y el handler tolera el bloqueo del runtime durante un tar; cambiar a spawn_blocking si los tiempos escalan): `tar::Builder` con `flate2::write::GzEncoder` empaqueta el `db_path` bajo `surreal/` + `agent.key` raíz, luego sha256 del archivo final. Path timestamped `pharma-backup-YYYYMMDDTHHMMSSZ.tar.gz`.
- **Decisión locked**: backup NO es tenant-scoped — el dump es per-install. El operador `admin/owner` ve todos los tenants juntos. Si en el futuro hay multi-tenant fuerte se puede agregar un endpoint per-tenant export-JSON; por ahora "una farmacia, una instalación" hace que el backup global sea lo correcto.
- **Decisión locked**: el endpoint NO detiene el servicio. SurrealKv (LSM) tolera lecturas concurrentes con writes; un snapshot puede estar pocos ms desfasado pero es crash-recoverable on restore (WAL replay). Para un backup totalmente quiesced, el operador detiene el servicio Windows antes (documentar en MSI flow Fase 9).
- **Gotcha tropezado (no en memoria — no es duradero)**: usar `route_layer(role::layer(...))` con la firma `Stack<Extension, FromFnLayer>` actual sobre un Router de una sola ruta + merge() dio `Missing request extension: AllowedRoles` en tests con axum 0.8 — el `Extension` no llegaba al middleware. Workaround pragmático para esta slice: chequeo `require_admin` inline en el handler (1 línea, lee `claims.roles`). El resto de los endpoints siguen funcionando con `route_layer` porque tienen múltiples rutas y un `reads.merge(writes)` final. Investigar el patrón mínimo que reproduce el bug → BACKLOG menor.
- **Tests** (2 integration en `crates/api/tests/backup.rs`): admin bearer → 201 con report cuyo `.tar.gz` en disco matchea `bytes` + `sha256` + contiene `agent.key` + al menos una entrada bajo `surreal/`; sin bearer → 401. Workspace verde (120+ tests).
- **Compat**: aditivo. Sin migración. Sin tocar otros endpoints.
- **Build/MSI/Smoke**: release build (7m19s). MSI `pharma-server-0.1.16-x86_64.msi` 11,984,896 bytes (mismo tamaño que 0.1.15 — solo wiring y dos deps lean), sha256 `2f372d3285c24a7af9598b685167759254e313cb93911a4b35ebcfbc316e3482`. Smoke real: stop→`msiexec /i` (MajorUpgrade quitó 0.1.15, exit 0)→Running→`/`=`{"version":"0.1.16"}`→`/health/ready` 200 `db:ok`.
- **Release**: `gh release create v0.1.16 --target feature/erp-parity`. PR `feature/erp-parity-backup` → `feature/erp-parity`. Versión 0.1.15 → 0.1.16 (bump + Cargo.lock mismo commit).
- **Pendiente Fase 8 derivado**: cron scheduler (jobs crate ya tiene el `tokio_cron_scheduler` boilerplate) puede ahora invocar `backup_now` programáticamente para backup automático nocturno. Próximo slice.
- **Pendiente**: ver `## BACKLOG` al tope.

---

## 2026-05-17 — Multi-lot split traceability sales (cierra BACKLOG #3)

- **Qué**: `POST /pos/sale` persiste el desglose COMPLETO de lotes FEFO consumidos por línea, no sólo el lote primario. PR #24 mergeado a `feature/erp-parity` (commit `fb68af6`).
- **Por qué**: el propio código en `crates/domain/src/sales/repo.rs:287-289` apuntaba a BACKLOG #3 ("Multi-lot split traceability: see BACKLOG"). Sin esto, refunds/auditoría/recalls solo conocen el lote head; si una venta consume A=4 + B=1, B queda fuera del trail. Ítem aún sin tomar mientras otras sesiones cerraban v0.1.11→v0.1.16 → pick de menor colisión + alta utilidad.
- **Diseño**: campo `order_item.batches_json: option<string>` (migración `0013_order_item_batches.surql`, additive). Mismo patrón JSON-string ya probado en `agent_order.lines_json` — sidestepea el trap SurrealQL de bindear arrays-of-objects. Legacy `batch` (lote head) intacto → backward compat total. Rows viejas + líneas sin FEFO quedan NULL.
- **Implementación**: `OrderItemDto.batches: Option<Vec<OrderItemBatchAllocation{batch,qty}>>` parseado on-read (silently None en payload inválido — fallback al `batch` primario). `apply_sale` escribe en orden FEFO de consumo, sum(qty)=quantity. Nuevo struct `OrderItemBatchAllocation` en `sales::model` con Serialize+Deserialize+ToSchema.
- **Tests**: test FEFO existente (`pos_sale_batch_tracked_fefo_decrements_earliest_expiry`) extendido — 5 unidades sobre lotes A=4(exp+30d) + B=10(exp+120d) → `batches=[{A,4},{B,1}]`, sum=5, `batch=A` (legacy compat). Test fallback (`pos_sale_non_batch_tracked_falls_back_to_product_stock`) asserta `batches.is_none()`. sales 16/16, workspace verde, clippy `-D warnings` clean, fmt clean.
- **Pendiente menor** (anota BACKLOG #2): el mismo split debe escribirse en `agent_fulfill` (path federado) — hoy decrementa stock con FEFO pero `agent_order.lines_json` no lleva el breakdown por allocation. Próximo slice.
- **Sin bump versión** (otras sesiones bumpearon 0.1.15→0.1.16 en paralelo sin esperar este commit; queda en pool 0.1.16+).
- **Estado vs goal**: ✅ trazabilidad multi-lote completa en POS · ⏳ replicar en path federado · ⏳ Fase 9 firma, Fase 10 sync, Fase 12 marketplace.
- **Pendiente**: ver `## BACKLOG` al tope.

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use axum_prometheus::PrometheusMetricLayerBuilder;
use serde::Serialize;

pub mod error;
mod health;
mod middleware;
pub mod openapi;
mod routes;
pub mod stock_webhook;
mod v1;

pub use middleware::{audit, rate_limit, role};

#[derive(Clone)]
pub struct AppState {
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub jwt: pharma_core::config::JwtConfig,
    pub db: Option<Arc<db::Db>>,
    pub metrics_token: Option<String>,
    /// Node federation identity (Ed25519). Loaded at startup; `None` only in
    /// unit tests that don't exercise `/agent/*`.
    pub node_identity: Option<Arc<agent::Identity>>,
    /// SurrealKv data directory on disk. Used by the backup endpoint to know
    /// what to tar. `None` in unit tests with kv-mem.
    pub data_dir: Option<std::path::PathBuf>,
    /// Node license — controls feature entitlement. Falls back to
    /// [`license::License::free_default`] when `data/license.json` is missing
    /// or invalid (invariant: core gratis always works — ADR-0005).
    /// `ArcSwap` allows lock-free hot-reload from
    /// `POST /api/v1/admin/license/reload` without restarting the service.
    pub license: Arc<arc_swap::ArcSwap<license::License>>,
    /// On-disk path that `POST /api/v1/admin/license/reload` re-reads. `None`
    /// only in unit tests with kv-mem.
    pub license_path: Option<std::path::PathBuf>,
    /// Per-tenant + per-IP rate-limit state. `None` disables both limiters
    /// (most unit tests). Production wires it from `AppConfig.rate_limit`.
    pub rate_limit: Option<Arc<rate_limit::RateLimitState>>,
    /// Serve interactive API docs (Swagger UI + OpenAPI JSON). Mirrors
    /// `AppConfig.docs.enabled`. Default `true`; flip off on hardened boxes to
    /// keep the API surface off the wire.
    pub docs_enabled: bool,
    /// Public read-only catalog endpoint config. Default `enabled = false` so
    /// the route returns 404 (ADR-0005 opt-in).
    pub public_catalog: pharma_core::config::PublicCatalogConfig,
    /// Public web-push orders endpoint config (ADR-0012 pattern 2). Default
    /// `enabled = false` + empty secret so the route returns 404.
    pub public_orders: pharma_core::config::PublicOrdersConfig,
    /// ERP→web stock-change webhook config (ADR-0013). Always present; a
    /// disabled/empty config makes the dispatcher a no-op (the common case).
    pub stock_webhook: Arc<stock_webhook::StockWebhookConfig>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/app", get(app_index))
        .merge(health::router())
        .merge(openapi::router(&state))
        .merge(routes::router())
        .merge(v1::router(state.clone()))
        .layer(audit::layer(state.clone()))
        .with_state(state)
}

/// Stable per-machine data root. The Windows service runs as LocalSystem with
/// CWD = `C:\Windows\System32`, and the MSI ships no `config/`, so a relative
/// `db.path` would otherwise land under System32. Anchor it to the same
/// `%ProgramData%\PharmaServer` dir the installer (`main.wxs` DATAFOLDER)
/// creates, so data is backed up and removed together. Dev (non-Windows, or an
/// absolute path) keeps its existing behavior.
#[cfg(windows)]
fn install_data_base() -> std::path::PathBuf {
    std::env::var_os("ProgramData")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData"))
        .join("PharmaServer")
}

#[cfg(not(windows))]
fn install_data_base() -> std::path::PathBuf {
    std::path::PathBuf::from(".")
}

fn resolve_data_path(p: &str) -> String {
    let path = std::path::Path::new(p);
    if path.is_absolute() {
        return p.to_string();
    }
    let rel = path.strip_prefix(".").unwrap_or(path);
    install_data_base().join(rel).to_string_lossy().into_owned()
}

/// Placeholder JWT secret shipped in `config/default.toml`. Booting production
/// with this value means every issued token is forgeable by anyone who reads
/// the public default config → full auth bypass + cross-tenant access.
const PLACEHOLDER_JWT_SECRET: &str = "change-me-in-production";

/// SECURITY (preprod review F3): reject the placeholder JWT secret unless the
/// caller explicitly opts into insecure local dev. `allow_insecure` is normally
/// `env PHARMA_ALLOW_INSECURE_JWT == "1"`. Returns `Err` when boot must refuse.
fn check_jwt_secret(secret: &str, allow_insecure: bool) -> anyhow::Result<()> {
    if secret == PLACEHOLDER_JWT_SECRET && !allow_insecure {
        anyhow::bail!(
            "JWT secret es el placeholder por defecto ('{PLACEHOLDER_JWT_SECRET}'). \
             Inyecta un secreto fuerte vía PHARMA__JWT__SECRET (ej: `openssl rand -hex 32`) \
             antes de iniciar en producción. Para desarrollo local: PHARMA_ALLOW_INSECURE_JWT=1."
        );
    }
    Ok(())
}

pub async fn run(mut cfg: pharma_core::config::AppConfig) -> anyhow::Result<()> {
    let allow_insecure = std::env::var("PHARMA_ALLOW_INSECURE_JWT").as_deref() == Ok("1");
    check_jwt_secret(&cfg.jwt.secret, allow_insecure)?;

    let resolved = resolve_data_path(&cfg.db.path);
    if resolved != cfg.db.path {
        tracing::info!(from = %cfg.db.path, to = %resolved, "db path anchored to install data dir");
        cfg.db.path = resolved;
    }
    if let Some(parent) = std::path::Path::new(&cfg.db.path).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!(error = %e, dir = %parent.display(), "could not create data dir");
        }
    }

    let db_handle = match db::connect(&cfg.db).await {
        Ok(h) => match db::run_embedded(&h).await {
            Ok(outcomes) => {
                let applied: Vec<&str> = outcomes
                    .iter()
                    .filter(|o| o.applied)
                    .map(|o| o.id.as_str())
                    .collect();
                tracing::info!(
                    total = outcomes.len(),
                    applied = ?applied,
                    skipped = outcomes.len() - applied.len(),
                    "startup migrations complete"
                );
                Some(Arc::new(h))
            }
            Err(e) => {
                tracing::error!(error = %e, "startup migrations FAILED; serving degraded (no schema) — /health/ready will report unavailable");
                None
            }
        },
        Err(e) => {
            tracing::warn!(error = %e, "db connect failed, /health/ready will report degraded");
            None
        }
    };

    let metrics_token = cfg.metrics.token.clone().filter(|t| !t.is_empty());
    if metrics_token.is_none() {
        tracing::warn!("metrics token not configured; /metrics will return 401");
    }

    // Node federation identity: persisted alongside the SurrealKv data dir so
    // it is backed up with tenant data. Generated once, reused thereafter.
    let node_identity = {
        let db_path = std::path::PathBuf::from(&cfg.db.path);
        let dir = db_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let key_path = dir.join("agent.key");
        match agent::Identity::load_or_init(&key_path) {
            Ok(id) => {
                tracing::info!(did = %id.did(), key = %key_path.display(), "node identity ready");
                Some(Arc::new(id))
            }
            Err(e) => {
                tracing::warn!(error = %e, "node identity init failed; /agent/* disabled");
                None
            }
        }
    };

    // License: load from `<data_dir>/license.json` if present and valid;
    // otherwise fall back to Free-tier default. Errors are logged but never
    // block startup — core ERP must always run (ADR-0005).
    let (license, license_path) = {
        let db_path = std::path::PathBuf::from(&cfg.db.path);
        let dir = db_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let path = license::default_license_path(dir);
        let lic = load_license_from(&path);
        (Arc::new(arc_swap::ArcSwap::from_pointee(lic)), Some(path))
    };

    let state = AppState {
        started_at: chrono::Utc::now(),
        jwt: cfg.jwt.clone(),
        db: db_handle,
        metrics_token,
        node_identity,
        data_dir: Some(std::path::PathBuf::from(&cfg.db.path)),
        license,
        license_path,
        rate_limit: Some(Arc::new(rate_limit::RateLimitState::new(
            cfg.rate_limit.clone(),
        ))),
        docs_enabled: cfg.docs.enabled,
        public_catalog: cfg.public_catalog.clone(),
        public_orders: cfg.public_orders.clone(),
        stock_webhook: Arc::new(cfg.stock_webhook.clone()),
    };
    if state.stock_webhook.enabled && !state.stock_webhook.target_url.is_empty() {
        tracing::info!(
            target = %state.stock_webhook.target_url,
            "stock webhook enabled (ERP→web push, ADR-0013)"
        );
    }

    let (prom_layer, prom_handle) = PrometheusMetricLayerBuilder::new()
        .with_prefix("pharma")
        .with_ignore_patterns(&["/metrics", "/health/live", "/health/ready"])
        .with_default_metrics()
        .build_pair();

    let metrics_router = Router::new()
        .route(
            "/metrics",
            get(
                move |State(s): State<AppState>, headers: axum::http::HeaderMap| {
                    let h = prom_handle.clone();
                    async move {
                        match authorize_metrics(&s, &headers) {
                            Ok(()) => h.render().into_response(),
                            Err((status, msg)) => {
                                (status, Json(serde_json::json!({ "error": msg }))).into_response()
                            }
                        }
                    }
                },
            ),
        )
        .with_state(state.clone());

    // Scheduler hub (Fase 8). One `JobScheduler` hosts every cron-driven
    // job: nightly backup + retention prune, hourly idempotency_key TTL
    // purge. Spawned even if the backup schedule is empty, so the TTL purge
    // still runs.
    let sched_db = state.db.clone();
    let sched_data = state.data_dir.clone();
    let backup_sched = cfg.backup.schedule.clone();
    let retention = cfg.backup.retention_days;
    // CRL refresh (opt-in, ADR-0006). Sin `url` ⇒ `None` ⇒ el hub no agenda
    // nada (cero red). El cache CRL vive junto al `license.json`
    // (= `license_path.parent()`), que NO es `data_dir` (subdir surreal) — por
    // eso derivamos el dir del `license_path`, igual que `load_license_from`.
    let crl_refresh = cfg.crl.url.as_ref().filter(|u| !u.is_empty()).map(|url| {
        let crl_dir = state
            .license_path
            .as_ref()
            .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
            .or_else(|| state.data_dir.clone())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        CrlRefresh {
            url: url.clone(),
            schedule: cfg
                .crl
                .schedule
                .clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "0 0 */6 * * *".to_string()),
            data_dir: crl_dir,
            license_path: state.license_path.clone(),
            license: state.license.clone(),
        }
    });
    tokio::spawn(async move {
        if let Err(e) =
            spawn_scheduler_hub(backup_sched, sched_data, retention, sched_db, crl_refresh).await
        {
            tracing::error!(error = %e, "scheduler hub failed to start");
        }
    });

    let app = build_router(state).merge(metrics_router).layer(prom_layer);

    let addr: SocketAddr = cfg.bind.parse()?;
    tracing::info!(%addr, "pharma-api listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Scheduler hub. One `JobScheduler` hosts every cron-driven job:
/// * backup (`backup_schedule`, if non-empty) + retention prune,
/// * idempotency_key TTL purge (hourly, always on when a DB is present).
async fn spawn_scheduler_hub(
    backup_schedule: Option<String>,
    db_path: Option<std::path::PathBuf>,
    retention_days: u32,
    db: Option<Arc<db::Db>>,
    crl_refresh: Option<CrlRefresh>,
) -> anyhow::Result<()> {
    let sched = tokio_cron_scheduler::JobScheduler::new().await?;

    // Backup job (optional).
    if let (Some(schedule), Some(db_path)) = (
        backup_schedule.as_ref().filter(|s| !s.is_empty()).cloned(),
        db_path.clone(),
    ) {
        let job = backup_job(&schedule, db_path, retention_days)?;
        sched.add(job).await?;
        tracing::info!(%schedule, retention_days, "backup scheduler started");
    }

    // Idempotency TTL purge (fixed cadence; hourly is plenty given the 24h
    // TTL of stored keys). Skipped if no DB handle is wired.
    if let Some(db) = db.clone() {
        let job = idempotency_purge_job(db)?;
        sched.add(job).await?;
        tracing::info!("idempotency TTL purge job started (hourly)");
    }

    // CRL refresh (opt-in, ADR-0006). Only scheduled when an endpoint is set.
    if let Some(crl) = crl_refresh {
        let schedule = crl.schedule.clone();
        let url = crl.url.clone();
        let job = crl_refresh_job(crl)?;
        sched.add(job).await?;
        tracing::info!(%schedule, %url, "CRL refresh job started (ADR-0006)");
    }

    sched.start().await?;
    // Hold the scheduler alive for the lifetime of the process.
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}

fn backup_job(
    schedule: &str,
    db_path: std::path::PathBuf,
    retention_days: u32,
) -> anyhow::Result<tokio_cron_scheduler::Job> {
    let job_path = db_path.clone();
    let job = tokio_cron_scheduler::Job::new_async(schedule, move |_uuid, _l| {
        let p = job_path.clone();
        Box::pin(async move {
            let p_run = p.clone();
            let result = tokio::task::spawn_blocking(move || v1::backup_now(&p_run)).await;
            match result {
                Ok(Ok(rep)) => {
                    tracing::info!(
                        path = %rep.path, bytes = rep.bytes, sha256 = %rep.sha256,
                        duration_ms = rep.duration_ms,
                        "scheduled backup completed"
                    );
                }
                Ok(Err(e)) => tracing::error!(error = %e, "scheduled backup failed"),
                Err(e) => tracing::error!(error = %e, "scheduled backup task panicked"),
            }
            if retention_days > 0 {
                let p_prune = p.clone();
                if let Ok(Ok(removed)) =
                    tokio::task::spawn_blocking(move || v1::prune_backups(&p_prune, retention_days))
                        .await
                {
                    if removed > 0 {
                        tracing::info!(removed, "pruned old backups");
                    }
                }
            }
        })
    })?;
    Ok(job)
}

/// Hourly job that drops `idempotency_key` rows whose `expires_at` has passed.
/// `0 0 * * * *` = every hour, second 0.
fn idempotency_purge_job(db: Arc<db::Db>) -> anyhow::Result<tokio_cron_scheduler::Job> {
    let job = tokio_cron_scheduler::Job::new_async("0 0 * * * *", move |_uuid, _l| {
        let db = db.clone();
        Box::pin(async move {
            match domain::sales::service::purge_expired_idempotency(db.as_ref()).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(removed = n, "purged expired idempotency keys"),
                Err(e) => tracing::error!(error = %e, "idempotency purge failed"),
            }
        })
    })?;
    Ok(job)
}

/// Parámetros del job de refresh CRL (ADR-0006). Sólo se construye cuando hay
/// un endpoint configurado; ver `run`. `data_dir` es el dir que aloja
/// `license.json` + `crl_state.json` (= `license_path.parent()`), no el subdir
/// SurrealKv.
struct CrlRefresh {
    url: String,
    schedule: String,
    data_dir: std::path::PathBuf,
    license_path: Option<std::path::PathBuf>,
    license: Arc<arc_swap::ArcSwap<license::License>>,
}

/// Job periódico que recorre la cadena firmada de CRLs del CDN, la aplica al
/// cache local y re-evalúa la license activa (degrade-to-Free lock-free vía
/// `ArcSwap` si quedó revocada — ADR-0005 §6, nunca kill-switch). Best-effort:
/// cualquier error de red/verify se loguea y se ignora; el core nunca se
/// bloquea por el estado del CRL.
fn crl_refresh_job(p: CrlRefresh) -> anyhow::Result<tokio_cron_scheduler::Job> {
    let job = tokio_cron_scheduler::Job::new_async(p.schedule.as_str(), move |_uuid, _l| {
        let url = p.url.clone();
        let dir = p.data_dir.clone();
        let lic_path = p.license_path.clone();
        let lic = p.license.clone();
        Box::pin(async move {
            let res = refresh_crl_once(&url, &dir).await;
            match &res {
                Ok(0) => {}
                Ok(n) => {
                    tracing::info!(applied = n, "CRL refresh: aplicadas versiones nuevas");
                    // Re-evaluar revocación: `load_license_from` re-lee el
                    // license.json + el crl_state.json recién escrito y degrada
                    // a Free si la license activa quedó revocada.
                    if let Some(path) = lic_path.as_ref() {
                        let fresh = load_license_from(path);
                        lic.store(std::sync::Arc::new(fresh));
                        tracing::info!("license re-evaluada tras CRL refresh");
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "CRL refresh falló (best-effort, ignorado)");
                }
            }
            // Observabilidad: `pharma_crl_refresh_total{result}` con result en un
            // set cerrado (applied|noop|error) — cardinalidad baja, sin PII. Un
            // alza de `error` = el nodo no puede alcanzar el CDN y se está
            // quedando ciego a revocaciones (relevante para seguridad), pese a
            // que el core siga operativo (best-effort, ADR-0005).
            metrics::counter!("pharma_crl_refresh_total", "result" => crl_refresh_label(&res))
                .increment(1);
        })
    })?;
    Ok(job)
}

/// Etiqueta de métrica para una pasada de refresh CRL — set cerrado para
/// mantener baja la cardinalidad de `pharma_crl_refresh_total{result}`.
fn crl_refresh_label(res: &anyhow::Result<u64>) -> &'static str {
    match res {
        Ok(0) => "noop",
        Ok(_) => "applied",
        Err(_) => "error",
    }
}

/// Una pasada de refresh: descarga + aplica la cadena de diffs desde el CDN.
/// Usa las claves de producción embebidas. Devuelve cuántas versiones se
/// aplicaron (0 = el cache ya estaba al día).
async fn refresh_crl_once(base_url: &str, data_dir: &std::path::Path) -> anyhow::Result<u64> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let base = base_url.trim_end_matches('/').to_string();
    apply_crl_chain(data_dir, license::keys::LICENSER_KEYS, move |n| {
        let client = client.clone();
        let base = base.clone();
        async move {
            let url = format!("{base}/crl-v{n}.json");
            let resp = client.get(&url).send().await?;
            // 404 = no hay más versiones (cabeza de la cadena) → parar limpio.
            if resp.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            let bytes = resp.error_for_status()?.bytes().await?;
            Ok(Some(bytes.to_vec()))
        }
    })
    .await
}

/// Núcleo HTTP-agnóstico del refresh: recorre la cadena `crl-v{N}.json` hacia
/// adelante desde `last_seen + 1`, verificando + aplicando cada versión, hasta
/// que `fetch` devuelve `None` (no existe esa versión todavía → cabeza de la
/// cadena). Persiste el cache sólo si se aplicó algo. Puro de red (el `fetch`
/// se inyecta) para poder testearlo sin socket. `apply_version` ya garantiza
/// monotonicidad + verificación de firma, así que un blob alterado o fuera de
/// secuencia aborta la pasada sin corromper el cache en disco.
async fn apply_crl_chain<F, Fut>(
    data_dir: &std::path::Path,
    keys: &[(&str, &str)],
    mut fetch: F,
) -> anyhow::Result<u64>
where
    F: FnMut(u64) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Option<Vec<u8>>>>,
{
    let state_path = license::default_crl_state_path(data_dir);
    let mut state = license::load_crl_state(&state_path)?;
    let mut applied = 0u64;
    // Cota dura por pasada: un nodo muy atrasado se pone al día en pasadas
    // sucesivas en vez de bloquear el scheduler. (El bootstrap por snapshot es
    // la vía rápida vía `pharma license crl-import --snapshot`.)
    const MAX_PER_RUN: u64 = 1000;
    for _ in 0..MAX_PER_RUN {
        let next = state.last_seen_version + 1;
        let Some(bytes) = fetch(next).await? else {
            break;
        };
        let v = license::crl::parse_and_verify_crl_with_keys(&bytes, keys)?;
        state.apply_version(&v)?;
        applied += 1;
    }
    if applied > 0 {
        if let Some(parent) = state_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        license::save_crl_state(&state, &state_path)?;
    }
    Ok(applied)
}

/// Read + verify a license from disk, falling back to `License::free_default`
/// on any error. Logs both paths. Returned by both startup and the
/// `admin/license/reload` handler, so the policy stays in one place.
///
/// Revocación (ADR-0006): si existe un cache CRL (`crl_state.json`, hermano
/// del license.json) y lista el `license_id`, la license degrada a Free —
/// nunca kill-switch (ADR-0005 §6). Cache CRL ausente/corrupto ⇒ se ignora
/// (offline-first: la revocación es best-effort, no bloqueante).
pub fn load_license_from(path: &std::path::Path) -> license::License {
    load_license_from_with_keys(path, license::keys::LICENSER_KEYS)
}

/// Variante de [`load_license_from`] con tabla de claves del licenser
/// inyectable — mirrors `license::parse_and_verify_with_keys`. Producción
/// siempre pasa las claves embebidas (`LICENSER_KEYS`); los tests E2E pasan un
/// keypair efímero para mintar una license firmada sin la clave privada real.
/// La consulta del CRL (`crl_state.json`) NO usa claves: es el cache local ya
/// aplicado (estado confiable), sólo se chequea pertenencia al set revocado.
pub fn load_license_from_with_keys(
    path: &std::path::Path,
    keys: &[(&str, &str)],
) -> license::License {
    if !path.exists() {
        tracing::info!(path = %path.display(), "no license file; running Free tier");
        return license::License::free_default(uuid::Uuid::nil());
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(),
                "license file unreadable; falling back to Free");
            return license::License::free_default(uuid::Uuid::nil());
        }
    };
    match license::verify::parse_and_verify_with_keys(&bytes, keys) {
        Ok(lic) => finalize_license_with_crl(lic, path),
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(),
                "license file present but invalid; falling back to Free");
            license::License::free_default(uuid::Uuid::nil())
        }
    }
}

/// Aplica la política de revocación (ADR-0006) a una license ya verificada:
/// si el cache CRL local (`crl_state.json`, hermano del `license.json`) la
/// lista, degrada a Free — nunca kill-switch (ADR-0005 §6). Cache
/// ausente/corrupto ⇒ se ignora (offline-first: revocación best-effort).
fn finalize_license_with_crl(lic: license::License, path: &std::path::Path) -> license::License {
    if let Some(dir) = path.parent() {
        let crl_path = license::default_crl_state_path(dir);
        match license::load_crl_state(&crl_path) {
            Ok(state) if state.is_revoked(&lic.license_id) => {
                tracing::warn!(
                    license_id = %lic.license_id,
                    crl_version = state.last_seen_version,
                    "license REVOCADA según CRL local; degradando a Free \
                     (core gratis sigue operativo, ADR-0005)"
                );
                return license::License::free_default(uuid::Uuid::nil());
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, path = %crl_path.display(),
                    "crl_state.json ilegible; se ignora (revocación best-effort)");
            }
        }
    }
    tracing::info!(
        tier = lic.tier.as_str(),
        license_id = %lic.license_id,
        features = lic.features.len(),
        path = %path.display(),
        "license loaded"
    );
    lic
}

pub fn default_config() -> pharma_core::config::AppConfig {
    pharma_core::config::AppConfig {
        bind: "0.0.0.0:8080".into(),
        db: pharma_core::config::DbConfig {
            path: "./data/surreal".into(),
            namespace: "pharma".into(),
            database: "main".into(),
        },
        jwt: pharma_core::config::JwtConfig {
            secret: "change-me".into(),
            issuer: "pharma-server".into(),
            ttl_seconds: 3600,
        },
        otlp: pharma_core::config::OtlpConfig {
            endpoint: None,
            service_name: "pharma-api".into(),
        },
        metrics: pharma_core::config::MetricsConfig { token: None },
        backup: pharma_core::config::BackupConfig::default(),
        rate_limit: pharma_core::config::RateLimitConfig::default(),
        docs: pharma_core::config::DocsConfig::default(),
        public_catalog: pharma_core::config::PublicCatalogConfig::default(),
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
        stock_webhook: pharma_core::config::StockWebhookConfig::default(),
        crl: pharma_core::config::CrlConfig::default(),
    }
}

fn authorize_metrics(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<(), (StatusCode, &'static str)> {
    let expected = state
        .metrics_token
        .as_deref()
        .ok_or((StatusCode::UNAUTHORIZED, "metrics endpoint not configured"))?;
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or((StatusCode::UNAUTHORIZED, "missing bearer token"))?;
    if constant_time_eq(expected.as_bytes(), supplied.as_bytes()) {
        Ok(())
    } else {
        Err((StatusCode::UNAUTHORIZED, "invalid token"))
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn load_or_default() -> pharma_core::config::AppConfig {
    pharma_core::config::AppConfig::load().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "config load failed, using defaults");
        default_config()
    })
}

#[derive(Serialize)]
struct Root {
    name: &'static str,
    version: &'static str,
}

async fn root() -> Json<Root> {
    Json(Root {
        name: "pharma-server",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn app_index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn jwt_guard_rejects_placeholder_without_optin() {
        let err = check_jwt_secret(PLACEHOLDER_JWT_SECRET, false).unwrap_err();
        assert!(err.to_string().contains("PHARMA__JWT__SECRET"));
    }

    #[test]
    fn jwt_guard_allows_placeholder_with_dev_optin() {
        assert!(check_jwt_secret(PLACEHOLDER_JWT_SECRET, true).is_ok());
    }

    #[test]
    fn jwt_guard_allows_real_secret() {
        assert!(check_jwt_secret("a-strong-injected-secret-xyz", false).is_ok());
    }

    fn state_with(token: Option<&str>) -> AppState {
        AppState {
            started_at: chrono::Utc::now(),
            jwt: pharma_core::config::JwtConfig {
                secret: "x".into(),
                issuer: "x".into(),
                ttl_seconds: 60,
            },
            db: None,
            metrics_token: token.map(String::from),
            node_identity: None,
            data_dir: None,
            license: Arc::new(arc_swap::ArcSwap::from_pointee(
                license::License::free_default(uuid::Uuid::nil()),
            )),
            license_path: None,
            rate_limit: None,
            docs_enabled: true,
            public_catalog: pharma_core::config::PublicCatalogConfig::default(),
            public_orders: pharma_core::config::PublicOrdersConfig::default(),
            stock_webhook: Arc::new(stock_webhook::StockWebhookConfig::default()),
        }
    }

    #[test]
    fn metrics_no_token_configured_returns_401() {
        let s = state_with(None);
        let h = HeaderMap::new();
        let err = authorize_metrics(&s, &h).unwrap_err();
        assert_eq!(err.0, StatusCode::UNAUTHORIZED);
        assert_eq!(err.1, "metrics endpoint not configured");
    }

    #[test]
    fn metrics_missing_header_returns_401() {
        let s = state_with(Some("secret"));
        let h = HeaderMap::new();
        let err = authorize_metrics(&s, &h).unwrap_err();
        assert_eq!(err.1, "missing bearer token");
    }

    #[test]
    fn metrics_wrong_token_returns_401() {
        let s = state_with(Some("secret"));
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "Bearer nope".parse().unwrap());
        let err = authorize_metrics(&s, &h).unwrap_err();
        assert_eq!(err.1, "invalid token");
    }

    #[test]
    fn metrics_correct_token_ok() {
        let s = state_with(Some("secret"));
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "Bearer secret".parse().unwrap());
        authorize_metrics(&s, &h).expect("authorized");
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }
}

#[cfg(test)]
mod crl_refresh_tests {
    use super::*;
    use agent::canonical::canonical_bytes;
    use agent::identity::Identity;
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    use serde_json::{json, Value};
    use std::collections::HashMap;

    /// Firma un `crl-v{N}.json` con un keypair efímero (mismo esquema que
    /// `crates/license` tests: canonical-JSON sin `signature` → Ed25519 → b64).
    fn sign_crl(
        id: &Identity,
        key_id: &str,
        version: u64,
        prev: Option<u64>,
        added: &[&str],
    ) -> Vec<u8> {
        let mut v = json!({
            "schema_version": 1,
            "crl_version": version,
            "previous_version": prev,
            "published_at": "2026-06-13T00:00:00Z",
            "diff": {
                "added": added.iter().map(|l| json!({
                    "license_id": l,
                    "revoked_at": "2026-06-12T00:00:00Z",
                })).collect::<Vec<_>>(),
                "removed": [],
            },
            "issuer_did": id.did(),
            "key_id": key_id,
            "signature": "",
        });
        let unsigned = {
            let mut u = v.clone();
            u.as_object_mut().unwrap().remove("signature");
            canonical_bytes(&u).unwrap()
        };
        let sig = id.sign(&unsigned);
        v["signature"] = Value::String(B64.encode(sig.to_bytes()));
        serde_json::to_vec(&v).unwrap()
    }

    #[tokio::test]
    async fn walks_chain_persists_and_stops_on_404() {
        let id = Identity::generate();
        let key_id = "lk-test";
        let did = id.did();
        let keys: &[(&str, &str)] = &[(key_id, &did)];

        let mut cdn: HashMap<u64, Vec<u8>> = HashMap::new();
        cdn.insert(1, sign_crl(&id, key_id, 1, None, &["lic_a"]));
        cdn.insert(2, sign_crl(&id, key_id, 2, Some(1), &["lic_b"]));

        let dir = std::env::temp_dir().join(format!("crl-refresh-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        // Primera pasada: aplica v1 + v2, para en v3 (404 ⇒ None).
        let applied = apply_crl_chain(&dir, keys, |n| {
            let blob = cdn.get(&n).cloned();
            async move { Ok(blob) }
        })
        .await
        .unwrap();
        assert_eq!(applied, 2);

        let state = license::load_crl_state(&license::default_crl_state_path(&dir)).unwrap();
        assert_eq!(state.last_seen_version, 2);
        assert!(state.is_revoked("lic_a") && state.is_revoked("lic_b"));

        // Segunda pasada idempotente: v3 sigue 404 ⇒ 0 aplicadas.
        let again = apply_crl_chain(&dir, keys, |n| {
            let blob = cdn.get(&n).cloned();
            async move { Ok(blob) }
        })
        .await
        .unwrap();
        assert_eq!(again, 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn refresh_label_maps_result_to_closed_set() {
        assert_eq!(crl_refresh_label(&Ok(0)), "noop");
        assert_eq!(crl_refresh_label(&Ok(3)), "applied");
        assert_eq!(
            crl_refresh_label(&Err(anyhow::anyhow!("red caída"))),
            "error"
        );
    }

    #[tokio::test]
    async fn tampered_blob_aborts_without_writing_cache() {
        let id = Identity::generate();
        let key_id = "lk-test";
        let did = id.did();
        let keys: &[(&str, &str)] = &[(key_id, &did)];

        // Firma con `lic_a`, luego altera el payload a `lic_z` ⇒ firma inválida.
        let tampered = String::from_utf8(sign_crl(&id, key_id, 1, None, &["lic_a"]))
            .unwrap()
            .replace("lic_a", "lic_z")
            .into_bytes();

        let dir = std::env::temp_dir().join(format!("crl-refresh-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let res = apply_crl_chain(&dir, keys, |_n| {
            let blob = tampered.clone();
            async move { Ok(Some(blob)) }
        })
        .await;
        assert!(res.is_err(), "blob alterado debe abortar la pasada");

        // Nada aplicado ⇒ el cache no se escribió (sigue en default).
        let state = license::load_crl_state(&license::default_crl_state_path(&dir)).unwrap();
        assert_eq!(state.last_seen_version, 0);
        assert!(!state.is_revoked("lic_z"));

        std::fs::remove_dir_all(&dir).ok();
    }
}

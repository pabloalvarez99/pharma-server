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
mod openapi;
mod routes;
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

pub async fn run(mut cfg: pharma_core::config::AppConfig) -> anyhow::Result<()> {
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
    };

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
    tokio::spawn(async move {
        if let Err(e) = spawn_scheduler_hub(backup_sched, sched_data, retention, sched_db).await {
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

/// Read + verify a license from disk, falling back to `License::free_default`
/// on any error. Logs both paths. Returned by both startup and the
/// `admin/license/reload` handler, so the policy stays in one place.
pub fn load_license_from(path: &std::path::Path) -> license::License {
    if path.exists() {
        match license::load_from_disk(path) {
            Ok(lic) => {
                tracing::info!(
                    tier = lic.tier.as_str(),
                    license_id = %lic.license_id,
                    features = lic.features.len(),
                    path = %path.display(),
                    "license loaded"
                );
                lic
            }
            Err(e) => {
                tracing::warn!(error = %e, path = %path.display(),
                    "license file present but invalid; falling back to Free");
                license::License::free_default(uuid::Uuid::nil())
            }
        }
    } else {
        tracing::info!(path = %path.display(), "no license file; running Free tier");
        license::License::free_default(uuid::Uuid::nil())
    }
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

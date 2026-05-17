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

pub use middleware::{audit, role};

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
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/app", get(app_index))
        .merge(health::router())
        .merge(openapi::router())
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

    let state = AppState {
        started_at: chrono::Utc::now(),
        jwt: cfg.jwt.clone(),
        db: db_handle,
        metrics_token,
        node_identity,
        data_dir: Some(std::path::PathBuf::from(&cfg.db.path)),
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

    let app = build_router(state).merge(metrics_router).layer(prom_layer);

    let addr: SocketAddr = cfg.bind.parse()?;
    tracing::info!(%addr, "pharma-api listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
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

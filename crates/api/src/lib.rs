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

pub use middleware::{audit, role};

#[derive(Clone)]
pub struct AppState {
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub jwt: pharma_core::config::JwtConfig,
    pub db: Option<Arc<db::Db>>,
    pub metrics_token: Option<String>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/app", get(app_index))
        .merge(health::router())
        .merge(openapi::router())
        .merge(routes::router())
        .layer(audit::layer(state.clone()))
        .with_state(state)
}

pub async fn run(cfg: pharma_core::config::AppConfig) -> anyhow::Result<()> {
    let db_handle = match db::connect(&cfg.db).await {
        Ok(h) => Some(Arc::new(h)),
        Err(e) => {
            tracing::warn!(error = %e, "db connect failed, /health/ready will report degraded");
            None
        }
    };

    let metrics_token = cfg.metrics.token.clone().filter(|t| !t.is_empty());
    if metrics_token.is_none() {
        tracing::warn!("metrics token not configured; /metrics will return 401");
    }

    let state = AppState {
        started_at: chrono::Utc::now(),
        jwt: cfg.jwt.clone(),
        db: db_handle,
        metrics_token,
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

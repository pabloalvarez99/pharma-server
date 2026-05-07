use std::{net::SocketAddr, sync::Arc};

use axum::{routing::get, Json, Router};
use serde::Serialize;

mod health;
mod middleware;
mod openapi;
mod routes;

#[derive(Clone)]
pub struct AppState {
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub jwt: pharma_core::config::JwtConfig,
    pub db: Option<Arc<db::Db>>,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .merge(health::router())
        .merge(openapi::router())
        .merge(routes::router())
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

    let state = AppState {
        started_at: chrono::Utc::now(),
        jwt: cfg.jwt.clone(),
        db: db_handle,
    };

    let app = build_router(state);

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
    }
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

use std::net::SocketAddr;

use axum::{routing::get, Json, Router};
use serde::Serialize;

mod health;
mod openapi;

#[derive(Clone)]
pub struct AppState {
    pub started_at: chrono::DateTime<chrono::Utc>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    telemetry::init("pharma-api")?;

    let cfg = pharma_core::config::AppConfig::load().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "config load failed, using defaults");
        default_config()
    });

    let state = AppState { started_at: chrono::Utc::now() };

    let app = Router::new()
        .route("/", get(root))
        .merge(health::router())
        .merge(openapi::router())
        .with_state(state);

    let addr: SocketAddr = cfg.bind.parse()?;
    tracing::info!(%addr, "pharma-api listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Serialize)]
struct Root {
    name: &'static str,
    version: &'static str,
}

async fn root() -> Json<Root> {
    Json(Root { name: "pharma-server", version: env!("CARGO_PKG_VERSION") })
}

fn default_config() -> pharma_core::config::AppConfig {
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

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init(service_name: &str) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,axum=info,tower_http=info,surrealdb=warn"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json().with_current_span(true).with_span_list(false))
        .try_init()?;
    tracing::info!(service = service_name, "telemetry initialized");
    Ok(())
}

use anyhow::Context;
use pharma_core::config::OtlpConfig;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init(service_name: &str) -> anyhow::Result<()> {
    init_inner(service_name, None)
}

pub fn init_with_otlp(service_name: &str, otlp: &OtlpConfig) -> anyhow::Result<()> {
    init_inner(service_name, Some(otlp))
}

fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,axum=info,tower_http=info,surrealdb=warn"))
}

fn init_inner(service_name: &str, otlp: Option<&OtlpConfig>) -> anyhow::Result<()> {
    let fmt_layer = fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(false);

    let otlp_layer = match otlp
        .and_then(|c| c.endpoint.as_deref())
        .filter(|s| !s.is_empty())
    {
        Some(endpoint) => Some(build_otlp_layer(otlp.unwrap(), endpoint)?),
        None => None,
    };

    tracing_subscriber::registry()
        .with(env_filter())
        .with(fmt_layer)
        .with(otlp_layer)
        .try_init()?;

    tracing::info!(
        service = service_name,
        otlp = otlp
            .and_then(|c| c.endpoint.as_deref())
            .filter(|s| !s.is_empty())
            .unwrap_or("disabled"),
        "telemetry initialized"
    );
    Ok(())
}

fn build_otlp_layer<S>(
    cfg: &OtlpConfig,
    endpoint: &str,
) -> anyhow::Result<tracing_opentelemetry::OpenTelemetryLayer<S, opentelemetry_sdk::trace::Tracer>>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    use opentelemetry::{trace::TracerProvider as _, KeyValue};
    use opentelemetry_otlp::{SpanExporter, WithExportConfig};
    use opentelemetry_sdk::{runtime, trace::TracerProvider, Resource};

    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .context("build OTLP gRPC span exporter")?;

    let resource = Resource::new(vec![KeyValue::new(
        "service.name",
        cfg.service_name.clone(),
    )]);

    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, runtime::Tokio)
        .with_resource(resource)
        .build();

    let tracer = provider.tracer(cfg.service_name.clone());
    opentelemetry::global::set_tracer_provider(provider);

    Ok(tracing_opentelemetry::layer().with_tracer(tracer))
}

pub fn shutdown() {
    opentelemetry::global::shutdown_tracer_provider();
}

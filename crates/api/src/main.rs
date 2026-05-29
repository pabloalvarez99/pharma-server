#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = api::load_or_default();
    pharma_telemetry::init_with_otlp("pharma-api", &cfg.otlp)?;
    let result = api::run(cfg).await;
    pharma_telemetry::shutdown();
    result
}

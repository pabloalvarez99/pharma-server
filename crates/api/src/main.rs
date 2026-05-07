#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = api::load_or_default();
    telemetry::init_with_otlp("pharma-api", &cfg.otlp)?;
    let result = api::run(cfg).await;
    telemetry::shutdown();
    result
}

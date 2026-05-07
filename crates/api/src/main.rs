#[tokio::main]
async fn main() -> anyhow::Result<()> {
    telemetry::init("pharma-api")?;
    let cfg = api::load_or_default();
    api::run(cfg).await
}

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "pharma", version, about = "pharma-server admin CLI")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Apply pending migrations against the configured SurrealDB.
    Migrate,
    /// Create a new tenant.
    TenantCreate { name: String },
    /// Create an admin user for a tenant.
    UserCreate {
        #[arg(long)]
        tenant: String,
        #[arg(long)]
        email: String,
    },
    /// Print effective configuration.
    Config,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = telemetry::init("pharma-cli");
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Migrate => {
            tracing::info!("migrate: TODO — apply migrations/*.surql");
        }
        Cmd::TenantCreate { name } => {
            tracing::info!(%name, "tenant-create: TODO");
        }
        Cmd::UserCreate { tenant, email } => {
            tracing::info!(%tenant, %email, "user-create: TODO");
        }
        Cmd::Config => {
            let cfg = pharma_core::config::AppConfig::load()?;
            println!("{}", serde_json::to_string_pretty(&cfg)?);
        }
    }
    Ok(())
}

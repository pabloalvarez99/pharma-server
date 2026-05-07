use std::path::PathBuf;

use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "pharma", version, about = "pharma-server admin CLI")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Apply pending migrations against the configured SurrealDB.
    Migrate {
        /// Override the migrations directory (default: ./migrations).
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Create a new tenant.
    TenantCreate {
        /// Display name.
        name: String,
        /// URL-safe slug (defaults to lowercased + dash-joined name).
        #[arg(long)]
        slug: Option<String>,
    },
    /// Create a user for a tenant.
    UserCreate {
        /// Tenant slug.
        #[arg(long)]
        tenant: String,
        #[arg(long)]
        email: String,
        /// Comma-separated roles (e.g. admin,pharmacist).
        #[arg(long, default_value = "")]
        roles: String,
        /// Password (read from PHARMA_PASSWORD env or interactive prompt if omitted).
        #[arg(long)]
        password: Option<String>,
    },
    /// Print effective configuration.
    Config,
}

#[derive(Debug, Serialize, Deserialize)]
struct TenantRow {
    id: surrealdb::sql::Thing,
    name: String,
    slug: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct UserRow {
    id: surrealdb::sql::Thing,
    email: String,
    tenant: surrealdb::sql::Thing,
    roles: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = telemetry::init("pharma-cli");
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Migrate { dir } => {
            let cfg = pharma_core::config::AppConfig::load()?;
            let dir = dir.unwrap_or_else(|| PathBuf::from("migrations"));
            let db_handle = db::connect(&cfg.db).await?;
            let outcomes = db::run_migrations(&db_handle, &dir).await?;
            let applied: Vec<_> = outcomes.iter().filter(|o| o.applied).collect();
            let skipped: Vec<_> = outcomes.iter().filter(|o| !o.applied).collect();
            for o in &applied {
                println!("applied  {}", o.id);
            }
            for o in &skipped {
                println!("skipped  {} (already applied)", o.id);
            }
            tracing::info!(
                total = outcomes.len(),
                applied = applied.len(),
                skipped = skipped.len(),
                "migrate complete"
            );
        }
        Cmd::TenantCreate { name, slug } => {
            let slug = slug.unwrap_or_else(|| slugify(&name));
            let cfg = pharma_core::config::AppConfig::load()?;
            let db_handle = db::connect(&cfg.db).await?;

            let mut res = db_handle
                .query("CREATE tenant SET name = $name, slug = $slug RETURN AFTER")
                .bind(("name", name.clone()))
                .bind(("slug", slug.clone()))
                .await
                .context("CREATE tenant query")?;
            let row: Option<TenantRow> = res.take(0)?;
            let row = row.ok_or_else(|| anyhow!("tenant create returned no row"))?;
            println!("tenant created: id={} slug={}", row.id, row.slug);
            tracing::info!(tenant_id = %row.id, %slug, "tenant created");
        }
        Cmd::UserCreate {
            tenant,
            email,
            roles,
            password,
        } => {
            let password = resolve_password(password)?;
            let roles_vec: Vec<String> = roles
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let hash = auth::password::hash(&password)?;

            let cfg = pharma_core::config::AppConfig::load()?;
            let db_handle = db::connect(&cfg.db).await?;

            let mut tenant_q = db_handle
                .query("SELECT * FROM tenant WHERE slug = $slug LIMIT 1")
                .bind(("slug", tenant.clone()))
                .await
                .context("lookup tenant by slug")?;
            let tenant_row: Option<TenantRow> = tenant_q.take(0)?;
            let tenant_row =
                tenant_row.ok_or_else(|| anyhow!("tenant with slug '{tenant}' not found"))?;

            let mut res = db_handle
                .query(
                    "CREATE user SET tenant = $tenant, email = $email, \
                     password = $password, roles = $roles RETURN AFTER",
                )
                .bind(("tenant", tenant_row.id.clone()))
                .bind(("email", email.clone()))
                .bind(("password", hash))
                .bind(("roles", roles_vec.clone()))
                .await
                .context("CREATE user query")?;
            let row: Option<UserRow> = res.take(0)?;
            let row = row.ok_or_else(|| anyhow!("user create returned no row"))?;
            println!(
                "user created: id={} email={} tenant={} roles={:?}",
                row.id, row.email, row.tenant, row.roles
            );
            tracing::info!(user_id = %row.id, tenant = %tenant, %email, "user created");
        }
        Cmd::Config => {
            let cfg = pharma_core::config::AppConfig::load()?;
            println!("{}", serde_json::to_string_pretty(&cfg)?);
        }
    }
    Ok(())
}

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn resolve_password(arg: Option<String>) -> anyhow::Result<String> {
    if let Some(p) = arg {
        return Ok(p);
    }
    if let Ok(p) = std::env::var("PHARMA_PASSWORD") {
        if !p.is_empty() {
            return Ok(p);
        }
    }
    let p = rpassword::prompt_password("Password: ")?;
    if p.is_empty() {
        return Err(anyhow!("password cannot be empty"));
    }
    let confirm = rpassword::prompt_password("Confirm: ")?;
    if p != confirm {
        return Err(anyhow!("passwords do not match"));
    }
    Ok(p)
}

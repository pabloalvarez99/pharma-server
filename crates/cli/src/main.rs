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
    /// List tenants.
    TenantList {
        /// Output as JSON instead of table.
        #[arg(long)]
        json: bool,
    },
    /// List users (optionally filtered by tenant slug).
    UserList {
        /// Tenant slug filter.
        #[arg(long)]
        tenant: Option<String>,
        /// Output as JSON instead of table.
        #[arg(long)]
        json: bool,
    },
    /// Print effective configuration.
    Config,
    /// Agent ecosystem identity (Ed25519) — Fase 11 federation.
    Agent {
        #[command(subcommand)]
        cmd: AgentCmd,
    },
}

#[derive(Subcommand)]
enum AgentCmd {
    /// Generate (idempotent) the node keypair + print the DID.
    Init {
        /// Override key file path (default: <db dir>/agent.key).
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Print this node's DID.
    Did {
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Emit a self-signed AgentCard JSON (for out-of-band discovery).
    Card {
        #[arg(long)]
        name: String,
        /// pharmacy | supplier | distributor | lab
        #[arg(long, default_value = "pharmacy")]
        kind: String,
        /// ISO-3166 region hint, e.g. CL-CO.
        #[arg(long, default_value = "")]
        region: String,
        /// Reachable base URL (LAN ok; empty if relay-only).
        #[arg(long, default_value = "")]
        endpoint: String,
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Verify an AgentCard or Envelope JSON file's signature.
    Verify {
        /// Path to a JSON file (card or envelope).
        file: PathBuf,
    },
}

/// Default agent key path: sibling of the SurrealKv data dir.
fn agent_key_path(explicit: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    let cfg = pharma_core::config::AppConfig::load()?;
    let db_path = PathBuf::from(&cfg.db.path);
    let dir = db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    Ok(dir.join("agent.key"))
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
    let _ = telemetry::init_cli("pharma-cli");
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
        Cmd::TenantList { json } => {
            let cfg = pharma_core::config::AppConfig::load()?;
            let db_handle = db::connect(&cfg.db).await?;
            let mut res = db_handle
                .query("SELECT * FROM tenant ORDER BY slug")
                .await
                .context("SELECT tenant")?;
            let rows: Vec<TenantRow> = res.take(0)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else {
                println!("{:<40}  {:<24}  NAME", "ID", "SLUG");
                for r in &rows {
                    println!("{:<40}  {:<24}  {}", r.id.to_string(), r.slug, r.name);
                }
                println!("({} tenants)", rows.len());
            }
        }
        Cmd::UserList { tenant, json } => {
            let cfg = pharma_core::config::AppConfig::load()?;
            let db_handle = db::connect(&cfg.db).await?;
            let rows: Vec<UserRow> = if let Some(slug) = tenant {
                let mut tq = db_handle
                    .query("SELECT * FROM tenant WHERE slug = $slug LIMIT 1")
                    .bind(("slug", slug.clone()))
                    .await
                    .context("lookup tenant by slug")?;
                let tenant_row: Option<TenantRow> = tq.take(0)?;
                let tenant_row =
                    tenant_row.ok_or_else(|| anyhow!("tenant with slug '{slug}' not found"))?;
                let mut res = db_handle
                    .query("SELECT * FROM user WHERE tenant = $tenant ORDER BY email")
                    .bind(("tenant", tenant_row.id.clone()))
                    .await
                    .context("SELECT user by tenant")?;
                res.take(0)?
            } else {
                let mut res = db_handle
                    .query("SELECT * FROM user ORDER BY email")
                    .await
                    .context("SELECT user")?;
                res.take(0)?
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else {
                println!("{:<40}  {:<32}  {:<24}  ROLES", "ID", "EMAIL", "TENANT");
                for r in &rows {
                    println!(
                        "{:<40}  {:<32}  {:<24}  {}",
                        r.id.to_string(),
                        r.email,
                        r.tenant.to_string(),
                        r.roles.join(",")
                    );
                }
                println!("({} users)", rows.len());
            }
        }
        Cmd::Config => {
            let cfg = pharma_core::config::AppConfig::load()?;
            println!("{}", serde_json::to_string_pretty(&cfg)?);
        }
        Cmd::Agent { cmd } => match cmd {
            AgentCmd::Init { path } => {
                let p = agent_key_path(path)?;
                if let Some(parent) = p.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                let existed = p.exists();
                let id = agent::Identity::load_or_init(&p)?;
                println!("{}", id.did());
                tracing::info!(did = %id.did(), key = %p.display(), reused = existed, "agent identity ready");
            }
            AgentCmd::Did { path } => {
                let p = agent_key_path(path)?;
                let id = agent::Identity::load(&p).with_context(|| {
                    format!("no agent key at {} — run `pharma agent init`", p.display())
                })?;
                println!("{}", id.did());
            }
            AgentCmd::Card {
                name,
                kind,
                region,
                endpoint,
                path,
            } => {
                let p = agent_key_path(path)?;
                let id = agent::Identity::load(&p).with_context(|| {
                    format!("no agent key at {} — run `pharma agent init`", p.display())
                })?;
                let kind = match kind.to_lowercase().as_str() {
                    "pharmacy" => agent::AgentKind::Pharmacy,
                    "supplier" => agent::AgentKind::Supplier,
                    "distributor" => agent::AgentKind::Distributor,
                    "lab" => agent::AgentKind::Lab,
                    other => return Err(anyhow!("kind inválido: {other}")),
                };
                let card = agent::AgentCard::new(&id, name, kind, region, endpoint);
                println!("{}", card.to_json()?);
            }
            AgentCmd::Verify { file } => {
                let content = std::fs::read_to_string(&file)
                    .with_context(|| format!("read {}", file.display()))?;
                // Try card first, then envelope.
                if let Ok(card) = agent::AgentCard::from_json(&content) {
                    match card.verify() {
                        Ok(()) => {
                            println!("OK card  did={} name={}", card.did, card.name);
                            return Ok(());
                        }
                        Err(e) => return Err(anyhow!("card signature INVALID: {e}")),
                    }
                }
                let env = agent::Envelope::from_json(&content)
                    .context("not a valid AgentCard or Envelope JSON")?;
                match env.verify() {
                    Ok(()) => println!(
                        "OK envelope  from={} topic={} msg_id={}",
                        env.from, env.topic, env.msg_id
                    ),
                    Err(e) => return Err(anyhow!("envelope signature INVALID: {e}")),
                }
            }
        },
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

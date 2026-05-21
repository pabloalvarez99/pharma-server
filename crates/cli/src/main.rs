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
    /// License management — import, status, features (Fase 10c).
    License {
        #[command(subcommand)]
        cmd: LicenseCmd,
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

#[derive(Subcommand)]
enum LicenseCmd {
    /// Import a .lic file. Verifies the Ed25519 signature offline and
    /// persists to <data dir>/license.json. Exits non-zero if invalid.
    Import {
        /// Path to the .lic / .json license file.
        file: PathBuf,
    },
    /// Print active license summary: tier, expiry, features, key.
    Status,
    /// List entitled feature keys.
    Features {
        /// Output as JSON array instead of one-per-line.
        #[arg(long)]
        json: bool,
    },
    /// Verify a license file's signature without importing.
    Verify {
        /// Path to a .lic / .json license file.
        file: PathBuf,
    },
    /// Print the active license JSON to stdout (useful for bug reports).
    Export,
    /// Remove the active license. Reverts to Free tier. Requires --force.
    Clear {
        #[arg(long)]
        force: bool,
    },
    /// Call `POST /api/v1/admin/license/reload` on a running pharma-server.
    /// Useful after `pharma license import` to swap the active license
    /// without restarting the service. Requires an admin/owner bearer token.
    Reload {
        /// Base URL of the running server (e.g. http://localhost:8080).
        #[arg(long, default_value = "http://localhost:8080")]
        url: String,
        /// Bearer token of an admin/owner user. Reads from
        /// `PHARMA_ADMIN_TOKEN` if omitted.
        #[arg(long)]
        token: Option<String>,
    },
    /// Fetch a license by id from a remote pharma-license-server, verify
    /// Ed25519 offline, persist it locally, and optionally hot-reload the
    /// running server in one shot.
    Activate {
        /// License id (`lic_*` ULID) emitted by the license-server.
        license_id: String,
        /// Base URL of the license-server.
        #[arg(long, default_value = "https://pharma-license-server.vercel.app")]
        server: String,
        /// If set, POST `/api/v1/admin/license/reload` on the local server
        /// after persisting. Requires --reload-token (or PHARMA_ADMIN_TOKEN).
        #[arg(long)]
        reload_url: Option<String>,
        /// Bearer token for the reload endpoint.
        #[arg(long)]
        reload_token: Option<String>,
    },
}

/// Default license path: sibling of the SurrealKv data dir.
fn license_path() -> anyhow::Result<PathBuf> {
    let cfg = pharma_core::config::AppConfig::load()?;
    let db_path = PathBuf::from(&cfg.db.path);
    let dir = db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    Ok(license::default_license_path(dir))
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
        Cmd::License { cmd } => match cmd {
            LicenseCmd::Import { file } => {
                let bytes =
                    std::fs::read(&file).with_context(|| format!("read {}", file.display()))?;
                let lic = license::parse_and_verify(&bytes)
                    .with_context(|| format!("verify {}", file.display()))?;
                let dest = license_path()?;
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                license::save_to_disk(&lic, &dest)?;
                println!(
                    "license imported: tier={} id={} features={} → {}",
                    lic.tier.as_str(),
                    lic.license_id,
                    lic.features.len(),
                    dest.display()
                );
            }
            LicenseCmd::Status => {
                let path = license_path()?;
                if !path.exists() {
                    println!("No license file at {}.", path.display());
                    println!("Tier: free (default).");
                    return Ok(());
                }
                match license::load_from_disk(&path) {
                    Ok(lic) => {
                        let now = chrono::Utc::now();
                        let grace = chrono::Duration::days(30);
                        let status = if license::is_expired(&lic, now, grace) {
                            "expired"
                        } else if license::is_in_grace(&lic, now, grace) {
                            "grace"
                        } else {
                            "active"
                        };
                        println!("Tier:        {}", lic.tier.as_str());
                        println!("Status:      {status}");
                        println!("License ID:  {}", lic.license_id);
                        println!("Tenant:      {}", lic.tenant_id);
                        match lic.expires_at {
                            Some(e) => println!("Expires:     {e}"),
                            None => println!("Expires:     never (perpetual Free)"),
                        }
                        println!("Seats:       {}", lic.seat_count);
                        println!("Features ({}):", lic.features.len());
                        for f in &lic.features {
                            println!("  - {f}");
                        }
                        println!("Issuer DID:  {}", lic.issuer_did);
                        println!("Key ID:      {}", lic.key_id);
                    }
                    Err(e) => {
                        eprintln!("License file invalid: {e}");
                        eprintln!("Running Free tier as fallback.");
                        std::process::exit(1);
                    }
                }
            }
            LicenseCmd::Features { json } => {
                let path = license_path()?;
                let lic = if path.exists() {
                    license::load_from_disk(&path)?
                } else {
                    license::License::free_default(uuid::Uuid::nil())
                };
                if json {
                    println!("{}", serde_json::to_string_pretty(&lic.features)?);
                } else {
                    for f in &lic.features {
                        println!("{f}");
                    }
                }
            }
            LicenseCmd::Verify { file } => {
                let bytes =
                    std::fs::read(&file).with_context(|| format!("read {}", file.display()))?;
                match license::parse_and_verify(&bytes) {
                    Ok(lic) => {
                        println!(
                            "OK  tier={} id={} key_id={} expires={:?}",
                            lic.tier.as_str(),
                            lic.license_id,
                            lic.key_id,
                            lic.expires_at
                        );
                    }
                    Err(e) => {
                        eprintln!("INVALID: {e}");
                        std::process::exit(1);
                    }
                }
            }
            LicenseCmd::Export => {
                let path = license_path()?;
                if !path.exists() {
                    return Err(anyhow!("no license at {}", path.display()));
                }
                let bytes = std::fs::read(&path)?;
                println!("{}", String::from_utf8_lossy(&bytes));
            }
            LicenseCmd::Reload { url, token } => {
                let token = token
                    .or_else(|| std::env::var("PHARMA_ADMIN_TOKEN").ok())
                    .ok_or_else(|| {
                        anyhow!("no token: pasa --token <T> o exporta PHARMA_ADMIN_TOKEN")
                    })?;
                let endpoint = format!("{}/api/v1/admin/license/reload", url.trim_end_matches('/'));
                let client = reqwest::Client::new();
                let resp = client
                    .post(&endpoint)
                    .bearer_auth(&token)
                    .send()
                    .await
                    .with_context(|| format!("POST {endpoint}"))?;
                let status = resp.status();
                let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                if !status.is_success() {
                    return Err(anyhow!("reload failed: HTTP {} body={}", status, body));
                }
                println!(
                    "reloaded: tier={} status={} features={} key_id={}",
                    body["tier"].as_str().unwrap_or("?"),
                    body["status"].as_str().unwrap_or("?"),
                    body["features"].as_array().map(|a| a.len()).unwrap_or(0),
                    body["key_id"].as_str().unwrap_or("?")
                );
            }
            LicenseCmd::Clear { force } => {
                if !force {
                    return Err(anyhow!("requiere --force para borrar la license activa"));
                }
                let path = license_path()?;
                if path.exists() {
                    std::fs::remove_file(&path)
                        .with_context(|| format!("remove {}", path.display()))?;
                    println!("license removed: {} (tier free vigente)", path.display());
                } else {
                    println!("no license to remove at {}", path.display());
                }
            }
            LicenseCmd::Activate {
                license_id,
                server,
                reload_url,
                reload_token,
            } => {
                let base = server.trim_end_matches('/');
                let url = format!("{base}/api/licenses/{license_id}");
                let client = reqwest::Client::new();
                let resp = client
                    .get(&url)
                    .send()
                    .await
                    .with_context(|| format!("GET {url}"))?;
                let status = resp.status();
                let bytes = resp
                    .bytes()
                    .await
                    .with_context(|| format!("read body from {url}"))?;
                if !status.is_success() {
                    let body = String::from_utf8_lossy(&bytes);
                    return Err(anyhow!(
                        "license fetch failed: HTTP {} body={}",
                        status,
                        body
                    ));
                }
                let lic = license::parse_and_verify(&bytes)
                    .context("license signature verification failed (downloaded bytes)")?;
                if lic.license_id != license_id {
                    return Err(anyhow!(
                        "id mismatch: requested={} but server returned={}",
                        license_id,
                        lic.license_id
                    ));
                }
                let dest = license_path()?;
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                license::save_to_disk(&lic, &dest)?;
                println!(
                    "license activated: tier={} id={} expires={:?} features={} → {}",
                    lic.tier.as_str(),
                    lic.license_id,
                    lic.expires_at,
                    lic.features.len(),
                    dest.display()
                );

                if let Some(reload_url) = reload_url {
                    let token = reload_token
                        .or_else(|| std::env::var("PHARMA_ADMIN_TOKEN").ok())
                        .ok_or_else(|| {
                            anyhow!(
                                "no reload token: pasa --reload-token <T> o exporta PHARMA_ADMIN_TOKEN"
                            )
                        })?;
                    let endpoint = format!(
                        "{}/api/v1/admin/license/reload",
                        reload_url.trim_end_matches('/')
                    );
                    let resp = client
                        .post(&endpoint)
                        .bearer_auth(&token)
                        .send()
                        .await
                        .with_context(|| format!("POST {endpoint}"))?;
                    let status = resp.status();
                    let body: serde_json::Value =
                        resp.json().await.unwrap_or(serde_json::Value::Null);
                    if !status.is_success() {
                        return Err(anyhow!("reload failed: HTTP {} body={}", status, body));
                    }
                    println!(
                        "server reloaded: tier={} status={} features={}",
                        body["tier"].as_str().unwrap_or("?"),
                        body["status"].as_str().unwrap_or("?"),
                        body["features"].as_array().map(|a| a.len()).unwrap_or(0),
                    );
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

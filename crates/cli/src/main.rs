mod backup_cmd;
mod dte_cmd;

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
        /// Comma-separated roles. Valid: cashier|pharmacist|admin|owner.
        /// Default `cashier` (least privilege).
        #[arg(long, default_value = "cashier")]
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
    /// DTE (Documentos Tributarios Electrónicos SII) — listar, ver, exportar, anular, stats.
    Dte {
        #[command(subcommand)]
        cmd: dte_cmd::DteCmd,
    },
    /// CAF (Códigos de Autorización de Folios SII) — import, list, peek next folio.
    Caf {
        #[command(subcommand)]
        cmd: dte_cmd::CafCmd,
    },
    /// Cert digital (.pfx) — import encrypt-at-rest, list, info.
    Cert {
        #[command(subcommand)]
        cmd: dte_cmd::CertCmd,
    },
    /// Backup y restauración de la base de datos local.
    Backup {
        #[command(subcommand)]
        cmd: backup_cmd::BackupCmd,
    },
    /// Webhook ERP→web (ADR-0013 Patrón B) — probar conectividad/firma.
    Webhook {
        #[command(subcommand)]
        cmd: WebhookCmd,
    },
    /// Sembrar data demo (productos + lotes + movimientos) para un tenant.
    /// Mismo servicio que usa el botón "datos demo" de la app. Marca DEMO,
    /// reversible con --force. NO se ejecuta solo.
    SeedDemo {
        /// Slug del tenant a sembrar.
        #[arg(long)]
        tenant: String,
        /// Vertical del pack: pharmacy (default) | minimarket.
        #[arg(long, default_value = "pharmacy")]
        vertical: String,
        /// Si ya hay data demo, regenerarla (wipe + reseed) en vez de fallar.
        #[arg(long)]
        force: bool,
        /// Salida como JSON en vez de texto.
        #[arg(long)]
        json: bool,
    },
    /// Claves de API con scopes para el seam público DSS (ADR-0014 L1).
    /// Endurece `/public/catalog` (scope `catalog:read`) y
    /// `/public/orders/web` (scope `orders:write`). Activa el chequeo con
    /// `public_catalog.require_api_key` / `public_orders.require_api_key`.
    ApiKey {
        #[command(subcommand)]
        cmd: ApiKeyCmd,
    },
}

#[derive(Subcommand)]
enum ApiKeyCmd {
    /// Crear una clave para un tenant. Imprime el secreto UNA sola vez
    /// (solo se guarda su hash SHA-256). Guárdalo: no se puede recuperar.
    Create {
        /// Tenant slug.
        #[arg(long)]
        tenant: String,
        /// Scopes separados por coma. Válidos: catalog:read, orders:write.
        #[arg(long, default_value = "catalog:read")]
        scopes: String,
        /// Etiqueta opcional (ej: "storefront DSS coquimbo").
        #[arg(long)]
        label: Option<String>,
    },
    /// Listar claves de un tenant (id + hash + scopes; nunca el secreto).
    List {
        /// Tenant slug.
        #[arg(long)]
        tenant: String,
        /// Salida como JSON.
        #[arg(long)]
        json: bool,
    },
    /// Revocar (desactivar) una clave por su record id.
    Revoke {
        /// Record id completo, ej: `api_key:xxxxx` (de `pharma api-key list`).
        id: String,
    },
}

#[derive(Subcommand)]
enum WebhookCmd {
    /// Envía un webhook de stock sintético al endpoint configurado para
    /// verificar conectividad + firma HMAC end-to-end (no toca la DB).
    /// Usa `[stock_webhook]` del config salvo override por flags.
    TestStock {
        /// Tenant slug (campo `tenant_slug`/`X-Pharma-Tenant` del payload).
        #[arg(long)]
        tenant: String,
        /// SKU comercial (`external_id`). Default `TEST-SKU`.
        #[arg(long, default_value = "TEST-SKU")]
        sku: String,
        /// Stock final a reportar (`new_stock`). Default 0.
        #[arg(long, default_value_t = 0)]
        stock: i64,
        /// URL destino. Default: `stock_webhook.target_url` del config.
        #[arg(long)]
        url: Option<String>,
        /// HMAC secret. Default: `stock_webhook.hmac_secret` del config
        /// (o env `PHARMA__STOCK_WEBHOOK__HMAC_SECRET`).
        #[arg(long)]
        secret: Option<String>,
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
    /// Inspect the embedded licenser trust store (ADR-0007 key rotation).
    Keys {
        #[command(subcommand)]
        cmd: KeysCmd,
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
    /// Import a signed CRL file (diff `crl-vN.json` or full `snapshot-vN.json`,
    /// ADR-0006). Verifies the Ed25519 signature, applies it to the local
    /// revocation cache (`data/crl_state.json`) and reports if the active
    /// license became revoked. Offline path: the CDN refresh job uses the
    /// same primitives; this lets an operator apply a CRL by hand.
    CrlImport {
        /// Path to the CRL JSON file.
        file: PathBuf,
        /// Treat the file as a full snapshot (replaces local cache) instead
        /// of an incremental diff.
        #[arg(long)]
        snapshot: bool,
    },
    /// Print the local CRL cache: last seen version + revoked license ids.
    CrlStatus,
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

#[derive(Subcommand)]
enum KeysCmd {
    /// List the licenser key_ids embedded in this binary with rotation status
    /// (active emitter / legacy / retired). Offline — no private seed, no
    /// network (ADR-0002, ADR-0007).
    List {
        /// Output as a JSON array instead of a table.
        #[arg(long)]
        json: bool,
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

#[derive(Debug, Serialize, Deserialize)]
struct ApiKeyRow {
    id: surrealdb::sql::Thing,
    key_hash: String,
    scopes: Vec<String>,
    #[serde(default)]
    label: Option<String>,
    active: bool,
}

/// SHA-256 hex of a plaintext API key. MUST stay byte-identical to
/// `api::api_key::hash_key` (the seam verifier) — both are plain SHA-256 hex,
/// cross-pinned by the SHA-256("abc") known-answer test in that module. The CLI
/// does not depend on the `api` crate (avoids pulling the axum graph), so the
/// 3-line hash is replicated here.
fn hash_api_key(plaintext: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(plaintext.as_bytes());
    hex::encode(h.finalize())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = pharma_telemetry::init_cli("pharma-cli");
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
            const VALID_ROLES: &[&str] = &["cashier", "pharmacist", "admin", "owner"];
            if roles_vec.is_empty() {
                return Err(anyhow!(
                    "--roles no puede estar vacío; valores válidos: {VALID_ROLES:?}"
                ));
            }
            for r in &roles_vec {
                if !VALID_ROLES.contains(&r.as_str()) {
                    return Err(anyhow!(
                        "rol inválido '{r}'; valores válidos: {VALID_ROLES:?}"
                    ));
                }
            }
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
                let card = agent::AgentCard::new(&id, name, kind, region, endpoint)?;
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
            LicenseCmd::Keys { cmd } => match cmd {
                KeysCmd::List { json } => {
                    let rows = license::keys::list_keys();
                    if json {
                        let arr: Vec<_> = rows
                            .iter()
                            .map(|r| {
                                serde_json::json!({
                                    "key_id": r.key_id,
                                    "did": r.did,
                                    "accepted": r.accepted,
                                    "legacy": r.legacy,
                                    "active": r.active,
                                })
                            })
                            .collect();
                        println!("{}", serde_json::to_string_pretty(&arr)?);
                    } else {
                        println!(
                            "{:<18} {:<8} {:<7} {:<8} DID",
                            "KEY_ID", "ESTADO", "ACTIVA", "LEGADO"
                        );
                        for r in &rows {
                            let estado = if r.accepted { "vigente" } else { "retirada" };
                            println!(
                                "{:<18} {:<8} {:<7} {:<8} {}",
                                r.key_id,
                                estado,
                                if r.active { "sí" } else { "no" },
                                if r.legacy { "sí" } else { "no" },
                                r.did
                            );
                        }
                    }
                }
            },
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
            LicenseCmd::CrlImport { file, snapshot } => {
                let bytes =
                    std::fs::read(&file).with_context(|| format!("read {}", file.display()))?;
                let lic_path = license_path()?;
                let dir = lic_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."));
                let state_path = license::default_crl_state_path(dir);
                let mut state = license::load_crl_state(&state_path)
                    .with_context(|| format!("crl cache {} ilegible", state_path.display()))?;
                if snapshot {
                    let s = license::parse_and_verify_snapshot(&bytes)
                        .with_context(|| format!("verify snapshot {}", file.display()))?;
                    state.apply_snapshot(&s)?;
                } else {
                    let v = license::parse_and_verify_crl(&bytes)
                        .with_context(|| format!("verify CRL {}", file.display()))?;
                    state.apply_version(&v)?;
                }
                if let Some(parent) = state_path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                license::save_crl_state(&state, &state_path)?;
                println!(
                    "CRL aplicado: version={} revocadas={} → {}",
                    state.last_seen_version,
                    state.revoked.len(),
                    state_path.display()
                );
                if lic_path.exists() {
                    if let Ok(lic) = license::load_from_disk(&lic_path) {
                        if state.is_revoked(&lic.license_id) {
                            println!(
                                "ATENCIÓN: la license activa {} está REVOCADA — el \
                                 server degradará a Free al próximo reload/restart \
                                 (core gratis sigue operativo).",
                                lic.license_id
                            );
                        }
                    }
                }
            }
            LicenseCmd::CrlStatus => {
                let lic_path = license_path()?;
                let dir = lic_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."));
                let state_path = license::default_crl_state_path(dir);
                let state = license::load_crl_state(&state_path)?;
                println!("CRL cache:   {}", state_path.display());
                println!("Version:     {}", state.last_seen_version);
                match state.updated_at {
                    Some(t) => println!("Actualizado: {t}"),
                    None => println!("Actualizado: nunca (sin CRL aplicado)"),
                }
                println!("Revocadas ({}):", state.revoked.len());
                for id in &state.revoked {
                    println!("  - {id}");
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
        Cmd::SeedDemo {
            tenant,
            vertical,
            force,
            json,
        } => {
            let cfg = pharma_core::config::AppConfig::load()?;
            let db_handle = db::connect(&cfg.db).await?;
            let mut tq = db_handle
                .query("SELECT * FROM tenant WHERE slug = $slug LIMIT 1")
                .bind(("slug", tenant.clone()))
                .await
                .context("lookup tenant by slug")?;
            let tenant_row: Option<TenantRow> = tq.take(0)?;
            let tenant_row =
                tenant_row.ok_or_else(|| anyhow!("tenant with slug '{tenant}' not found"))?;
            let summary = domain::seed::seed_demo(&db_handle, &tenant_row.id, &vertical, force)
                .await
                .map_err(|e| anyhow!("seed demo falló: {e}"))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                println!(
                    "seed demo ok: vertical={} productos={} lotes={} movimientos={} \
                     proveedores={} ordenes_compra={} ventas_historicas={} borrados={}",
                    summary.vertical,
                    summary.products_created,
                    summary.batches_created,
                    summary.movements_emitted,
                    summary.suppliers_created,
                    summary.purchase_orders_created,
                    summary.historic_orders_created,
                    summary.wiped
                );
            }
            tracing::info!(
                tenant = %tenant,
                vertical = %summary.vertical,
                products = summary.products_created,
                wiped = summary.wiped,
                "seed demo complete"
            );
        }
        Cmd::ApiKey { cmd } => match cmd {
            ApiKeyCmd::Create {
                tenant,
                scopes,
                label,
            } => {
                const VALID_SCOPES: &[&str] = &["catalog:read", "orders:write"];
                let scopes_vec: Vec<String> = scopes
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if scopes_vec.is_empty() {
                    return Err(anyhow!(
                        "--scopes no puede estar vacío; válidos: {VALID_SCOPES:?}"
                    ));
                }
                for s in &scopes_vec {
                    if !VALID_SCOPES.contains(&s.as_str()) {
                        return Err(anyhow!("scope inválido '{s}'; válidos: {VALID_SCOPES:?}"));
                    }
                }
                let cfg = pharma_core::config::AppConfig::load()?;
                let db_handle = db::connect(&cfg.db).await?;
                let mut tq = db_handle
                    .query("SELECT * FROM tenant WHERE slug = $slug LIMIT 1")
                    .bind(("slug", tenant.clone()))
                    .await
                    .context("lookup tenant by slug")?;
                let tenant_row: Option<TenantRow> = tq.take(0)?;
                let tenant_row =
                    tenant_row.ok_or_else(|| anyhow!("tenant with slug '{tenant}' not found"))?;

                // High-entropy secret (244 bits = two UUIDv4). Shown once.
                let secret = format!(
                    "pk_{}{}",
                    uuid::Uuid::new_v4().simple(),
                    uuid::Uuid::new_v4().simple()
                );
                let hash = hash_api_key(&secret);
                let mut res = db_handle
                    .query(
                        "CREATE api_key SET tenant = $tenant, key_hash = $hash, \
                         scopes = $scopes, label = $label, active = true RETURN AFTER",
                    )
                    .bind(("tenant", tenant_row.id.clone()))
                    .bind(("hash", hash))
                    .bind(("scopes", scopes_vec.clone()))
                    .bind(("label", label.clone()))
                    .await
                    .context("CREATE api_key query")?;
                let row: Option<ApiKeyRow> = res.take(0)?;
                let row = row.ok_or_else(|| anyhow!("api_key create returned no row"))?;
                println!("api key creada: id={} scopes={:?}", row.id, row.scopes);
                println!("SECRETO (guárdalo, no se vuelve a mostrar):");
                println!("{secret}");
                tracing::info!(api_key_id = %row.id, tenant = %tenant, scopes = ?scopes_vec, "api key created");
            }
            ApiKeyCmd::List { tenant, json } => {
                let cfg = pharma_core::config::AppConfig::load()?;
                let db_handle = db::connect(&cfg.db).await?;
                let mut tq = db_handle
                    .query("SELECT * FROM tenant WHERE slug = $slug LIMIT 1")
                    .bind(("slug", tenant.clone()))
                    .await
                    .context("lookup tenant by slug")?;
                let tenant_row: Option<TenantRow> = tq.take(0)?;
                let tenant_row =
                    tenant_row.ok_or_else(|| anyhow!("tenant with slug '{tenant}' not found"))?;
                let mut res = db_handle
                    .query("SELECT * FROM api_key WHERE tenant = $t ORDER BY created_at")
                    .bind(("t", tenant_row.id.clone()))
                    .await
                    .context("SELECT api_key")?;
                let rows: Vec<ApiKeyRow> = res.take(0)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&rows)?);
                } else {
                    println!("{:<28}  {:<7}  {:<24}  SCOPES", "ID", "ACTIVE", "LABEL");
                    for r in &rows {
                        println!(
                            "{:<28}  {:<7}  {:<24}  {}",
                            r.id.to_string(),
                            r.active,
                            r.label.clone().unwrap_or_default(),
                            r.scopes.join(",")
                        );
                    }
                    println!("({} claves)", rows.len());
                }
            }
            ApiKeyCmd::Revoke { id } => {
                let thing = surrealdb::sql::thing(&id)
                    .map_err(|_| anyhow!("id inválido '{id}' (esperado api_key:xxxx)"))?;
                let cfg = pharma_core::config::AppConfig::load()?;
                let db_handle = db::connect(&cfg.db).await?;
                let mut res = db_handle
                    .query("UPDATE $id SET active = false RETURN AFTER")
                    .bind(("id", thing))
                    .await
                    .context("UPDATE api_key")?;
                let row: Option<ApiKeyRow> = res.take(0)?;
                match row {
                    Some(r) => println!("api key revocada: id={} active={}", r.id, r.active),
                    None => return Err(anyhow!("no existe api_key con id '{id}'")),
                }
                tracing::info!(api_key = %id, "api key revoked");
            }
        },
        Cmd::Backup { cmd } => backup_cmd::run(cmd).await?,
        Cmd::Dte { cmd } => dte_cmd::run_dte(cmd).await?,
        Cmd::Caf { cmd } => dte_cmd::run_caf(cmd).await?,
        Cmd::Cert { cmd } => dte_cmd::run_cert(cmd).await?,
        Cmd::Webhook { cmd } => match cmd {
            WebhookCmd::TestStock {
                tenant,
                sku,
                stock,
                url,
                secret,
            } => webhook_test_stock(tenant, sku, stock, url, secret).await?,
        },
    }
    Ok(())
}

/// Build a synthetic ADR-0013 stock payload, HMAC-sign it, and POST it to the
/// configured (or overridden) web endpoint. Surfaces the HTTP status so an
/// operator can confirm the storefront accepts the signature before wiring the
/// live trigger. Pure connectivity probe — reads no tenant data.
async fn webhook_test_stock(
    tenant: String,
    sku: String,
    stock: i64,
    url: Option<String>,
    secret: Option<String>,
) -> anyhow::Result<()> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let cfg = pharma_core::config::AppConfig::load().ok();
    let target = url
        .or_else(|| cfg.as_ref().map(|c| c.stock_webhook.target_url.clone()))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            anyhow!("sin URL destino: pasa --url o configura [stock_webhook] target_url")
        })?;
    let secret = secret
        .or_else(|| cfg.as_ref().map(|c| c.stock_webhook.hmac_secret.clone()))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            anyhow!("sin HMAC secret: pasa --secret o exporta PHARMA__STOCK_WEBHOOK__HMAC_SECRET")
        })?;

    let ts = chrono::Utc::now().to_rfc3339();
    let key = uuid::Uuid::now_v7().to_string();
    let body = serde_json::json!({
        "schema_version": "1.0",
        "tenant_slug": tenant,
        "external_id": sku,
        "new_stock": stock,
        "in_stock": stock > 0,
        "ts": ts,
        "idempotency_key": key,
    });
    let raw = serde_json::to_vec(&body)?;

    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any length");
    mac.update(&raw);
    let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

    println!("POST {target}");
    println!("  X-Pharma-Tenant: {tenant}");
    println!("  Idempotency-Key: {key}");
    let resp = reqwest::Client::new()
        .post(&target)
        .header("content-type", "application/json")
        .header("x-pharma-signature", &signature)
        .header("x-pharma-timestamp", &ts)
        .header("x-pharma-tenant", &tenant)
        .header("idempotency-key", &key)
        .body(raw)
        .send()
        .await
        .with_context(|| format!("POST {target}"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status.is_success() {
        println!("OK: HTTP {status}");
    } else {
        return Err(anyhow!("webhook rechazado: HTTP {status} body={text}"));
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

#[cfg(test)]
mod webhook_tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn test_stock_signs_and_posts_then_reports_2xx() {
        let server = MockServer::start_async().await;
        // The web side verifies HMAC-SHA256 over the raw body; here we just
        // assert the signature header is present + well-formed and the expected
        // headers ride along, then return 200.
        let m = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/api/webhooks/pharma-stock")
                    .header("x-pharma-tenant", "coquimbo-centro")
                    .header_exists("x-pharma-signature")
                    .header_exists("idempotency-key");
                then.status(200).body("ok");
            })
            .await;

        let url = format!("{}/api/webhooks/pharma-stock", server.base_url());
        webhook_test_stock(
            "coquimbo-centro".into(),
            "PARA-500".into(),
            42,
            Some(url),
            Some("shared-secret".into()),
        )
        .await
        .expect("synthetic webhook should succeed");
        m.assert_async().await;
    }

    #[tokio::test]
    async fn test_stock_errors_on_non_2xx() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST);
                then.status(401).body("bad signature");
            })
            .await;
        let err = webhook_test_stock(
            "t".into(),
            "SKU".into(),
            0,
            Some(server.base_url()),
            Some("s".into()),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("401"), "got: {err}");
    }
}

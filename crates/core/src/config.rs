use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub bind: String,
    pub db: DbConfig,
    pub jwt: JwtConfig,
    pub otlp: OtlpConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub backup: BackupConfig,
    /// Serve interactive API docs (Swagger UI at `/swagger-ui` + the OpenAPI
    /// JSON at `/api-docs/openapi.json`). Default `true` so dev/LAN boxes get
    /// docs out of the box; a hardened prod deployment can set
    /// `PHARMA__DOCS__ENABLED=false` to keep the full API surface off the wire.
    /// `#[serde(default = ...)]` keeps existing configs (and env without the
    /// key) deserializing to the default-on behavior.
    #[serde(default = "default_docs_enabled")]
    pub docs: DocsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocsConfig {
    /// Mount Swagger UI + OpenAPI JSON. Default `true`.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for DocsConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn default_true() -> bool {
    true
}

fn default_docs_enabled() -> DocsConfig {
    DocsConfig::default()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Cron expression (`sec min hour day month weekday`, UTC). Empty/None
    /// disables the scheduler. Recommended: `"0 0 3 * * *"` (every day 03:00).
    #[serde(default)]
    pub schedule: Option<String>,
    /// Retention in days. Backups older than this are pruned after each run.
    /// `0` = keep forever.
    #[serde(default)]
    pub retention_days: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsConfig {
    #[serde(default)]
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    pub path: String,
    pub namespace: String,
    pub database: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    pub secret: String,
    pub issuer: String,
    pub ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpConfig {
    pub endpoint: Option<String>,
    pub service_name: String,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let cfg = config::Config::builder()
            .add_source(config::File::with_name("config/default").required(false))
            .add_source(config::File::with_name("config/local").required(false))
            .add_source(config::Environment::with_prefix("PHARMA").separator("__"))
            .build()?;
        Ok(cfg.try_deserialize()?)
    }
}

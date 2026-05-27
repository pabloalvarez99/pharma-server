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
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
    /// Serve interactive API docs (Swagger UI at `/swagger-ui` + the OpenAPI
    /// JSON at `/api-docs/openapi.json`). Default `true` so dev/LAN boxes get
    /// docs out of the box; a hardened prod deployment can set
    /// `PHARMA__DOCS__ENABLED=false` to keep the full API surface off the wire.
    /// `#[serde(default = ...)]` keeps existing configs (and env without the
    /// key) deserializing to the default-on behavior.
    #[serde(default = "default_docs_enabled")]
    pub docs: DocsConfig,
}

/// Per-tenant + per-IP rate-limit settings. Token-bucket (governor crate).
/// All fields default to sane production values; missing config section keeps
/// the limiter enabled with defaults. Set `enabled = false` to disable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_rl_enabled")]
    pub enabled: bool,
    /// Per-tenant sustained rate (requests per minute).
    #[serde(default = "default_tenant_per_min")]
    pub tenant_per_min: u32,
    /// Per-tenant burst capacity (max tokens stored).
    #[serde(default = "default_tenant_burst")]
    pub tenant_burst: u32,
    /// Per-IP sustained rate (requests per minute).
    #[serde(default = "default_ip_per_min")]
    pub ip_per_min: u32,
    /// Per-IP burst capacity (max tokens stored).
    #[serde(default = "default_ip_burst")]
    pub ip_burst: u32,
}

fn default_rl_enabled() -> bool {
    true
}
fn default_tenant_per_min() -> u32 {
    120
}
fn default_tenant_burst() -> u32 {
    30
}
fn default_ip_per_min() -> u32 {
    60
}
fn default_ip_burst() -> u32 {
    20
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: default_rl_enabled(),
            tenant_per_min: default_tenant_per_min(),
            tenant_burst: default_tenant_burst(),
            ip_per_min: default_ip_per_min(),
            ip_burst: default_ip_burst(),
        }
    }
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

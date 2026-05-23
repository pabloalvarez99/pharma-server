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
    pub jobs: JobsConfig,
}

/// Cron job toggles for the `jobs` crate. Every flag defaults true so a fresh
/// install opts in to the full nightly hygiene set without extra config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobsConfig {
    /// Daily 03:00 UTC purge of `idempotency_key` rows whose 24h TTL has
    /// elapsed.
    #[serde(default = "JobsConfig::default_true")]
    pub idempotency_purge_enabled: bool,
    /// Daily 08:00 UTC scan that writes a `notification` row per tenant with
    /// `admin_setting near_expiry_alert_enabled = "true"` and at least one
    /// batch with `expiry_date <= now + 30d AND stock > 0`.
    #[serde(default = "JobsConfig::default_true")]
    pub near_expiry_alert_enabled: bool,
    /// Daily 02:00 UTC tar.gz snapshot of the SurrealKv data dir + `agent.key`
    /// to `<data_dir>/backups/`.
    #[serde(default = "JobsConfig::default_true")]
    pub backup_auto_enabled: bool,
    /// Backups older than this (mtime) are pruned after each auto-run. `0` =
    /// keep forever.
    #[serde(default = "JobsConfig::default_retention")]
    pub backup_retention_days: u32,
}

impl JobsConfig {
    fn default_true() -> bool {
        true
    }
    fn default_retention() -> u32 {
        7
    }
}

impl Default for JobsConfig {
    fn default() -> Self {
        Self {
            idempotency_purge_enabled: true,
            near_expiry_alert_enabled: true,
            backup_auto_enabled: true,
            backup_retention_days: 7,
        }
    }
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

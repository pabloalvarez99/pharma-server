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
    /// ERP→web stock-change push webhook (ADR-0013). Disabled by default; the
    /// common case (no web storefront) carries zero runtime overhead.
    #[serde(default)]
    pub stock_webhook: StockWebhookConfig,
}

/// Configuration for the stock-change webhook (ADR-0013 Patrón B).
///
/// When `enabled` is false or `target_url` is empty the dispatcher is a
/// no-op. The HMAC secret should be injected via env
/// (`PHARMA__STOCK_WEBHOOK__HMAC_SECRET`) and never committed in
/// `config/local.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StockWebhookConfig {
    /// Master switch. Default `false` — opt-in per ADR-0005 invariants.
    #[serde(default)]
    pub enabled: bool,
    /// Absolute URL of the web endpoint, e.g.
    /// `https://tu-farmacia.cl/api/webhooks/pharma-stock`. Empty = no-op.
    #[serde(default)]
    pub target_url: String,
    /// Shared HMAC-SHA256 secret. Empty = no-op (refuse to sign with nothing).
    #[serde(default)]
    pub hmac_secret: String,
    #[serde(default)]
    pub retry: RetryConfig,
}


/// Bounded retry policy for webhook delivery (ADR-0013): 3 retries with
/// backoff `[1, 5, 30]` seconds, then drop + WARN. Never blocks the POS path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Number of retries *after* the first attempt. Default `3`.
    #[serde(default = "default_retry_tries")]
    pub tries: u32,
    /// Seconds to wait before each retry. Default `[1, 5, 30]`.
    #[serde(default = "default_retry_backoff")]
    pub backoff_secs: Vec<u64>,
}

fn default_retry_tries() -> u32 {
    3
}

fn default_retry_backoff() -> Vec<u64> {
    vec![1, 5, 30]
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            tries: default_retry_tries(),
            backoff_secs: default_retry_backoff(),
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

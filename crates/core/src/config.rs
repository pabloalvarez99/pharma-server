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
    #[serde(default)]
    pub public_catalog: PublicCatalogConfig,
    #[serde(default)]
    pub public_orders: PublicOrdersConfig,
    /// ERP→web stock-change push webhook (ADR-0013). Disabled by default; the
    /// common case (no web storefront) carries zero runtime overhead.
    #[serde(default)]
    pub stock_webhook: StockWebhookConfig,
}

/// Public web-push orders endpoint (ADR-0012 pattern 2).
///
/// `POST /api/v1/public/orders/web` receives HMAC-signed orders from the
/// external Tu Farmacia website and writes them into the on-prem ERP with
/// `channel='web'`. The endpoint is unauthenticated — the caller proves
/// identity via the `X-Pharma-Signature` header (HMAC-SHA256 of the raw
/// request body using [`Self::hmac_secret`]).
///
/// **Opt-in by design (ADR-0005 "no surprise public exposure").** When
/// [`Self::enabled`] is `false` (default) OR [`Self::hmac_secret`] is empty,
/// the route returns 404 from inside the handler — identical to the
/// "unknown tenant" response, so a probe cannot tell whether the feature
/// is configured.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicOrdersConfig {
    /// Default `false` — opt-in only.
    #[serde(default = "default_public_orders_enabled")]
    pub enabled: bool,
    /// Shared HMAC-SHA256 secret. Empty string disables the endpoint
    /// (returns 404). Inject via `PHARMA__PUBLIC_ORDERS__HMAC_SECRET` env
    /// var in production; never commit a real value.
    #[serde(default)]
    pub hmac_secret: String,
}

fn default_public_orders_enabled() -> bool {
    false
}

impl Default for PublicOrdersConfig {
    fn default() -> Self {
        Self {
            enabled: default_public_orders_enabled(),
            hmac_secret: String::new(),
        }
    }
}

/// Public read-only catalog endpoint exposure (Fase 12 sync online opt-in).
///
/// **Opt-in by design (ADR-0005 "no surprise public exposure").** When
/// `enabled = false` (default), `GET /api/v1/public/catalog` returns 404 — the
/// route effectively doesn't exist. Only operators who *explicitly* set
/// `enabled = true` accept the trade-off of letting an unauthed peer read a
/// scrubbed subset of their catalog (name, price, in-stock bool, active
/// ingredient — never cost_price, never internal id, never stock count).
///
/// `cors_origins` is a strict allow-list. Requests with `Origin` headers not
/// in the list get no `Access-Control-Allow-Origin` header (browser will
/// reject the cross-origin response). Server-to-server clients (no Origin
/// header) are unaffected — the rate-limit middleware still applies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicCatalogConfig {
    /// Default `false` — opt-in only.
    #[serde(default = "default_public_catalog_enabled")]
    pub enabled: bool,
    /// Allowed browser origins for CORS. Default: official Tu Farmacia website.
    /// Set to `[]` (or any explicit list) to override.
    #[serde(default = "default_public_catalog_cors_origins")]
    pub cors_origins: Vec<String>,
}

fn default_public_catalog_enabled() -> bool {
    false
}

fn default_public_catalog_cors_origins() -> Vec<String> {
    vec!["https://tu-farmacia.cl".to_string()]
}

impl Default for PublicCatalogConfig {
    fn default() -> Self {
        Self {
            enabled: default_public_catalog_enabled(),
            cors_origins: default_public_catalog_cors_origins(),
        }
    }
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

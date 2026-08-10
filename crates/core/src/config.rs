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
    /// CRL (revocation) refresh — opt-in (ADR-0006). Sin `url` ⇒ deshabilitado:
    /// cero red, offline-first (ADR-0005). Cuando hay `url`, un job recorre la
    /// cadena firmada de diffs `crl-v{N}.json` desde la última versión vista y
    /// degrada a Free cualquier license revocada. Best-effort: los errores de
    /// red se loguean y se ignoran (la revocación nunca bloquea el core).
    #[serde(default)]
    pub crl: CrlConfig,
    /// Global CORS allow-list for cross-origin browser clients (the web front on
    /// Vercel / `rutagent.cl`). Default `[]` ⇒ CORS off: only same-origin calls
    /// (the server serving its own `/app`) work — zero exposure. Add the front
    /// origins to let a cross-origin web app call `/api/v1` (ADR-0018).
    #[serde(default)]
    pub cors: CorsConfig,
    /// SaaS provisioning endpoint (`POST /admin/v1/tenants`). Sin key ⇒ la ruta
    /// responde 404 como si no existiera (instalaciones on-prem no la exponen).
    #[serde(default)]
    pub provisioning: ProvisioningConfig,
    /// Respaldo cifrado del usuario en bucket propio (ADR-0022 / ADR-0023).
    /// Default `backend = "none"` ⇒ el nodo valida y no guarda, que es lo
    /// correcto para una instalación on-prem: sus bytes no van a la nube de
    /// nadie salvo que el dueño lo pida.
    #[serde(default)]
    pub user_backup: UserBackupConfig,
}

/// Bucket del respaldo cifrado del usuario (ADR-0023).
///
/// El server sólo ve ciphertext: nada de acá le permite leer un respaldo. Las
/// credenciales del bucket son de la **infraestructura**, no del usuario, y
/// nunca viajan al teléfono — un APK que llevara la key del bucket le daría a
/// cualquiera que lo descompile permiso de escritura sobre los respaldos de
/// todo el mundo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBackupConfig {
    /// `none` (default, valida y no guarda) · `memory` (lab, muere con el
    /// proceso) · `s3` (R2 u otro S3-compatible).
    #[serde(default = "default_user_backup_backend")]
    pub backend: String,
    /// Nombre del bucket.
    #[serde(default)]
    pub bucket: String,
    /// Endpoint S3. R2: `https://<account_id>.r2.cloudflarestorage.com`.
    #[serde(default)]
    pub endpoint: String,
    /// Región de la firma SigV4. R2 usa `auto`.
    #[serde(default = "default_user_backup_region")]
    pub region: String,
    /// Inyectar por `PHARMA__USER_BACKUP__ACCESS_KEY_ID`. Nunca commitear.
    #[serde(default)]
    pub access_key_id: String,
    /// Inyectar por `PHARMA__USER_BACKUP__SECRET_ACCESS_KEY`. Nunca commitear.
    #[serde(default)]
    pub secret_access_key: String,
    /// Prefijo de las claves dentro del bucket.
    #[serde(default = "default_user_backup_prefix")]
    pub prefix: String,
    /// Tope por sobre en bytes (default 4 MiB — ver `domain::user_backup::BackupQuota`).
    #[serde(default = "default_max_envelope_bytes")]
    pub max_envelope_bytes: u64,
    /// Versiones conservadas por tenant; la más vieja sale al entrar la nueva.
    #[serde(default = "default_max_versions")]
    pub max_versions_per_tenant: u32,
    /// Piso de segundos entre subidas del mismo tenant (control de costo: los
    /// PUT son el 96% de la factura, no los bytes).
    #[serde(default = "default_min_seconds_between_uploads")]
    pub min_seconds_between_uploads: u64,
    /// Techo de subidas por tenant en un día UTC. 0 = sin tope.
    ///
    /// El piso de segundos frena la ráfaga pero no la factura: deja pasar 96
    /// subidas diarias, que a 1.000.000 de usuarios son US$ 157.680/año. Este
    /// es el que acota el gasto (ver `domain::user_backup::BackupQuota`).
    #[serde(default = "default_max_uploads_per_day")]
    pub max_uploads_per_day: u32,
    /// Días que sobrevive un sobre sin que nadie lo toque. 0 = para siempre.
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    /// Camino de rescate sin JWT (`POST /api/v1/user-backup/rescue`).
    ///
    /// Default `true`: apagarlo deja a quien perdió el teléfono sin forma de
    /// volver, que es el caso que el respaldo existe para cubrir. Se apaga en
    /// on-prem, donde el respaldo remoto no está en juego.
    #[serde(default = "default_true")]
    pub rescue_enabled: bool,
}

fn default_user_backup_backend() -> String {
    "none".into()
}
fn default_user_backup_region() -> String {
    "auto".into()
}
fn default_user_backup_prefix() -> String {
    "user-backup/".into()
}
fn default_max_envelope_bytes() -> u64 {
    4 * 1024 * 1024
}
fn default_max_versions() -> u32 {
    5
}
fn default_min_seconds_between_uploads() -> u64 {
    15 * 60
}
fn default_max_uploads_per_day() -> u32 {
    6
}
fn default_retention_days() -> u32 {
    400
}

impl Default for UserBackupConfig {
    fn default() -> Self {
        Self {
            backend: default_user_backup_backend(),
            bucket: String::new(),
            endpoint: String::new(),
            region: default_user_backup_region(),
            access_key_id: String::new(),
            secret_access_key: String::new(),
            prefix: default_user_backup_prefix(),
            max_envelope_bytes: default_max_envelope_bytes(),
            max_versions_per_tenant: default_max_versions(),
            min_seconds_between_uploads: default_min_seconds_between_uploads(),
            max_uploads_per_day: default_max_uploads_per_day(),
            retention_days: default_retention_days(),
            rescue_enabled: default_true(),
        }
    }
}

impl UserBackupConfig {
    /// `true` cuando el nodo tiene de verdad dónde guardar. Sin esto la API
    /// contesta `accepted: false` con razón, que es lo honesto.
    pub fn stores_anything(&self) -> bool {
        match self.backend.as_str() {
            "memory" => true,
            "s3" => {
                !self.bucket.trim().is_empty()
                    && !self.endpoint.trim().is_empty()
                    && !self.access_key_id.trim().is_empty()
                    && !self.secret_access_key.trim().is_empty()
            }
            _ => false,
        }
    }

    /// Por qué este nodo no guarda — para el `reason` de la respuesta. `None`
    /// cuando sí guarda.
    pub fn why_not_storing(&self) -> Option<String> {
        match self.backend.as_str() {
            "memory" | "s3" if self.stores_anything() => None,
            "s3" => Some(
                "el bucket está configurado a medias (faltan bucket/endpoint/credenciales)"
                    .into(),
            ),
            "none" => Some(
                "este servidor no guarda respaldos en la nube (backend = none)".into(),
            ),
            otro => Some(format!("backend de respaldo desconocido: {otro:?}")),
        }
    }
}

/// Config del endpoint de aprovisionamiento SaaS. Solo el license-server
/// (signup web) conoce la key; se inyecta vía `PHARMA__PROVISIONING__KEY`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProvisioningConfig {
    /// Shared secret comparado constante-tiempo contra `X-Provisioning-Key`.
    /// `None`/vacío ⇒ endpoint apagado (404).
    #[serde(default)]
    pub key: Option<String>,
}

/// Global CORS settings for cross-origin browser clients (the web front).
/// Empty `allowed_origins` ⇒ disabled (same-origin only). ADR-0018.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CorsConfig {
    /// Exact browser origins allowed to call `/api/v1` cross-origin (scheme +
    /// host [+ port], no trailing slash, e.g. `https://rutagent.cl`). Empty ⇒
    /// CORS disabled. Set via `config/local.toml` `[cors] allowed_origins`.
    #[serde(default)]
    pub allowed_origins: Vec<String>,
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
    /// Require a tenant-scoped API key with the `orders:write` scope, on top of
    /// the HMAC body signature (ADR-0014 L1 "endurecer el seam"). Default
    /// `false` keeps the existing HMAC-only behavior. Fail-closed: when `true`,
    /// a request without a valid scoped key is rejected (401/403), never falls
    /// open. Keys are minted with `pharma api-key create`.
    #[serde(default)]
    pub require_api_key: bool,
}

fn default_public_orders_enabled() -> bool {
    false
}

impl Default for PublicOrdersConfig {
    fn default() -> Self {
        Self {
            enabled: default_public_orders_enabled(),
            hmac_secret: String::new(),
            require_api_key: false,
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
    /// Require a tenant-scoped API key with the `catalog:read` scope (ADR-0014
    /// L1 "endurecer el seam"). Default `false` keeps the existing slug-only
    /// public read. Fail-closed: when `true`, a request without a valid scoped
    /// key for the requested tenant is rejected (401/403). Keys are minted with
    /// `pharma api-key create`.
    #[serde(default)]
    pub require_api_key: bool,
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
            require_api_key: false,
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

fn default_backup_enabled() -> bool {
    true
}
fn default_backup_retention_count() -> u32 {
    14
}
fn default_backup_log_retention() -> u32 {
    90
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Master switch for the nightly snapshot job. `true` (default) runs it on
    /// a sensible default schedule unless `schedule` overrides it; `false`
    /// disables it entirely. Offline-first data safety is a Free-tier pillar
    /// (ADR-0005), so it ships ON.
    #[serde(default = "default_backup_enabled")]
    pub enabled: bool,
    /// Cron expression (`sec min hour day month weekday`, UTC). Empty/None ⇒
    /// the job runs on the default `"0 0 3 * * *"` (every day 03:00) when
    /// `enabled`. Ignored when `enabled = false`.
    #[serde(default)]
    pub schedule: Option<String>,
    /// Age-based retention in days. Backups older than this are pruned after
    /// each run. `0` = disabled (count-based `retention_count` governs).
    #[serde(default)]
    pub retention_days: u32,
    /// Count-based retention: keep the N newest snapshot archives, prune the
    /// rest after each run. `0` = keep all. The primary knob (a fresh install
    /// has no notion of "days"); `retention_days` is an optional extra bound.
    #[serde(default = "default_backup_retention_count")]
    pub retention_count: u32,
    /// Keep the N newest `backup_log` audit rows (independent of the archives
    /// on disk). `0` = keep all.
    #[serde(default = "default_backup_log_retention")]
    pub log_retention: u32,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: default_backup_enabled(),
            schedule: None,
            retention_days: 0,
            retention_count: default_backup_retention_count(),
            log_retention: default_backup_log_retention(),
        }
    }
}

/// CRL refresh (ADR-0006). Opt-in: deshabilitado salvo que se configure `url`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CrlConfig {
    /// Base URL del CDN que sirve los CRL firmados (con o sin slash final),
    /// p.ej. `https://cdn.pharma.example/crl`. El job pide
    /// `{url}/crl-v{N}.json`. Vacío/None ⇒ refresh deshabilitado.
    #[serde(default)]
    pub url: Option<String>,
    /// Cron (`sec min hour day month weekday`, UTC). None/empty ⇒ cada 6 horas
    /// (`0 0 */6 * * *`). La revocación es best-effort; no necesita ser densa.
    #[serde(default)]
    pub schedule: Option<String>,
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

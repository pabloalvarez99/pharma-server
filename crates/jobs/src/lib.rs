//! Cron jobs hub for `pharma-server`.
//!
//! Hosts every scheduled background job inside a single
//! [`tokio_cron_scheduler::JobScheduler`] so the process owns one timer
//! thread, one set of metrics, and one graceful-shutdown handle:
//!
//! | Job                  | Cron (UTC)       | Toggle (`JobsConfig`)             |
//! |----------------------|------------------|-----------------------------------|
//! | `backup-auto`        | `0 0 2 * * *`    | `backup_auto_enabled`             |
//! | `idempotency-purge`  | `0 0 3 * * *`    | `idempotency_purge_enabled`       |
//! | `near-expiry-alert`  | `0 0 8 * * *`    | `near_expiry_alert_enabled`       |
//!
//! Cron format is `tokio-cron-scheduler` 7-field `sec min hour dom mon dow [year]`
//! (UTC), so `0 0 2 * * *` = "second 0 of minute 0 of hour 2, every day". The
//! API surface is intentionally tiny:
//!
//! ```ignore
//! let scheduler = jobs::Scheduler::start(db, data_dir, opts).await?;
//! // … axum::serve runs the http server …
//! scheduler.shutdown().await?;
//! ```
//!
//! ## Idempotency purge
//! Delegates to [`domain::sales::service::purge_expired_idempotency`]: drops
//! every `idempotency_key` row whose `expires_at` (= insert time + 24h) is in
//! the past. Tenant-wide; the cron is a process-level housekeeper.
//!
//! ## Near-expiry alert
//! For every tenant where `admin_setting key='near_expiry_alert_enabled'` and
//! `value='true'`, lists `product_batch` rows with `active=true`, `stock>0`
//! and `expiry_date <= now + 30d`, and emits one `notification` row of
//! `kind='near_expiry'` with the batch summary as `payload`. Default is OFF
//! (no setting present ⇒ no notification ⇒ no rows written), so existing
//! installs aren't spammed.
//!
//! ## Backup auto
//! Same on-disk format as `POST /api/v1/admin/backup`: tar.gz of the
//! SurrealKv data dir + sibling `agent.key`. Filename pattern is
//! `auto-<YYYY-MM-DD>.snapshot` under `<data_dir>/backups/` so the cron
//! artifacts are easy to spot next to on-demand `pharma-backup-*.tar.gz`. The
//! retention prune drops auto-snapshots older than `backup_retention_days`
//! (default 7); it never touches on-demand backups.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use surrealdb::engine::local::Db as LocalDb;
use surrealdb::sql::Thing;
use surrealdb::Surreal;
use thiserror::Error;
use tokio_cron_scheduler::{Job, JobScheduler};

/// Re-export for callers that want to pass the same handle the rest of the
/// app uses (`Arc<db::Db>`).
pub type Db = Surreal<LocalDb>;

/// Cron expressions (UTC). `sec min hour dom mon dow`.
const CRON_BACKUP: &str = "0 0 2 * * *";
const CRON_IDEMPOTENCY_PURGE: &str = "0 0 3 * * *";
const CRON_NEAR_EXPIRY: &str = "0 0 8 * * *";

/// Near-expiry horizon. Anything expiring inside this window with `stock > 0`
/// is reported.
const NEAR_EXPIRY_WINDOW_DAYS: i64 = 30;

/// `admin_setting` key the per-tenant near-expiry alert checks. Value must be
/// the literal string `"true"` (case-sensitive) — the same convention used by
/// `federation_enabled`.
const NEAR_EXPIRY_SETTING_KEY: &str = "near_expiry_alert_enabled";

/// Errors surfaced by [`Scheduler::start`] / [`Scheduler::shutdown`]. Job
/// bodies log + swallow per-tick errors so a single bad day doesn't kill the
/// schedule.
#[derive(Debug, Error)]
pub enum JobError {
    #[error("scheduler init failed: {0}")]
    Init(#[from] tokio_cron_scheduler::JobSchedulerError),
}

/// Public knobs for [`Scheduler::start`]. Construct via [`Default`] or
/// [`SchedulerOpts::from_config`].
#[derive(Debug, Clone)]
pub struct SchedulerOpts {
    /// Enable the daily 03:00 UTC `idempotency_key` purge.
    pub idempotency_purge_enabled: bool,
    /// Enable the daily 08:00 UTC near-expiry alert sweep.
    pub near_expiry_alert_enabled: bool,
    /// Enable the daily 02:00 UTC backup snapshot.
    pub backup_auto_enabled: bool,
    /// Auto-backup retention in days; `0` keeps forever.
    pub backup_retention_days: u32,
}

impl Default for SchedulerOpts {
    fn default() -> Self {
        Self {
            idempotency_purge_enabled: true,
            near_expiry_alert_enabled: true,
            backup_auto_enabled: true,
            backup_retention_days: 7,
        }
    }
}

impl SchedulerOpts {
    /// Map [`pharma_core::config::JobsConfig`] one-to-one. The two structs are
    /// kept independent so the `jobs` crate doesn't leak into every
    /// downstream's `Cargo.lock`.
    pub fn from_config(cfg: &pharma_core::config::JobsConfig) -> Self {
        Self {
            idempotency_purge_enabled: cfg.idempotency_purge_enabled,
            near_expiry_alert_enabled: cfg.near_expiry_alert_enabled,
            backup_auto_enabled: cfg.backup_auto_enabled,
            backup_retention_days: cfg.backup_retention_days,
        }
    }
}

/// Owned wrapper around the started [`JobScheduler`]. Drop semantics: if the
/// scheduler is dropped without calling [`Scheduler::shutdown`], in-flight
/// jobs are abandoned and tokio_cron_scheduler emits a `Drop` log — always
/// prefer the explicit shutdown.
pub struct Scheduler {
    sched: JobScheduler,
    /// Held so the `Arc` keeps `db` alive for the closures' lifetime; jobs
    /// clone their own `Arc` internally.
    _db: Option<Arc<Db>>,
    _data_dir: Option<PathBuf>,
}

impl Scheduler {
    /// Build + start every enabled job. `db = None` skips the DB-bound jobs
    /// (idempotency purge + near-expiry); `data_dir = None` skips the backup
    /// job. This mirrors the existing fallback in `api::run` — a degraded
    /// `/health/ready` still has a live (but empty) scheduler.
    pub async fn start(
        db: Option<Arc<Db>>,
        data_dir: Option<PathBuf>,
        opts: SchedulerOpts,
    ) -> Result<Self, JobError> {
        let sched = JobScheduler::new().await?;

        if opts.backup_auto_enabled {
            if let Some(dir) = data_dir.clone() {
                let job = backup_auto_job(dir, opts.backup_retention_days)?;
                sched.add(job).await?;
                tracing::info!(
                    cron = CRON_BACKUP,
                    retention_days = opts.backup_retention_days,
                    "backup-auto job registered"
                );
            } else {
                tracing::warn!("backup-auto enabled but data_dir is None; skipping");
            }
        }

        if opts.idempotency_purge_enabled {
            if let Some(db) = db.clone() {
                let job = idempotency_purge_job(db)?;
                sched.add(job).await?;
                tracing::info!(
                    cron = CRON_IDEMPOTENCY_PURGE,
                    "idempotency-purge job registered"
                );
            } else {
                tracing::warn!("idempotency-purge enabled but db is None; skipping");
            }
        }

        if opts.near_expiry_alert_enabled {
            if let Some(db) = db.clone() {
                let job = near_expiry_job(db)?;
                sched.add(job).await?;
                tracing::info!(cron = CRON_NEAR_EXPIRY, "near-expiry-alert job registered");
            } else {
                tracing::warn!("near-expiry-alert enabled but db is None; skipping");
            }
        }

        sched.start().await?;
        tracing::info!("scheduler started");

        Ok(Self {
            sched,
            _db: db,
            _data_dir: data_dir,
        })
    }

    /// Graceful shutdown. Stops the timer, drains in-flight jobs, and clears
    /// the scheduler's internal handles. Safe to call from any task; takes
    /// `self` so the caller can't accidentally reuse a stopped scheduler.
    pub async fn shutdown(mut self) -> Result<(), JobError> {
        self.sched.shutdown().await?;
        tracing::info!("scheduler shut down");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// idempotency purge
// ---------------------------------------------------------------------------

fn idempotency_purge_job(db: Arc<Db>) -> Result<Job, JobError> {
    let job = Job::new_async(CRON_IDEMPOTENCY_PURGE, move |_uuid, _l| {
        let db = db.clone();
        Box::pin(async move {
            run_idempotency_purge(db.as_ref()).await;
        })
    })?;
    Ok(job)
}

/// Direct entry-point for tests + the cron closure. Always logs the outcome
/// at INFO (success / no-op) or ERROR (failure), never propagates.
pub async fn run_idempotency_purge(db: &Db) {
    match domain::sales::service::purge_expired_idempotency(db).await {
        Ok(0) => tracing::debug!("idempotency-purge: no expired keys"),
        Ok(n) => tracing::info!(removed = n, "idempotency-purge completed"),
        Err(e) => tracing::error!(error = %e, "idempotency-purge failed"),
    }
}

// ---------------------------------------------------------------------------
// near-expiry alert
// ---------------------------------------------------------------------------

fn near_expiry_job(db: Arc<Db>) -> Result<Job, JobError> {
    let job = Job::new_async(CRON_NEAR_EXPIRY, move |_uuid, _l| {
        let db = db.clone();
        Box::pin(async move {
            run_near_expiry_alert(db.as_ref()).await;
        })
    })?;
    Ok(job)
}

/// Entry-point for tests + the cron closure. Walks every tenant with the
/// `near_expiry_alert_enabled` setting flipped on, finds the urgent batches,
/// and writes a single `notification` row summarising them.
///
/// Each tenant is processed in isolation: a query failure on tenant `A`
/// doesn't block tenant `B`.
pub async fn run_near_expiry_alert(db: &Db) {
    let tenants = match enabled_tenants(db).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "near-expiry: failed to enumerate enabled tenants");
            return;
        }
    };
    if tenants.is_empty() {
        tracing::debug!("near-expiry: no tenants opted in");
        return;
    }

    let mut total_notifications = 0u64;
    let mut total_batches = 0u64;
    for tenant in &tenants {
        match scan_and_notify_tenant(db, tenant).await {
            Ok(0) => tracing::debug!(tenant = %tenant, "near-expiry: nothing urgent"),
            Ok(n) => {
                total_notifications += 1;
                total_batches += n;
                tracing::info!(
                    tenant = %tenant,
                    batches = n,
                    "near-expiry: notification emitted"
                );
            }
            Err(e) => {
                tracing::error!(tenant = %tenant, error = %e, "near-expiry: tenant scan failed");
            }
        }
    }
    if total_notifications > 0 {
        tracing::info!(
            tenants_alerted = total_notifications,
            total_batches,
            "near-expiry-alert sweep done"
        );
    }
}

#[derive(Debug, Deserialize)]
struct TenantRow {
    tenant: Thing,
}

async fn enabled_tenants(db: &Db) -> Result<Vec<Thing>, surrealdb::Error> {
    let mut r = db
        .query(
            "SELECT tenant FROM admin_setting \
             WHERE key = $k AND value = 'true'",
        )
        .bind(("k", NEAR_EXPIRY_SETTING_KEY))
        .await?
        .check()?;
    let rows: Vec<TenantRow> = r.take(0)?;
    Ok(rows.into_iter().map(|r| r.tenant).collect())
}

#[derive(Debug, Deserialize)]
struct BatchRow {
    id: Thing,
    product: Thing,
    batch_code: String,
    expiry_date: surrealdb::sql::Datetime,
    stock: i64,
}

/// Per-batch summary embedded in the notification payload. Round-trips through
/// SurrealDB's `payload object` field which is declared `FLEXIBLE` in
/// `0017_notification.surql` — without `FLEXIBLE`, a SCHEMAFULL `object` field
/// silently strips any sub-key without an explicit `DEFINE FIELD`, leaving the
/// stored value as `{}`.
#[derive(Debug, Serialize)]
struct BatchSummary {
    batch_id: String,
    product: String,
    batch_code: String,
    expiry_date: String,
    days_to_expiry: i64,
    stock: i64,
}

/// Body of a `near_expiry` notification's `payload`.
#[derive(Debug, Serialize)]
struct NearExpiryPayload {
    window_days: i64,
    generated_at: String,
    count: usize,
    batches: Vec<BatchSummary>,
}

/// Scan one tenant and, if any urgent batches exist, append exactly one
/// `notification` row. Returns the batch count (0 = nothing to do, no
/// notification written).
async fn scan_and_notify_tenant(db: &Db, tenant: &Thing) -> Result<u64, surrealdb::Error> {
    let window = surrealdb::sql::Duration::from(std::time::Duration::from_secs(
        NEAR_EXPIRY_WINDOW_DAYS as u64 * 86_400,
    ));
    let mut r = db
        .query(
            "SELECT id, product, batch_code, expiry_date, stock FROM product_batch \
             WHERE tenant = $t \
               AND active = true \
               AND stock > 0 \
               AND expiry_date <= time::now() + $w \
             ORDER BY expiry_date ASC",
        )
        .bind(("t", tenant.clone()))
        .bind(("w", window))
        .await?
        .check()?;
    let batches: Vec<BatchRow> = r.take(0)?;
    if batches.is_empty() {
        return Ok(0);
    }

    let now = chrono::Utc::now();
    let summaries: Vec<BatchSummary> = batches
        .iter()
        .map(|b| {
            let dt: chrono::DateTime<chrono::Utc> = b.expiry_date.0;
            BatchSummary {
                batch_id: b.id.to_string(),
                product: b.product.to_string(),
                batch_code: b.batch_code.clone(),
                expiry_date: dt.to_rfc3339(),
                days_to_expiry: (dt - now).num_days(),
                stock: b.stock,
            }
        })
        .collect();
    let payload = NearExpiryPayload {
        window_days: NEAR_EXPIRY_WINDOW_DAYS,
        generated_at: now.to_rfc3339(),
        count: summaries.len(),
        batches: summaries,
    };

    db.query(
        "CREATE notification SET \
            tenant = $t, \
            kind = 'near_expiry', \
            payload = $p, \
            seen = false",
    )
    .bind(("t", tenant.clone()))
    .bind(("p", payload))
    .await?
    .check()?;

    Ok(batches.len() as u64)
}

// ---------------------------------------------------------------------------
// backup auto
// ---------------------------------------------------------------------------

fn backup_auto_job(data_dir: PathBuf, retention_days: u32) -> Result<Job, JobError> {
    let job = Job::new_async(CRON_BACKUP, move |_uuid, _l| {
        let dir = data_dir.clone();
        Box::pin(async move {
            // tar+gz the data dir on a blocking thread so we don't stall the
            // tokio executor while writing potentially-large snapshots.
            let dir_for_run = dir.clone();
            let result = tokio::task::spawn_blocking(move || run_backup_auto(&dir_for_run)).await;
            match result {
                Ok(Ok(rep)) => tracing::info!(
                    path = %rep.path.display(),
                    bytes = rep.bytes,
                    duration_ms = rep.duration_ms,
                    "backup-auto completed"
                ),
                Ok(Err(e)) => tracing::error!(error = %e, "backup-auto failed"),
                Err(e) => tracing::error!(error = %e, "backup-auto task panicked"),
            }

            if retention_days > 0 {
                let dir_for_prune = dir.clone();
                match tokio::task::spawn_blocking(move || {
                    prune_auto_backups(&dir_for_prune, retention_days)
                })
                .await
                {
                    Ok(Ok(0)) => {}
                    Ok(Ok(n)) => tracing::info!(removed = n, "backup-auto: pruned old snapshots"),
                    Ok(Err(e)) => {
                        tracing::error!(error = %e, "backup-auto: prune failed");
                    }
                    Err(e) => tracing::error!(error = %e, "backup-auto: prune task panicked"),
                }
            }
        })
    })?;
    Ok(job)
}

/// Report returned by [`run_backup_auto`].
#[derive(Debug)]
pub struct BackupReport {
    pub path: PathBuf,
    pub bytes: u64,
    pub duration_ms: u128,
}

/// Tar.gz the SurrealKv data dir + sibling `agent.key` to
/// `<data_dir>/backups/auto-<YYYY-MM-DD>.snapshot`. Two ticks on the same UTC
/// day overwrite the snapshot — by design: nightly cadence, "1 file per day".
///
/// `db_path` is the SurrealKv subdir (same value as `AppState.data_dir`); its
/// parent owns the `backups/` directory and the `agent.key` file. Returns the
/// absolute path + size + wallclock duration.
pub fn run_backup_auto(db_path: &Path) -> anyhow::Result<BackupReport> {
    use std::io::Write;
    let started = std::time::Instant::now();
    let data_dir = db_path.parent().unwrap_or_else(|| Path::new("."));
    let backups_dir = data_dir.join("backups");
    std::fs::create_dir_all(&backups_dir)?;
    let ts = chrono::Utc::now().format("%Y-%m-%d");
    let out_path = backups_dir.join(format!("auto-{ts}.snapshot"));

    let file = std::fs::File::create(&out_path)?;
    let gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);

    if db_path.exists() {
        tar.append_dir_all("surreal", db_path)?;
    }
    let key_path = data_dir.join("agent.key");
    if key_path.exists() {
        let mut f = std::fs::File::open(&key_path)?;
        tar.append_file("agent.key", &mut f)?;
    }
    let gz = tar.into_inner()?;
    gz.finish()?.flush()?;

    let bytes = std::fs::metadata(&out_path)?.len();
    Ok(BackupReport {
        path: out_path,
        bytes,
        duration_ms: started.elapsed().as_millis(),
    })
}

/// Delete `<data_dir>/backups/auto-*.snapshot` files older than the latest
/// `retention_days` (by mtime). Returns how many files were removed. Only
/// touches files matching the `auto-*.snapshot` naming, so on-demand
/// `pharma-backup-*.tar.gz` artefacts created by the admin endpoint are left
/// alone.
pub fn prune_auto_backups(db_path: &Path, retention_days: u32) -> std::io::Result<usize> {
    if retention_days == 0 {
        return Ok(0);
    }
    let data_dir = db_path.parent().unwrap_or_else(|| Path::new("."));
    let backups_dir = data_dir.join("backups");
    if !backups_dir.exists() {
        return Ok(0);
    }
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(retention_days as u64 * 86_400);
    let mut removed = 0usize;
    for entry in std::fs::read_dir(&backups_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("auto-") || !name.ends_with(".snapshot") {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(mtime) = meta.modified() {
                if mtime < cutoff && std::fs::remove_file(entry.path()).is_ok() {
                    removed += 1;
                }
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opts_default_matches_jobs_config_default() {
        let opts = SchedulerOpts::default();
        let from_cfg = SchedulerOpts::from_config(&pharma_core::config::JobsConfig::default());
        assert_eq!(
            opts.idempotency_purge_enabled,
            from_cfg.idempotency_purge_enabled
        );
        assert_eq!(
            opts.near_expiry_alert_enabled,
            from_cfg.near_expiry_alert_enabled
        );
        assert_eq!(opts.backup_auto_enabled, from_cfg.backup_auto_enabled);
        assert_eq!(opts.backup_retention_days, from_cfg.backup_retention_days);
        assert!(opts.idempotency_purge_enabled);
        assert_eq!(opts.backup_retention_days, 7);
    }

    #[tokio::test]
    async fn start_with_all_disabled_still_starts_and_shuts_down() {
        let opts = SchedulerOpts {
            idempotency_purge_enabled: false,
            near_expiry_alert_enabled: false,
            backup_auto_enabled: false,
            backup_retention_days: 0,
        };
        let s = Scheduler::start(None, None, opts).await.expect("start");
        s.shutdown().await.expect("shutdown");
    }

    #[test]
    fn prune_auto_backups_noop_when_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("surreal");
        let removed = prune_auto_backups(&db_path, 7).unwrap();
        assert_eq!(removed, 0);
    }

    #[test]
    fn prune_auto_backups_zero_retention_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("surreal");
        std::fs::create_dir_all(&db_path).unwrap();
        let backups = tmp.path().join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        std::fs::write(backups.join("auto-2026-01-01.snapshot"), b"x").unwrap();
        let removed = prune_auto_backups(&db_path, 0).unwrap();
        assert_eq!(removed, 0);
    }
}

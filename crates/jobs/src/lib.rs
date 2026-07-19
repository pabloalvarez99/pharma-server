//! `jobs` — scheduled / cron-driven background work for pharma-server.
//!
//! Currently hosts the nightly **near-expiry alert scan** (see
//! [`near_expiry`]). The scheduler helpers here are intentionally decoupled
//! from how the Windows service boots: callers add the returned [`Job`] to an
//! existing [`JobScheduler`] (see `crates/api/src/lib.rs` for the backup /
//! idempotency-purge jobs that follow the same shape).

use std::sync::Arc;

use surrealdb::{Connection, Surreal};
use tokio_cron_scheduler::{Job, JobScheduler};

pub mod backup;
pub mod near_expiry;

pub use backup::{inspect_snapshot, retain_recent, SnapshotInspection};
pub use near_expiry::{run_near_expiry_scan, NearExpiryAlert};

/// Errors surfaced by background jobs.
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    /// A SurrealDB query or connection failure. Boxed because `surrealdb::Error`
    /// is ~1KB; keeping `Result<_, JobError>` cheap to return triggers clippy's
    /// `result_large_err` otherwise.
    #[error("database error: {0}")]
    Db(Box<surrealdb::Error>),
    /// Failed to build or register a cron job with the scheduler.
    #[error("scheduler error: {0}")]
    Schedule(String),
}

impl From<surrealdb::Error> for JobError {
    fn from(e: surrealdb::Error) -> Self {
        JobError::Db(Box::new(e))
    }
}

/// Default cron for the near-expiry scan: `0 0 6 * * *` = 06:00:00 daily (UTC).
/// Six fields (sec min hour day-of-month month day-of-week), matching the
/// `tokio-cron-scheduler` format used by the existing backup/purge jobs.
pub const NEAR_EXPIRY_DEFAULT_CRON: &str = "0 0 6 * * *";

/// Default cron for the nightly backup snapshot: `0 0 3 * * *` = 03:00:00 daily
/// (UTC). Used when `backup.enabled` is set but `backup.schedule` is empty —
/// the operator gets a sensible nightly snapshot without configuring cron.
pub const BACKUP_DEFAULT_CRON: &str = "0 0 3 * * *";

/// Build an empty [`JobScheduler`]. Thin wrapper retained for callers that
/// only need a scheduler handle.
pub async fn scheduler() -> anyhow::Result<JobScheduler> {
    let s = JobScheduler::new().await?;
    Ok(s)
}

/// Build the nightly near-expiry alert [`Job`].
///
/// Mirrors the backup / idempotency-purge registration in
/// `crates/api/src/lib.rs`: returns a `Job::new_async` that the caller adds to
/// a running [`JobScheduler`] — this fn does **not** start the scheduler or
/// touch service boot. On each tick it runs [`run_near_expiry_scan`] over
/// `within_days` and logs the per-tenant WARN digest; query failures are logged
/// (not propagated) so one bad night can't kill the scheduler thread.
///
/// `schedule` is a 6-field cron string (e.g. [`NEAR_EXPIRY_DEFAULT_CRON`]).
/// Generic over the connection so the same wiring works for the live
/// `Surreal<kv-surrealkv>` handle and the in-memory test engine.
pub fn schedule_near_expiry<C>(
    db: Arc<Surreal<C>>,
    schedule: &str,
    within_days: i64,
) -> Result<Job, JobError>
where
    C: Connection,
{
    Job::new_async(schedule, move |_uuid, _l| {
        let db = db.clone();
        Box::pin(async move {
            match run_near_expiry_scan(db.as_ref(), within_days).await {
                Ok(alerts) if alerts.is_empty() => {
                    tracing::debug!(within_days, "near-expiry scan: no lots near expiry")
                }
                // Per-tenant WARN digests are already emitted inside the scan;
                // this is just the run-level summary.
                Ok(alerts) => tracing::info!(
                    total = alerts.len(),
                    within_days,
                    "near-expiry scan completed"
                ),
                Err(e) => tracing::error!(error = %e, "near-expiry scan failed"),
            }
        })
    })
    .map_err(|e| JobError::Schedule(e.to_string()))
}

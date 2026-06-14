//! Install-wide audit log of backup snapshots (`backup_log` table, migration
//! 0028). NOT tenant-scoped — a snapshot covers the whole SurrealKv store; see
//! the migration header for the rationale.
//!
//! Any backup caller records one row here so an operator can audit *what* was
//! backed up and *when* — independently of whether the `.tar.gz` artifacts
//! still exist on disk:
//!   * the scheduled job (`crates/api` scheduler hub),
//!   * `POST /api/v1/admin/backup` (manual),
//!   * `pharma backup create` (cli).
//!
//! The functions are generic over the connection (same shape as
//! `jobs::schedule_near_expiry`) so they work against both the live
//! `Surreal<kv-surrealkv>` handle and the in-memory test engine.

use serde::{Deserialize, Serialize};
use surrealdb::{Connection, Surreal};

/// Errors surfaced by the backup-log persistence layer. `surrealdb::Error` is
/// boxed (it is ~1KB) so `Result<_, BackupLogError>` stays cheap to return —
/// otherwise clippy's `result_large_err` fires.
#[derive(Debug, thiserror::Error)]
pub enum BackupLogError {
    #[error("database error: {0}")]
    Db(Box<surrealdb::Error>),
}

impl From<surrealdb::Error> for BackupLogError {
    fn from(e: surrealdb::Error) -> Self {
        BackupLogError::Db(Box::new(e))
    }
}

/// Outcome of a backup attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupStatus {
    Ok,
    Failed,
}

impl BackupStatus {
    fn as_str(self) -> &'static str {
        match self {
            BackupStatus::Ok => "ok",
            BackupStatus::Failed => "failed",
        }
    }
}

/// What triggered the backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupSource {
    /// Cron-driven nightly job.
    Scheduled,
    /// `POST /api/v1/admin/backup`.
    Manual,
    /// `pharma backup create`.
    Cli,
}

impl BackupSource {
    fn as_str(self) -> &'static str {
        match self {
            BackupSource::Scheduled => "scheduled",
            BackupSource::Manual => "manual",
            BackupSource::Cli => "cli",
        }
    }
}

/// A backup-log row to insert. Artifact fields (`path`/`bytes`/`sha256`/
/// `duration_ms`) are populated on success and left `None` on failure, where
/// `error` carries the reason instead.
#[derive(Debug, Clone)]
pub struct NewBackupLog {
    pub status: BackupStatus,
    pub source: BackupSource,
    pub path: Option<String>,
    pub bytes: Option<u64>,
    pub sha256: Option<String>,
    pub duration_ms: Option<u128>,
    pub error: Option<String>,
}

impl NewBackupLog {
    /// A successful backup: artifact metadata, no error.
    pub fn ok(
        source: BackupSource,
        path: impl Into<String>,
        bytes: u64,
        sha256: impl Into<String>,
        duration_ms: u128,
    ) -> Self {
        Self {
            status: BackupStatus::Ok,
            source,
            path: Some(path.into()),
            bytes: Some(bytes),
            sha256: Some(sha256.into()),
            duration_ms: Some(duration_ms),
            error: None,
        }
    }

    /// A failed backup: just the reason.
    pub fn failed(source: BackupSource, error: impl Into<String>) -> Self {
        Self {
            status: BackupStatus::Failed,
            source,
            path: None,
            bytes: None,
            sha256: None,
            duration_ms: None,
            error: Some(error.into()),
        }
    }
}

/// A persisted `backup_log` row, read back newest-first by [`list`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupLogEntry {
    pub status: String,
    pub source: String,
    pub path: Option<String>,
    pub bytes: Option<i64>,
    pub sha256: Option<String>,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
    pub started_at: surrealdb::sql::Datetime,
}

#[derive(Debug, Deserialize)]
struct IdRow {
    id: surrealdb::sql::Thing,
}

/// Insert one audit row for a backup attempt. SurrealDB `int` is `i64`; `bytes`
/// (`u64`) and `duration_ms` (`u128`) are cast saturatingly so an absurd value
/// can never panic the insert.
pub async fn record<C: Connection>(
    db: &Surreal<C>,
    entry: NewBackupLog,
) -> Result<(), BackupLogError> {
    let bytes = entry.bytes.map(|b| i64::try_from(b).unwrap_or(i64::MAX));
    let duration_ms = entry
        .duration_ms
        .map(|d| i64::try_from(d).unwrap_or(i64::MAX));
    db.query(
        "CREATE backup_log SET status=$status, source=$source, path=$path, \
         bytes=$bytes, sha256=$sha256, duration_ms=$duration_ms, error=$error",
    )
    .bind(("status", entry.status.as_str()))
    .bind(("source", entry.source.as_str()))
    .bind(("path", entry.path))
    .bind(("bytes", bytes))
    .bind(("sha256", entry.sha256))
    .bind(("duration_ms", duration_ms))
    .bind(("error", entry.error))
    .await?
    .check()?;
    Ok(())
}

/// The `limit` most recent rows, newest first.
pub async fn list<C: Connection>(
    db: &Surreal<C>,
    limit: usize,
) -> Result<Vec<BackupLogEntry>, BackupLogError> {
    let mut res = db
        .query("SELECT * FROM backup_log ORDER BY started_at DESC LIMIT $limit")
        .bind(("limit", limit as i64))
        .await?;
    let rows: Vec<BackupLogEntry> = res.take(0)?;
    Ok(rows)
}

/// Prune the audit log, keeping the `keep` newest rows and deleting the rest.
/// Returns the number of rows removed. `keep == 0` is a no-op (keep all) —
/// mirrors `prune_backups(retention_days == 0)` so a zero default can never
/// nuke the whole audit trail. Deletes rows, never the `.tar.gz` files.
pub async fn prune_log<C: Connection>(
    db: &Surreal<C>,
    keep: usize,
) -> Result<usize, BackupLogError> {
    if keep == 0 {
        return Ok(0);
    }
    let mut res = db
        .query("SELECT id, started_at FROM backup_log ORDER BY started_at DESC START $keep")
        .bind(("keep", keep as i64))
        .await?;
    let stale: Vec<IdRow> = res.take(0)?;
    if stale.is_empty() {
        return Ok(0);
    }
    let ids: Vec<surrealdb::sql::Thing> = stale.into_iter().map(|r| r.id).collect();
    let removed = ids.len();
    db.query("DELETE backup_log WHERE id IN $ids")
        .bind(("ids", ids))
        .await?
        .check()?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use surrealdb::engine::local::Mem;

    /// Fresh in-memory DB with the 0028 schema applied (also asserts the
    /// migration SQL itself parses + runs).
    async fn mem_db() -> Surreal<surrealdb::engine::local::Db> {
        let db = Surreal::new::<Mem>(()).await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db.query(include_str!("../../../migrations/0028_backup_log.surql"))
            .await
            .unwrap()
            .check()
            .unwrap();
        db
    }

    #[tokio::test]
    async fn record_ok_then_list_reads_it_back() {
        let db = mem_db().await;
        record(
            &db,
            NewBackupLog::ok(
                BackupSource::Manual,
                "/data/backups/pharma-backup-x.tar.gz",
                4096,
                "deadbeef",
                12,
            ),
        )
        .await
        .unwrap();

        let rows = list(&db, 10).await.unwrap();
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.status, "ok");
        assert_eq!(r.source, "manual");
        assert_eq!(r.bytes, Some(4096));
        assert_eq!(r.sha256.as_deref(), Some("deadbeef"));
        assert_eq!(r.duration_ms, Some(12));
        assert!(r.error.is_none());
    }

    #[tokio::test]
    async fn record_failed_stores_error_and_null_artifacts() {
        let db = mem_db().await;
        record(
            &db,
            NewBackupLog::failed(BackupSource::Scheduled, "disco lleno"),
        )
        .await
        .unwrap();

        let rows = list(&db, 10).await.unwrap();
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.status, "failed");
        assert_eq!(r.source, "scheduled");
        assert!(r.path.is_none());
        assert!(r.bytes.is_none());
        assert_eq!(r.error.as_deref(), Some("disco lleno"));
    }

    #[tokio::test]
    async fn list_limit_caps_returned_rows() {
        let db = mem_db().await;
        for i in 0..5 {
            record(
                &db,
                NewBackupLog::ok(BackupSource::Cli, format!("/b/{i}.tar.gz"), 1, "h", 1),
            )
            .await
            .unwrap();
        }
        assert_eq!(list(&db, 2).await.unwrap().len(), 2);
        assert_eq!(list(&db, 100).await.unwrap().len(), 5);
    }

    #[tokio::test]
    async fn list_empty_is_ok() {
        let db = mem_db().await;
        assert!(list(&db, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn prune_log_keeps_n_deletes_rest() {
        let db = mem_db().await;
        for i in 0..7 {
            record(
                &db,
                NewBackupLog::ok(BackupSource::Scheduled, format!("/b/{i}.tar.gz"), 1, "h", 1),
            )
            .await
            .unwrap();
        }
        let removed = prune_log(&db, 3).await.unwrap();
        assert_eq!(removed, 4);
        assert_eq!(list(&db, 100).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn prune_log_keep_zero_is_noop() {
        let db = mem_db().await;
        for _ in 0..3 {
            record(
                &db,
                NewBackupLog::ok(BackupSource::Scheduled, "/b.tar.gz", 1, "h", 1),
            )
            .await
            .unwrap();
        }
        assert_eq!(prune_log(&db, 0).await.unwrap(), 0);
        assert_eq!(list(&db, 100).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn prune_log_fewer_than_keep_is_noop() {
        let db = mem_db().await;
        record(
            &db,
            NewBackupLog::ok(BackupSource::Scheduled, "/b.tar.gz", 1, "h", 1),
        )
        .await
        .unwrap();
        assert_eq!(prune_log(&db, 10).await.unwrap(), 0);
        assert_eq!(list(&db, 100).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn status_assert_rejects_bad_value() {
        // The SCHEMAFULL ASSERT must reject an out-of-domain status.
        let db = mem_db().await;
        let res = db
            .query("CREATE backup_log SET status='bogus', source='cli'")
            .await
            .unwrap()
            .check();
        assert!(res.is_err(), "schema must reject status='bogus'");
    }
}

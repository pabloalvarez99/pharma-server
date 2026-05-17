//! On-demand backup of the SurrealKv data dir + agent.key.
//!
//! `POST /api/v1/admin/backup` tars+gzips the entire data directory (the
//! parent of `cfg.db.path`, which holds both the SurrealKv files and the
//! `agent.key` identity file) into a timestamped artifact under
//! `<data_dir>/backups/`. Returns the absolute path + size + sha256.
//!
//! Safety: SurrealKv is an LSM store; concurrent reads while the service is
//! running produce a snapshot that may be a few ms behind the latest commit
//! but is always crash-recoverable on restore (WAL replay). For a fully
//! quiesced backup, stop the service first.
//!
//! Role: `admin`/`owner` only. NOT tenant-scoped — the dump is per-install,
//! so any tenant restoring it sees every tenant's data.

use std::io::Write;
use std::path::Path;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

const ADMIN_ROLES: &[&str] = &["admin", "owner"];

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new().route("/api/v1/admin/backup", post(create_backup))
}

fn require_admin(claims: &auth::Claims) -> Result<(), ApiError> {
    if claims
        .roles
        .iter()
        .any(|r| ADMIN_ROLES.contains(&r.as_str()))
    {
        Ok(())
    } else {
        Err(ApiError::forbidden())
    }
}

#[derive(Serialize)]
pub struct BackupReport {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub duration_ms: u128,
}

/// Prune `<data_dir>/backups/pharma-backup-*.tar.gz` older than
/// `retention_days`. `0` means keep forever.
pub fn prune_backups(db_path: &Path, retention_days: u32) -> std::io::Result<usize> {
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
        if !name.starts_with("pharma-backup-") || !name.ends_with(".tar.gz") {
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

async fn create_backup(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<axum::response::Response, ApiError> {
    require_admin(&claims)?;
    let data_path = state
        .data_dir
        .clone()
        .ok_or_else(ApiError::service_unavailable)?;
    let report = backup_now(&data_path).map_err(|e| ApiError::internal(format!("backup: {e}")))?;
    Ok((StatusCode::CREATED, Json(report)).into_response())
}

pub fn backup_now(db_path: &Path) -> anyhow::Result<BackupReport> {
    let started_at = chrono::Utc::now();
    let started_inst = std::time::Instant::now();
    let data_dir = db_path.parent().unwrap_or_else(|| Path::new("."));
    let backups_dir = data_dir.join("backups");
    std::fs::create_dir_all(&backups_dir)?;
    let ts = started_at.format("%Y%m%dT%H%M%SZ");
    let out_path = backups_dir.join(format!("pharma-backup-{ts}.tar.gz"));

    let file = std::fs::File::create(&out_path)?;
    let gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);

    // Pack the SurrealKv data subdir (db_path itself) under `surreal/`.
    if db_path.exists() {
        tar.append_dir_all("surreal", db_path)?;
    }
    // Pack the agent.key sibling so federation identity survives the restore.
    let key_path = data_dir.join("agent.key");
    if key_path.exists() {
        let mut f = std::fs::File::open(&key_path)?;
        tar.append_file("agent.key", &mut f)?;
    }
    let gz = tar.into_inner()?;
    gz.finish()?.flush()?;

    let bytes = std::fs::metadata(&out_path)?.len();
    let hash = sha256_of(&out_path)?;
    Ok(BackupReport {
        path: out_path.to_string_lossy().into_owned(),
        bytes,
        sha256: hash,
        started_at,
        duration_ms: started_inst.elapsed().as_millis(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_removes_only_files_older_than_retention() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("surreal");
        std::fs::create_dir_all(&db_path).unwrap();
        let backups = tmp.path().join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        // Make one old file (mtime 10 days ago) and one fresh.
        let old_file = backups.join("pharma-backup-old.tar.gz");
        let new_file = backups.join("pharma-backup-new.tar.gz");
        std::fs::write(&old_file, b"old").unwrap();
        std::fs::write(&new_file, b"new").unwrap();
        let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(10 * 86_400);
        filetime::set_file_mtime(&old_file, filetime::FileTime::from(old_time)).unwrap();

        // Keep 3 days → old goes, new stays.
        let removed = prune_backups(&db_path, 3).unwrap();
        assert_eq!(removed, 1);
        assert!(!old_file.exists());
        assert!(new_file.exists());

        // retention_days = 0 → no-op.
        let removed = prune_backups(&db_path, 0).unwrap();
        assert_eq!(removed, 0);
        assert!(new_file.exists());
    }
}

fn sha256_of(p: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

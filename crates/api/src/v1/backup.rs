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
struct BackupReport {
    path: String,
    bytes: u64,
    sha256: String,
    started_at: chrono::DateTime<chrono::Utc>,
    duration_ms: u128,
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

fn backup_now(db_path: &Path) -> anyhow::Result<BackupReport> {
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

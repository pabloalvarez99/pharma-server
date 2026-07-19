use anyhow::Context;
use include_dir::{include_dir, Dir};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::Db;

const TRACKING_TABLE: &str = "_migrations";

/// Migrations embedded into the binary at compile time. This is the source of
/// truth at runtime: the installed MSI ships only `pharma-service.exe`, with no
/// `migrations/` dir on disk, so the FS-based [`run`] is dev/CLI-only while
/// services use [`run_embedded`]. Append-only — adding a `migrations/NNNN_*.surql`
/// file is picked up automatically on next build.
static EMBEDDED_MIGRATIONS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../migrations");

#[derive(Debug, Deserialize)]
struct IdOnly {
    #[allow(dead_code)]
    id: surrealdb::sql::Thing,
}

#[derive(Debug, Clone)]
pub struct MigrationOutcome {
    pub id: String,
    pub applied: bool,
}

pub async fn ensure_tracking(db: &Db) -> anyhow::Result<()> {
    db.query(format!(
        "DEFINE TABLE IF NOT EXISTS {tbl} SCHEMAFULL; \
         DEFINE FIELD IF NOT EXISTS applied_at ON {tbl} TYPE datetime DEFAULT time::now();",
        tbl = TRACKING_TABLE
    ))
    .await
    .context("defining _migrations tracking table")?
    .check()?;
    Ok(())
}

pub fn discover<P: AsRef<Path>>(dir: P) -> anyhow::Result<Vec<PathBuf>> {
    let dir = dir.as_ref();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("reading migrations dir {}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("surql"))
        .collect();
    entries.sort();
    Ok(entries)
}

fn migration_id(path: &Path) -> anyhow::Result<String> {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .with_context(|| format!("invalid migration filename {}", path.display()))
}

pub async fn is_applied(db: &Db, id: &str) -> anyhow::Result<bool> {
    let mut res = db
        .query("SELECT id FROM type::thing($tbl, $id)")
        .bind(("tbl", TRACKING_TABLE.to_string()))
        .bind(("id", id.to_string()))
        .await?;
    let rows: Vec<IdOnly> = res.take(0)?;
    Ok(!rows.is_empty())
}

/// Apply one migration's SQL and record it, skipping if already applied.
/// Shared by the FS path ([`apply_one`]) and the embedded path
/// ([`run_embedded`]) so idempotency/tracking semantics are identical.
async fn apply_sql(db: &Db, id: &str, sql: &str) -> anyhow::Result<MigrationOutcome> {
    if is_applied(db, id).await? {
        return Ok(MigrationOutcome {
            id: id.to_string(),
            applied: false,
        });
    }
    db.query(sql)
        .await
        .with_context(|| format!("executing {id}"))?
        .check()
        .with_context(|| format!("checking results of {id}"))?;
    db.query("CREATE type::thing($tbl, $id) SET applied_at = time::now();")
        .bind(("tbl", TRACKING_TABLE.to_string()))
        .bind(("id", id.to_string()))
        .await
        .with_context(|| format!("recording migration {id}"))?
        .check()?;
    Ok(MigrationOutcome {
        id: id.to_string(),
        applied: true,
    })
}

pub async fn apply_one(db: &Db, path: &Path) -> anyhow::Result<MigrationOutcome> {
    let id = migration_id(path)?;
    let sql =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    apply_sql(db, &id, &sql).await
}

pub async fn run<P: AsRef<Path>>(db: &Db, dir: P) -> anyhow::Result<Vec<MigrationOutcome>> {
    ensure_tracking(db).await?;
    let files = discover(dir)?;
    let mut out = Vec::with_capacity(files.len());
    for f in files {
        let outcome = apply_one(db, &f).await?;
        out.push(outcome);
    }
    Ok(out)
}

/// Apply all migrations embedded in the binary (see [`EMBEDDED_MIGRATIONS`]).
/// CWD-independent — used by `pharma-service` at startup so a fresh MSI install
/// has a full schema before serving, with no `migrations/` dir on disk.
/// Idempotent: re-running skips already-applied migrations.
pub async fn run_embedded(db: &Db) -> anyhow::Result<Vec<MigrationOutcome>> {
    ensure_tracking(db).await?;
    let mut files: Vec<&include_dir::File> = EMBEDDED_MIGRATIONS
        .files()
        .filter(|f| f.path().extension().and_then(|s| s.to_str()) == Some("surql"))
        .collect();
    files.sort_by(|a, b| a.path().cmp(b.path()));
    let mut out = Vec::with_capacity(files.len());
    for f in files {
        let id = f
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .with_context(|| format!("invalid embedded migration name {}", f.path().display()))?
            .to_string();
        let sql = f
            .contents_utf8()
            .with_context(|| format!("embedded migration {id} is not valid UTF-8"))?;
        out.push(apply_sql(db, &id, sql).await?);
    }
    Ok(out)
}

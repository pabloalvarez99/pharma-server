use anyhow::Context;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::Db;

const TRACKING_TABLE: &str = "_migrations";

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

pub async fn apply_one(db: &Db, path: &Path) -> anyhow::Result<MigrationOutcome> {
    let id = migration_id(path)?;
    if is_applied(db, &id).await? {
        return Ok(MigrationOutcome { id, applied: false });
    }
    let sql =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    db.query(sql)
        .await
        .with_context(|| format!("executing {}", path.display()))?
        .check()
        .with_context(|| format!("checking results of {}", path.display()))?;
    db.query("CREATE type::thing($tbl, $id) SET applied_at = time::now();")
        .bind(("tbl", TRACKING_TABLE.to_string()))
        .bind(("id", id.clone()))
        .await
        .with_context(|| format!("recording migration {}", id))?
        .check()?;
    Ok(MigrationOutcome { id, applied: true })
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

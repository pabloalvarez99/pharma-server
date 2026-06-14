//! Backup operational helpers: **restore dry-run inspection** + **count-based
//! retention**.
//!
//! The archive format is the tar+gzip produced by `api::v1::backup_now` /
//! `pharma backup create`: a `surreal/` tree (the SurrealKv data dir) plus an
//! optional `agent.key` (federation identity).
//!
//! [`inspect_snapshot`] is a READ-ONLY validation — it opens the `.tar.gz`,
//! walks every entry, and reports what a restore WOULD write, *without*
//! touching the live data dir. Run it before `pharma backup restore` (which
//! overwrites the data dir) to confirm an archive is intact and actually
//! contains a SurrealKv tree.
//!
//! [`retain_recent`] is a count-based complement to
//! `api::v1::prune_backups` (which prunes by age in days): keep the N newest
//! `pharma-backup-*.tar.gz` files, delete the rest.

use std::path::{Path, PathBuf};

const BACKUP_PREFIX: &str = "pharma-backup-";
const BACKUP_SUFFIX: &str = ".tar.gz";

/// Errors from inspecting a snapshot archive.
#[derive(Debug, thiserror::Error)]
pub enum InspectError {
    #[error("archivo no encontrado: {0}")]
    NotFound(PathBuf),
    /// Unreadable gzip/tar stream (truncated, corrupt, or not an archive).
    #[error("archivo de backup corrupto o ilegible: {0}")]
    Corrupt(#[from] std::io::Error),
}

/// What a restore from a given snapshot would write. Produced by
/// [`inspect_snapshot`] without extracting anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotInspection {
    /// The archive that was inspected.
    pub path: PathBuf,
    /// Total entries (files + dir headers) in the archive.
    pub entries: usize,
    /// Number of regular files under `surreal/`.
    pub surreal_files: usize,
    /// Sum of the uncompressed size of every file entry, in bytes.
    pub total_uncompressed_bytes: u64,
    /// Whether a `surreal/` tree is present (required for a restorable backup).
    pub has_surreal_tree: bool,
    /// Whether the federation `agent.key` is present.
    pub has_agent_key: bool,
}

impl SnapshotInspection {
    /// A snapshot is restorable iff it carries a non-empty SurrealKv tree.
    /// An archive with only `agent.key` (or nothing) would leave the DB empty.
    pub fn is_restorable(&self) -> bool {
        self.has_surreal_tree && self.surreal_files > 0
    }
}

/// Inspect a backup archive WITHOUT extracting it (restore dry-run). Opens the
/// `.tar.gz`, validates it is a readable gzip+tar stream, and reports the
/// entries a restore would write. Never touches the live data dir.
pub fn inspect_snapshot(path: &Path) -> Result<SnapshotInspection, InspectError> {
    if !path.exists() {
        return Err(InspectError::NotFound(path.to_path_buf()));
    }
    let file = std::fs::File::open(path)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(gz);

    let mut entries = 0usize;
    let mut surreal_files = 0usize;
    let mut total_uncompressed_bytes = 0u64;
    let mut has_surreal_tree = false;
    let mut has_agent_key = false;

    // Any malformed gzip/tar surfaces here as io::Error → InspectError::Corrupt.
    for entry in tar.entries()? {
        let entry = entry?;
        let header = entry.header();
        let is_file = header.entry_type().is_file();
        let size = header.size()?;
        let entry_path = entry.path()?;
        let as_str = entry_path.to_string_lossy();

        entries += 1;
        if is_file {
            total_uncompressed_bytes += size;
        }
        if as_str.starts_with("surreal") {
            has_surreal_tree = true;
            if is_file {
                surreal_files += 1;
            }
        } else if as_str == "agent.key" {
            has_agent_key = true;
        }
    }

    Ok(SnapshotInspection {
        path: path.to_path_buf(),
        entries,
        surreal_files,
        total_uncompressed_bytes,
        has_surreal_tree,
        has_agent_key,
    })
}

/// Keep the `keep` newest `pharma-backup-*.tar.gz` files in `backups_dir` and
/// delete the rest. Returns the number of files removed.
///
/// `keep == 0` is a no-op (keep all) — mirrors `prune_backups(0)` so a zero
/// default can never wipe every backup. A missing dir yields `Ok(0)`.
pub fn retain_recent(backups_dir: &Path, keep: usize) -> std::io::Result<usize> {
    if keep == 0 || !backups_dir.exists() {
        return Ok(0);
    }

    // (mtime, path) for every backup artifact, newest first.
    let mut backups: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(backups_dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with(BACKUP_PREFIX) || !name.ends_with(BACKUP_SUFFIX) {
                return None;
            }
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((mtime, e.path()))
        })
        .collect();

    if backups.len() <= keep {
        return Ok(0);
    }

    backups.sort_by_key(|(mtime, _)| std::cmp::Reverse(*mtime));

    let mut removed = 0usize;
    for (_, path) in backups.into_iter().skip(keep) {
        if std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a `surreal/` + `agent.key` tar.gz like `backup create` produces.
    fn write_archive(path: &Path, with_surreal: bool, with_key: bool) {
        let file = std::fs::File::create(path).unwrap();
        let gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(gz);
        if with_surreal {
            let tmp = tempfile::tempdir().unwrap();
            let surreal = tmp.path().join("surreal");
            std::fs::create_dir_all(&surreal).unwrap();
            std::fs::write(surreal.join("data.db"), b"fake-surreal-data").unwrap();
            tar.append_dir_all("surreal", &surreal).unwrap();
        }
        if with_key {
            let tmp = tempfile::tempdir().unwrap();
            let key = tmp.path().join("agent.key");
            std::fs::write(&key, b"fake-key").unwrap();
            let mut f = std::fs::File::open(&key).unwrap();
            tar.append_file("agent.key", &mut f).unwrap();
        }
        let gz = tar.into_inner().unwrap();
        gz.finish().unwrap().flush().unwrap();
    }

    #[test]
    fn inspect_valid_archive_reports_surreal_and_key() {
        let tmp = tempfile::tempdir().unwrap();
        let arc = tmp.path().join("snap.tar.gz");
        write_archive(&arc, true, true);

        let rep = inspect_snapshot(&arc).unwrap();
        assert!(rep.has_surreal_tree);
        assert!(rep.has_agent_key);
        assert!(rep.surreal_files >= 1);
        assert!(rep.total_uncompressed_bytes > 0);
        assert!(rep.is_restorable());
    }

    #[test]
    fn inspect_does_not_extract_or_touch_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let arc = tmp.path().join("snap.tar.gz");
        write_archive(&arc, true, true);

        let before: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect();
        inspect_snapshot(&arc).unwrap();
        let after: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect();
        // Dry-run extracted nothing — the dir is unchanged (no surreal/, no agent.key).
        assert_eq!(before.len(), after.len());
        assert!(!tmp.path().join("surreal").exists());
        assert!(!tmp.path().join("agent.key").exists());
    }

    #[test]
    fn inspect_archive_without_surreal_is_not_restorable() {
        let tmp = tempfile::tempdir().unwrap();
        let arc = tmp.path().join("keyonly.tar.gz");
        write_archive(&arc, false, true);

        let rep = inspect_snapshot(&arc).unwrap();
        assert!(!rep.has_surreal_tree);
        assert!(rep.has_agent_key);
        assert!(!rep.is_restorable());
    }

    #[test]
    fn inspect_missing_file_errors_notfound() {
        let tmp = tempfile::tempdir().unwrap();
        let arc = tmp.path().join("nope.tar.gz");
        let err = inspect_snapshot(&arc).unwrap_err();
        assert!(matches!(err, InspectError::NotFound(_)));
    }

    #[test]
    fn inspect_corrupt_archive_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let arc = tmp.path().join("corrupt.tar.gz");
        // Not a gzip stream at all.
        std::fs::write(&arc, b"this is definitely not a gzip archive").unwrap();
        let err = inspect_snapshot(&arc).unwrap_err();
        assert!(matches!(err, InspectError::Corrupt(_)));
    }

    #[test]
    fn retain_recent_keeps_n_newest() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        // 5 backups with increasing mtimes.
        for i in 0..5 {
            let p = dir.join(format!("pharma-backup-2025010{i}-000000.tar.gz"));
            std::fs::write(&p, b"x").unwrap();
            let t = std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(1_000_000 + i as u64 * 86_400);
            filetime::set_file_mtime(&p, filetime::FileTime::from(t)).unwrap();
        }
        let removed = retain_recent(dir, 2).unwrap();
        assert_eq!(removed, 3);
        let left: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with(BACKUP_PREFIX))
            .collect();
        assert_eq!(left.len(), 2);
        // The two newest (days 3 and 4) survive.
        assert!(left.iter().any(|n| n.contains("20250104")));
        assert!(left.iter().any(|n| n.contains("20250103")));
    }

    #[test]
    fn retain_recent_keep_zero_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        for i in 0..3 {
            std::fs::write(dir.join(format!("pharma-backup-{i}.tar.gz")), b"x").unwrap();
        }
        assert_eq!(retain_recent(dir, 0).unwrap(), 0);
        assert_eq!(
            std::fs::read_dir(dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .count(),
            3
        );
    }

    #[test]
    fn retain_recent_fewer_than_keep_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("pharma-backup-1.tar.gz"), b"x").unwrap();
        assert_eq!(retain_recent(dir, 5).unwrap(), 0);
    }

    #[test]
    fn retain_recent_ignores_non_backup_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("pharma-backup-1.tar.gz"), b"x").unwrap();
        std::fs::write(dir.join("pharma-backup-2.tar.gz"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"keep me").unwrap();
        // keep 1 → one backup removed; the txt is untouched.
        let removed = retain_recent(dir, 1).unwrap();
        assert_eq!(removed, 1);
        assert!(dir.join("notes.txt").exists());
    }

    #[test]
    fn retain_recent_missing_dir_is_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        assert_eq!(retain_recent(&missing, 3).unwrap(), 0);
    }
}

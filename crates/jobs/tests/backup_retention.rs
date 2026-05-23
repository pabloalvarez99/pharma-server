//! Integration test for [`jobs::prune_auto_backups`] retention pruning.
//!
//! Lays down 10 fake `auto-YYYY-MM-DD.snapshot` files with mtimes staggered
//! 0..=9 days in the past, then prunes with `retention_days = 7`. The cutoff is
//! `now - 7d`, so the 3 files older than 7 days are removed and the 7 newest
//! survive. Also asserts that a sibling on-demand `pharma-backup-*.tar.gz`
//! artifact is never touched (different naming) and that the count is exact.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use filetime::{set_file_mtime, FileTime};

/// Build `<tmp>/surreal` (the db_path) + sibling `<tmp>/backups` dir, returning
/// both the db_path and the backups dir.
fn scaffold() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("surreal");
    std::fs::create_dir_all(&db_path).unwrap();
    let backups = tmp.path().join("backups");
    std::fs::create_dir_all(&backups).unwrap();
    (tmp, db_path, backups)
}

#[test]
fn prune_keeps_seven_newest_of_ten_daily_snapshots() {
    let (_tmp, db_path, backups) = scaffold();
    let now = SystemTime::now();

    // 10 daily snapshots, mtime 0..=9 days ago.
    for days_ago in 0..10u64 {
        let name = format!("auto-2026-05-{:02}.snapshot", 23 - days_ago);
        let path = backups.join(&name);
        std::fs::write(&path, b"snapshot").unwrap();
        let mtime = now - Duration::from_secs(days_ago * 86_400);
        set_file_mtime(&path, FileTime::from(mtime)).unwrap();
    }
    // A sibling on-demand artifact (different prefix/suffix) that must survive.
    let ondemand = backups.join("pharma-backup-20260514T010101Z.tar.gz");
    std::fs::write(&ondemand, b"ondemand").unwrap();
    set_file_mtime(
        &ondemand,
        FileTime::from(now - Duration::from_secs(20 * 86_400)),
    )
    .unwrap();

    // Keep last 7 days → files at mtime 7,8,9 days ago are dropped (3 removed).
    let removed = jobs::prune_auto_backups(&db_path, 7).unwrap();
    assert_eq!(removed, 3, "3 snapshots older than 7 days should be pruned");

    // 7 auto snapshots remain.
    let survivors: Vec<String> = std::fs::read_dir(&backups)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("auto-") && n.ends_with(".snapshot"))
        .collect();
    assert_eq!(
        survivors.len(),
        7,
        "7 newest snapshots survive: {survivors:?}"
    );

    // On-demand artifact untouched despite being 20 days old.
    assert!(ondemand.exists(), "on-demand backup must not be pruned");
}

#[test]
fn prune_zero_retention_keeps_everything() {
    let (_tmp, db_path, backups) = scaffold();
    let now = SystemTime::now();
    for days_ago in 0..10u64 {
        let path = backups.join(format!("auto-2026-04-{:02}.snapshot", 10 + days_ago));
        std::fs::write(&path, b"x").unwrap();
        set_file_mtime(
            &path,
            FileTime::from(now - Duration::from_secs(days_ago * 86_400)),
        )
        .unwrap();
    }
    let removed = jobs::prune_auto_backups(&db_path, 0).unwrap();
    assert_eq!(removed, 0, "retention 0 = keep forever");
    let count = std::fs::read_dir(&backups).unwrap().count();
    assert_eq!(count, 10);
}

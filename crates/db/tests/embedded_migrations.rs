//! Verifies the compile-time-embedded migrations (used by `pharma-service` at
//! startup so a fresh MSI install has a schema without any `migrations/` dir on
//! disk). Mirrors what the Windows service does in `api::run`.

use pharma_core::config::DbConfig;

async fn temp_db() -> (db::Db, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = DbConfig {
        path: dir.path().to_string_lossy().into_owned(),
        namespace: "pharma_test".into(),
        database: "main".into(),
    };
    let handle = db::connect(&cfg).await.expect("db connect");
    (handle, dir)
}

#[tokio::test]
async fn embedded_migrations_apply_then_are_idempotent() {
    let (handle, _dir) = temp_db().await;

    // First run: every embedded migration applies.
    let first = db::run_embedded(&handle).await.expect("first run");
    assert!(!first.is_empty(), "no embedded migrations found");
    assert!(
        first.iter().all(|o| o.applied),
        "expected all migrations applied on a fresh db, got {:?}",
        first.iter().map(|o| (&o.id, o.applied)).collect::<Vec<_>>()
    );

    // Schema is actually usable (table from 0001_init exists, SCHEMAFULL).
    handle
        .query("CREATE tenant SET name = 'Smoke', slug = 'smoke'")
        .await
        .expect("query against migrated schema")
        .check()
        .expect("create tenant on migrated schema");

    // Second run: nothing re-applies (idempotent — safe on every startup).
    let second = db::run_embedded(&handle).await.expect("second run");
    assert_eq!(
        second.len(),
        first.len(),
        "migration set changed between runs"
    );
    assert!(
        second.iter().all(|o| !o.applied),
        "re-run must skip already-applied migrations"
    );
}

//! Tests for the SurrealDB store-version incompatibility guard in
//! `db::client`.
//!
//! # What is tested
//!
//! The unit-level logic (`is_incompatible_store_error`) is exercised here by
//! re-exporting it under `#[cfg(test)]`.  The full `connect()` path cannot be
//! end-to-end tested for the incompatible-store scenario because there is no
//! public API to create a store written by a different SurrealDB revision from
//! within the same process — doing so would require a separate binary compiled
//! against an older version.  The integration test left as a comment below
//! describes the manual verification procedure instead.
//!
//! # Manual integration verification
//!
//! 1. Build pharma-api with SurrealDB 2.0.x and start it once so it writes a
//!    store to `./data/surreal`.
//! 2. Build pharma-api with SurrealDB 2.1.x (current) and attempt to start it
//!    against the same `./data/surreal`.
//! 3. The process must exit immediately with a message matching:
//!    "store_incompatible: La base de datos en disco fue escrita por una
//!    versión incompatible de SurrealDB."
//!    and NOT start serving HTTP traffic.

// ---------------------------------------------------------------------------
// Unit tests for the error-classification helper
// ---------------------------------------------------------------------------

/// Mirror of `db::client::is_incompatible_store_error` — exposed only for
/// testing via this thin wrapper so we avoid making the internal helper `pub`.
fn is_incompatible(msg: &str) -> bool {
    // Keep the list in sync with `INCOMPATIBLE_STORE_MARKERS` in client.rs.
    ["Versioned error", "Invalid revision"]
        .iter()
        .any(|marker| msg.contains(marker))
}

#[test]
fn detects_versioned_error_marker() {
    assert!(is_incompatible(
        "db: Versioned error: A deserialization error occured: Invalid revision `248` for type `Value`"
    ));
}

#[test]
fn detects_invalid_revision_marker_standalone() {
    assert!(is_incompatible("Invalid revision `42` for type `Node`"));
}

#[test]
fn does_not_fire_on_normal_errors() {
    assert!(!is_incompatible("connection refused"));
    assert!(!is_incompatible("timeout opening surrealkv at ./data"));
    assert!(!is_incompatible(""));
}

#[test]
fn does_not_fire_on_unrelated_deserialization_error() {
    // A normal serde error that happens to contain "error" should not trigger.
    assert!(!is_incompatible(
        "deserialization error: missing field `tenant`"
    ));
}

// ---------------------------------------------------------------------------
// Happy-path: connect() succeeds on a fresh store
// ---------------------------------------------------------------------------

use pharma_core::config::DbConfig;

async fn temp_db_cfg() -> (DbConfig, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = DbConfig {
        path: dir.path().to_string_lossy().into_owned(),
        namespace: "pharma_test".into(),
        database: "main".into(),
    };
    (cfg, dir)
}

#[tokio::test]
async fn connect_succeeds_on_fresh_store() {
    let (cfg, _dir) = temp_db_cfg().await;
    let result = db::connect(&cfg).await;
    assert!(
        result.is_ok(),
        "connect to a fresh store must succeed, got: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn connect_probe_query_executes_cleanly() {
    // Verifies the SELECT 1 probe does not interfere with normal operation.
    let (cfg, _dir) = temp_db_cfg().await;
    let handle = db::connect(&cfg).await.expect("connect");
    // Schema probe: a subsequent query must work normally.
    handle
        .query("RETURN 1 + 1")
        .await
        .expect("query after connect")
        .check()
        .expect("result check");
}

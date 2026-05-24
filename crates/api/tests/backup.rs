//! Backup endpoint integration test. Spins up an isolated tempdir SurrealKv,
//! seeds a tenant + admin user with a bearer token, hits POST
//! /api/v1/admin/backup, and verifies the resulting tar.gz exists, contains
//! the expected entries, and the JSON report matches the file on disk.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use flate2::read::GzDecoder;
use http_body_util::BodyExt;
use pharma_core::config::{DbConfig, JwtConfig};
use std::collections::HashSet;
use tempfile::TempDir;
use tower::ServiceExt;

const MIGRATIONS_DIR: &str = "../../migrations";

async fn spawn() -> (axum::Router, String, TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data_dir = tmp.path().to_path_buf();
    let db_path = data_dir.join("surreal");
    let cfg = DbConfig {
        path: db_path.to_string_lossy().into_owned(),
        namespace: "pharma_test".into(),
        database: "main".into(),
    };
    let handle = db::connect(&cfg).await.expect("connect");
    db::run_migrations(&handle, MIGRATIONS_DIR)
        .await
        .expect("migr");

    // Tenant + admin user with a valid bearer.
    let mut r = handle
        .query("CREATE tenant SET name='T', slug='t' RETURN id")
        .await
        .unwrap();
    let tenant: surrealdb::sql::Thing = r.take::<Option<_>>((0, "id")).unwrap().unwrap();
    let mut r = handle
        .query("CREATE user SET tenant=$t, email='a@t.l', password='x', roles=['admin'] RETURN id")
        .bind(("t", tenant.clone()))
        .await
        .unwrap();
    let user: surrealdb::sql::Thing = r.take::<Option<_>>((0, "id")).unwrap().unwrap();

    // Drop a stub agent.key so the backup test asserts both artifacts land in
    // the archive without needing the full identity init path.
    std::fs::write(data_dir.join("agent.key"), b"stub-key\n").unwrap();

    let jwt = JwtConfig {
        secret: "test-secret-test-secret-test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    };
    let token = auth::issue(
        &jwt,
        &user.to_string(),
        &tenant.to_string(),
        vec!["admin".into()],
    )
    .unwrap();
    let state = api::AppState {
        started_at: chrono::Utc::now(),
        jwt,
        db: Some(Arc::new(handle)),
        metrics_token: None,
        node_identity: None,
        data_dir: Some(db_path.clone()),
        license: Arc::new(arc_swap::ArcSwap::from_pointee(
            license::License::free_default(uuid::Uuid::nil()),
        )),
        license_path: None,
        rate_limit: None,
        public_catalog: pharma_core::config::PublicCatalogConfig::default(),
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
    };
    (api::build_router(state), token, tmp, db_path)
}

#[tokio::test]
async fn backup_endpoint_creates_tarball_with_surreal_and_agent_key() {
    let (app, token, _tmp, db_path) = spawn().await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/backup")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let path = v["path"].as_str().unwrap();
    let bytes = v["bytes"].as_u64().unwrap();
    let sha = v["sha256"].as_str().unwrap();
    assert!(path.ends_with(".tar.gz"));
    assert!(bytes > 0);
    assert_eq!(sha.len(), 64);

    // File on disk matches the report.
    let on_disk = std::fs::metadata(path).unwrap();
    assert_eq!(on_disk.len(), bytes);

    // Tarball contains expected entries: agent.key + at least one path
    // starting with `surreal/`.
    let f = std::fs::File::open(path).unwrap();
    let gz = GzDecoder::new(f);
    let mut tar = tar::Archive::new(gz);
    let mut names: HashSet<String> = HashSet::new();
    for entry in tar.entries().unwrap() {
        let e = entry.unwrap();
        names.insert(e.path().unwrap().to_string_lossy().into_owned());
    }
    assert!(names.iter().any(|n| n == "agent.key"));
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("surreal/") || n == "surreal"),
        "expected at least one entry under surreal/, got {:?}",
        names
    );

    // Backups dir landed where we expect (sibling of db_path).
    let backups_dir = db_path.parent().unwrap().join("backups");
    let listed: Vec<_> = std::fs::read_dir(&backups_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(listed.len(), 1);
}

#[tokio::test]
async fn backup_endpoint_rejects_without_bearer() {
    let (app, _token, _tmp, _db) = spawn().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/backup")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

//! Integration tests for `POST /api/v1/admin/import-customers`.
//!
//! End-to-end: real SurrealKv (tempdir) + migrations + axum router. Covers
//! the Supabase-migration contract:
//!
//!  1. Happy path → all rows landed as `created`, no errors.
//!  2. Re-importing the same email → all rows go to `updated`, none created.
//!  3. One bad row (malformed email) → that row in `errors[]`, others go in.
//!  4. Cross-tenant isolation — tenant B cannot see tenant A's customers.
//!  5. Non-admin role (cashier) → 403, no DB mutation.
//!
//! Tests use the same harness shape as `integration_db.rs` so future tests
//! can share helpers without churning the seed code.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use pharma_core::config::{DbConfig, JwtConfig};
use tempfile::TempDir;
use tower::ServiceExt;

const MIGRATIONS_DIR: &str = "../../migrations";

fn jwt_cfg() -> JwtConfig {
    JwtConfig {
        secret: "test-secret".into(),
        issuer: "pharma-test".into(),
        ttl_seconds: 60,
    }
}

struct TestDb {
    db: Arc<db::Db>,
    _dir: TempDir,
}

async fn spawn_test_db() -> TestDb {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = DbConfig {
        path: dir.path().to_string_lossy().into_owned(),
        namespace: "pharma_test".into(),
        database: "main".into(),
    };
    let handle = db::connect(&cfg).await.expect("db connect");
    db::run_migrations(&handle, MIGRATIONS_DIR)
        .await
        .expect("run migrations");
    TestDb {
        db: Arc::new(handle),
        _dir: dir,
    }
}

async fn seed_tenant(db: &db::Db, slug: &str) -> String {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let mut t = db
        .query("CREATE tenant SET name = $name, slug = $slug RETURN AFTER")
        .bind(("name", format!("Tenant {slug}")))
        .bind(("slug", slug.to_string()))
        .await
        .expect("create tenant");
    let row: Option<Row> = t.take(0).expect("decode tenant");
    row.expect("tenant row").id.to_string()
}

fn state_with_db(db: Arc<db::Db>) -> api::AppState {
    api::AppState {
        started_at: chrono::Utc::now(),
        jwt: jwt_cfg(),
        db: Some(db),
        metrics_token: None,
        node_identity: None,
        data_dir: None,
        license: Arc::new(arc_swap::ArcSwap::from_pointee(
            license::License::free_default(uuid::Uuid::nil()),
        )),
        license_path: None,
    }
}

fn token_for(tenant_id: &str, role: &str) -> String {
    auth::issue(&jwt_cfg(), "user:u1", tenant_id, vec![role.into()]).unwrap()
}

async fn post_import(
    app: &axum::Router,
    token: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/import-customers")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

#[tokio::test]
async fn three_valid_customers_all_created() {
    let t = spawn_test_db().await;
    let tenant_id = seed_tenant(&t.db, "acme").await;
    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant_id, "admin");

    let body = serde_json::json!({
        "customers": [
            { "email": "Adan@example.cl", "name": "Adan Ardiles", "phone": "+56974713824" },
            { "email": "gloria@example.cl", "name": "Gloria Cortes" },
            { "email": "admin@example.cl", "name": "Admin User", "rut": "12.345.678-9" },
        ]
    });
    let (status, json) = post_import(&app, &token, body).await;

    assert_eq!(status, StatusCode::OK, "body={json}");
    assert_eq!(json["created"], 3);
    assert_eq!(json["updated"], 0);
    assert_eq!(json["errors"].as_array().unwrap().len(), 0);

    // Email must have been lowercased on persistence.
    #[derive(serde::Deserialize)]
    struct Row {
        email: Option<String>,
        name: String,
    }
    let mut q =
        t.db.query("SELECT email, name FROM customer WHERE tenant = $t ORDER BY email")
            .bind(("t", surrealdb::sql::thing(&tenant_id).expect("thing")))
            .await
            .unwrap();
    let rows: Vec<Row> = q.take(0).unwrap();
    assert_eq!(rows.len(), 3, "exactly 3 customer rows persisted");
    assert_eq!(rows[0].email.as_deref(), Some("adan@example.cl"));
    assert_eq!(rows[0].name, "Adan Ardiles");
}

#[tokio::test]
async fn reimport_same_emails_updates_not_creates() {
    let t = spawn_test_db().await;
    let tenant_id = seed_tenant(&t.db, "acme").await;
    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant_id, "admin");

    let first = serde_json::json!({
        "customers": [
            { "email": "a@x.cl", "name": "Old Name A" },
            { "email": "b@x.cl", "name": "Old Name B" },
        ]
    });
    let (s1, j1) = post_import(&app, &token, first).await;
    assert_eq!(s1, StatusCode::OK);
    assert_eq!(j1["created"], 2);
    assert_eq!(j1["updated"], 0);

    // Same emails, different names + extra phone — names should overwrite.
    let second = serde_json::json!({
        "customers": [
            { "email": "a@x.cl", "name": "New Name A", "phone": "+56911111111" },
            { "email": "b@x.cl", "name": "New Name B" },
        ]
    });
    let (s2, j2) = post_import(&app, &token, second).await;
    assert_eq!(s2, StatusCode::OK, "body={j2}");
    assert_eq!(j2["created"], 0, "no new rows on re-import");
    assert_eq!(j2["updated"], 2);

    // Verify the update actually applied (name + phone).
    #[derive(serde::Deserialize)]
    struct Row {
        name: String,
        phone: Option<String>,
    }
    let mut q = t
        .db
        .query("SELECT name, phone FROM customer WHERE tenant = $t AND email = 'a@x.cl' LIMIT 1")
        .bind(("t", surrealdb::sql::thing(&tenant_id).expect("thing")))
        .await
        .unwrap();
    let rows: Vec<Row> = q.take(0).unwrap();
    let row = rows.first().expect("row A");
    assert_eq!(row.name, "New Name A");
    assert_eq!(row.phone.as_deref(), Some("+56911111111"));
}

#[tokio::test]
async fn invalid_email_collects_error_others_succeed() {
    let t = spawn_test_db().await;
    let tenant_id = seed_tenant(&t.db, "acme").await;
    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant_id, "admin");

    let body = serde_json::json!({
        "customers": [
            { "email": "ok1@x.cl", "name": "OK One" },
            { "email": "not-an-email", "name": "Bad Row" }, // idx 1, must error
            { "email": "", "name": "Also Bad" },             // idx 2, must error
            { "email": "ok2@x.cl", "name": "OK Two" },
        ]
    });
    let (status, json) = post_import(&app, &token, body).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "partial failure still returns 200; body={json}"
    );
    assert_eq!(json["created"], 2, "two good rows landed");
    assert_eq!(json["updated"], 0);
    let errors = json["errors"].as_array().expect("errors array");
    assert_eq!(errors.len(), 2);
    let idxs: Vec<u64> = errors.iter().map(|e| e["idx"].as_u64().unwrap()).collect();
    assert!(idxs.contains(&1), "bad email row reported");
    assert!(idxs.contains(&2), "empty email row reported");
}

#[tokio::test]
async fn cross_tenant_isolation() {
    let t = spawn_test_db().await;
    let tenant_a = seed_tenant(&t.db, "acme-a").await;
    let tenant_b = seed_tenant(&t.db, "acme-b").await;
    let app = api::build_router(state_with_db(t.db.clone()));

    // Import same email under two distinct tenants — both must create, no
    // cross-pollination. Then re-import on A must NOT touch B.
    let token_a = token_for(&tenant_a, "admin");
    let token_b = token_for(&tenant_b, "admin");

    let body = serde_json::json!({
        "customers": [{ "email": "shared@x.cl", "name": "Same Email" }]
    });
    let (sa, ja) = post_import(&app, &token_a, body.clone()).await;
    assert_eq!(sa, StatusCode::OK);
    assert_eq!(ja["created"], 1);

    let (sb, jb) = post_import(&app, &token_b, body.clone()).await;
    assert_eq!(sb, StatusCode::OK);
    assert_eq!(jb["created"], 1, "same email is a NEW row under tenant B");

    // Re-import on A: should be UPDATE (1), and B's row must stay untouched.
    let body2 = serde_json::json!({
        "customers": [{ "email": "shared@x.cl", "name": "Updated In A" }]
    });
    let (sa2, ja2) = post_import(&app, &token_a, body2).await;
    assert_eq!(sa2, StatusCode::OK);
    assert_eq!(ja2["updated"], 1);
    assert_eq!(ja2["created"], 0);

    // B's row must still hold the original name.
    #[derive(serde::Deserialize)]
    struct Row {
        name: String,
    }
    let mut q =
        t.db.query("SELECT name FROM customer WHERE tenant = $t AND email = 'shared@x.cl' LIMIT 1")
            .bind(("t", surrealdb::sql::thing(&tenant_b).expect("thing")))
            .await
            .unwrap();
    let rows: Vec<Row> = q.take(0).unwrap();
    assert_eq!(rows.first().expect("B row").name, "Same Email");
}

#[tokio::test]
async fn non_admin_role_returns_403() {
    let t = spawn_test_db().await;
    let tenant_id = seed_tenant(&t.db, "acme").await;
    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant_id, "cashier");

    let body = serde_json::json!({
        "customers": [{ "email": "x@x.cl", "name": "Forbidden" }]
    });
    let (status, json) = post_import(&app, &token, body).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "body={json}");
    assert_eq!(json["error"]["code"], "FORBIDDEN");

    // No row should have been created.
    #[derive(serde::Deserialize)]
    struct Count {
        c: i64,
    }
    let mut q =
        t.db.query("SELECT count() AS c FROM customer WHERE tenant = $t GROUP ALL")
            .bind(("t", surrealdb::sql::thing(&tenant_id).expect("thing")))
            .await
            .unwrap();
    let rows: Vec<Count> = q.take(0).unwrap();
    let count = rows.first().map(|r| r.c).unwrap_or(0);
    assert_eq!(count, 0, "cashier must not have mutated any row");
}

#[tokio::test]
async fn batch_over_max_returns_400() {
    let t = spawn_test_db().await;
    let tenant_id = seed_tenant(&t.db, "acme").await;
    let app = api::build_router(state_with_db(t.db.clone()));
    let token = token_for(&tenant_id, "admin");

    let mut customers = Vec::new();
    for i in 0..201 {
        customers.push(serde_json::json!({
            "email": format!("u{i}@x.cl"),
            "name": format!("User {i}"),
        }));
    }
    let body = serde_json::json!({ "customers": customers });
    let (status, json) = post_import(&app, &token, body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body={json}");
    assert_eq!(json["error"]["code"], "BATCH_TOO_LARGE");
}

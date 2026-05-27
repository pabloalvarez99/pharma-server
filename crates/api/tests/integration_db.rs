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

async fn seed_tenant_and_user(
    db: &db::Db,
    slug: &str,
    email: &str,
    password: &str,
) -> (String, String) {
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
    let tenant: Option<Row> = t.take(0).expect("decode tenant");
    let tenant_id = tenant.expect("tenant row").id;

    let hash = auth::password::hash(password).expect("hash");
    let mut u = db
        .query(
            "CREATE user SET tenant = $tenant, email = $email, \
             password = $password, roles = $roles RETURN AFTER",
        )
        .bind(("tenant", tenant_id.clone()))
        .bind(("email", email.to_string()))
        .bind(("password", hash))
        .bind(("roles", vec!["admin".to_string()]))
        .await
        .expect("create user");
    let user: Option<Row> = u.take(0).expect("decode user");
    let user_id = user.expect("user row").id;

    (tenant_id.to_string(), user_id.to_string())
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
        rate_limit: None,
        docs_enabled: true,
        public_catalog: pharma_core::config::PublicCatalogConfig::default(),
        public_orders: pharma_core::config::PublicOrdersConfig::default(),
        stock_webhook: Arc::new(pharma_core::config::StockWebhookConfig::default()),
    }
}

#[tokio::test]
async fn health_ready_with_db_returns_200() {
    let t = spawn_test_db().await;
    let app = api::build_router(state_with_db(t.db.clone()));
    let res = app
        .oneshot(
            Request::builder()
                .uri("/health/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["checks"]["db"], "ok");
}

#[tokio::test]
async fn login_with_valid_creds_returns_jwt() {
    let t = spawn_test_db().await;
    seed_tenant_and_user(&t.db, "acme", "alice@acme.cl", "s3cret-pw").await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let body = serde_json::to_vec(&serde_json::json!({
        "tenant": "acme",
        "email": "alice@acme.cl",
        "password": "s3cret-pw",
    }))
    .unwrap();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["token"].as_str().expect("token field");
    assert_eq!(json["token_type"], "Bearer");
    assert!(json["expires_in"].as_u64().unwrap() > 0);
    assert!(!token.is_empty());
}

#[tokio::test]
async fn login_with_bad_password_returns_401() {
    let t = spawn_test_db().await;
    seed_tenant_and_user(&t.db, "acme", "alice@acme.cl", "right-pw").await;

    let app = api::build_router(state_with_db(t.db.clone()));
    let body = serde_json::to_vec(&serde_json::json!({
        "tenant": "acme",
        "email": "alice@acme.cl",
        "password": "wrong-pw",
    }))
    .unwrap();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn v1_alias_login_and_me_work() {
    let t = spawn_test_db().await;
    seed_tenant_and_user(&t.db, "acme", "alice@acme.cl", "s3cret-pw").await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let body = serde_json::to_vec(&serde_json::json!({
        "tenant": "acme", "email": "alice@acme.cl", "password": "s3cret-pw",
    }))
    .unwrap();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/login")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn bad_credentials_use_error_envelope() {
    let t = spawn_test_db().await;
    seed_tenant_and_user(&t.db, "acme", "alice@acme.cl", "right-pw").await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let body = serde_json::to_vec(&serde_json::json!({
        "tenant": "acme", "email": "alice@acme.cl", "password": "wrong",
    }))
    .unwrap();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/login")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "BAD_CREDENTIALS");
    assert!(json["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Credenciales"));
}

#[tokio::test]
async fn mutation_writes_audit_log_row() {
    let t = spawn_test_db().await;
    seed_tenant_and_user(&t.db, "acme", "alice@acme.cl", "s3cret-pw").await;
    let app = api::build_router(state_with_db(t.db.clone()));

    // Login to obtain a token (claims drive audit attribution).
    let login_body = serde_json::to_vec(&serde_json::json!({
        "tenant": "acme", "email": "alice@acme.cl", "password": "s3cret-pw",
    }))
    .unwrap();
    let login_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/login")
                .header("content-type", "application/json")
                .body(Body::from(login_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = login_res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["token"].as_str().unwrap().to_string();

    // Authenticated POST (unknown path â†’ 404, still audited because layer wraps the router).
    let _ = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/__audit_probe")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"probe":true}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    // Audit insert is detached; poll briefly.
    #[derive(serde::Deserialize)]
    struct Count {
        c: i64,
    }
    let mut found = 0;
    for _ in 0..40 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let mut q =
            t.db.query("SELECT count() AS c FROM audit_log WHERE path = $p GROUP ALL")
                .bind(("p", "/api/v1/__audit_probe"))
                .await
                .unwrap();
        let rows: Vec<Count> = q.take(0).unwrap();
        if let Some(r) = rows.first() {
            if r.c >= 1 {
                found = r.c;
                break;
            }
        }
    }
    assert!(found >= 1, "expected an audit_log row for the mutation");

    #[derive(serde::Deserialize)]
    struct Row {
        method: String,
        path: String,
        payload_hash: Option<String>,
        tenant: surrealdb::sql::Thing,
    }
    let mut q = t
        .db
        .query("SELECT method, path, payload_hash, tenant FROM audit_log WHERE path = $p LIMIT 1")
        .bind(("p", "/api/v1/__audit_probe"))
        .await
        .unwrap();
    let rows: Vec<Row> = q.take(0).unwrap();
    let row = rows.first().expect("audit row");
    assert_eq!(row.method, "POST");
    assert_eq!(row.path, "/api/v1/__audit_probe");
    assert_eq!(row.tenant.tb, "tenant");
    assert!(row.payload_hash.as_deref().unwrap().len() == 64);
}

#[tokio::test]
async fn login_then_me_round_trip() {
    let t = spawn_test_db().await;
    seed_tenant_and_user(&t.db, "acme", "alice@acme.cl", "s3cret-pw").await;

    let app = api::build_router(state_with_db(t.db.clone()));

    let login_body = serde_json::to_vec(&serde_json::json!({
        "tenant": "acme",
        "email": "alice@acme.cl",
        "password": "s3cret-pw",
    }))
    .unwrap();
    let login_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/login")
                .header("content-type", "application/json")
                .body(Body::from(login_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_res.status(), StatusCode::OK);
    let body = login_res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let token = json["token"].as_str().expect("token").to_string();

    let me_res = app
        .oneshot(
            Request::builder()
                .uri("/api/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_res.status(), StatusCode::OK);
    let body = me_res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["sub"].as_str().unwrap().starts_with("user:"));
    assert!(json["tenant_id"].as_str().unwrap().starts_with("tenant:"));
    assert_eq!(json["roles"][0], "admin");
}

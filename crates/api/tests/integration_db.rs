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

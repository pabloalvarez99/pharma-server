//! User-management HTTP tests: admin-gated credential CRUD, tenant isolation,
//! anti-lockout guards, password handling, and the create→login roundtrip.
//!
//! Real kv-surrealkv store (all migrations) + JWT, same harness as
//! `config_branches.rs` / `integration_db.rs`.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use pharma_core::config::{DbConfig, JwtConfig};
use serde_json::{json, Value};
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

/// Create a tenant + a user with the given roles. Returns `(tenant_id, jwt)`.
/// The user's email is `u@<slug>.cl`; its DB password is a plaintext placeholder
/// (these seeded users authenticate via the directly-issued JWT, never `/login`).
async fn seed_user(db: &db::Db, jwt: &JwtConfig, slug: &str, roles: &[&str]) -> (String, String) {
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

    let role_vec: Vec<String> = roles.iter().map(|s| s.to_string()).collect();
    let mut u = db
        .query("CREATE user SET tenant = $t, email = $email, password = 'x', roles = $roles RETURN AFTER")
        .bind(("t", tenant_id.clone()))
        .bind(("email", format!("u@{slug}.cl")))
        .bind(("roles", role_vec.clone()))
        .await
        .expect("create user");
    let user: Option<Row> = u.take(0).expect("decode user");
    let user_id = user.expect("user row").id;

    let token = auth::issue(jwt, &user_id.to_string(), &tenant_id.to_string(), role_vec)
        .expect("issue jwt");
    (tenant_id.to_string(), token)
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

async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: &str,
    body: Value,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn get(app: &axum::Router, uri: &str, token: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

/// Unauthenticated POST (for `/api/v1/login`).
async fn post_anon(app: &axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

fn id_of_email(list: &Value, email: &str) -> String {
    list.as_array()
        .expect("list array")
        .iter()
        .find(|u| u["email"] == email)
        .unwrap_or_else(|| panic!("user {email} not in list"))["id"]
        .as_str()
        .expect("id str")
        .to_string()
}

#[tokio::test]
async fn create_list_and_login_roundtrip() {
    let t = spawn_test_db().await;
    let (_tenant, admin) = seed_user(&t.db, &jwt_cfg(), "acme", &["admin"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    // Admin creates a cashier credential.
    let (st, created) = send(
        &app,
        "POST",
        "/api/v1/users",
        &admin,
        json!({ "email": "  Cajero@Acme.CL ", "password": "Cajero123", "roles": ["cashier"] }),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "create user: {created}");
    assert_eq!(created["email"], "cajero@acme.cl", "email normalized");
    assert_eq!(created["roles"], json!(["cashier"]));
    assert_eq!(created["active"], true);
    assert!(created["id"].is_string());
    assert!(
        created.get("password").is_none(),
        "password hash must never be returned"
    );

    // It shows up in the list (with the seed admin) and no hash leaks.
    let (st, list) = get(&app, "/api/v1/users", &admin).await;
    assert_eq!(st, StatusCode::OK);
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 2, "admin + cashier");
    for u in arr {
        assert!(u.get("password").is_none(), "no hash in list");
    }

    // The new cashier can actually log in with the chosen password.
    let (st, sess) = post_anon(
        &app,
        "/api/v1/login",
        json!({ "tenant": "acme", "email": "cajero@acme.cl", "password": "Cajero123" }),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "new user login: {sess}");
    assert!(sess["token"].is_string(), "got a JWT");
}

#[tokio::test]
async fn non_admin_cannot_manage_users() {
    let t = spawn_test_db().await;
    let (_tenant, cashier) = seed_user(&t.db, &jwt_cfg(), "acme", &["cashier"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (st, _) = get(&app, "/api/v1/users", &cashier).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "cashier cannot list users");

    let (st, _) = send(
        &app,
        "POST",
        "/api/v1/users",
        &cashier,
        json!({ "email": "x@acme.cl", "password": "Whatever1", "roles": ["cashier"] }),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "cashier cannot create users");
}

#[tokio::test]
async fn duplicate_email_conflicts() {
    let t = spawn_test_db().await;
    let (_tenant, admin) = seed_user(&t.db, &jwt_cfg(), "acme", &["admin"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let body = json!({ "email": "dup@acme.cl", "password": "Cajero123", "roles": ["cashier"] });
    let (st, _) = send(&app, "POST", "/api/v1/users", &admin, body.clone()).await;
    assert_eq!(st, StatusCode::OK);

    let (st, err) = send(&app, "POST", "/api/v1/users", &admin, body).await;
    assert_eq!(st, StatusCode::CONFLICT, "duplicate email rejected: {err}");
}

#[tokio::test]
async fn weak_password_and_bad_role_rejected() {
    let t = spawn_test_db().await;
    let (_tenant, admin) = seed_user(&t.db, &jwt_cfg(), "acme", &["admin"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (st, _) = send(
        &app,
        "POST",
        "/api/v1/users",
        &admin,
        json!({ "email": "a@acme.cl", "password": "short", "roles": ["cashier"] }),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "password <8 rejected");

    let (st, _) = send(
        &app,
        "POST",
        "/api/v1/users",
        &admin,
        json!({ "email": "b@acme.cl", "password": "GoodPass1", "roles": ["wizard"] }),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "unknown role rejected");

    let (st, _) = send(
        &app,
        "POST",
        "/api/v1/users",
        &admin,
        json!({ "email": "c@acme.cl", "password": "GoodPass1", "roles": [] }),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "no roles rejected");
}

#[tokio::test]
async fn cross_tenant_access_is_404() {
    let t = spawn_test_db().await;
    let jwt = jwt_cfg();
    let (_a, admin_a) = seed_user(&t.db, &jwt, "acme", &["admin"]).await;
    let (_b, admin_b) = seed_user(&t.db, &jwt, "beta", &["admin"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    // A creates a user.
    let (st, created) = send(
        &app,
        "POST",
        "/api/v1/users",
        &admin_a,
        json!({ "email": "only-a@acme.cl", "password": "Cajero123", "roles": ["cashier"] }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let uid = created["id"].as_str().unwrap().to_string();

    // B cannot see A's user in their list…
    let (_, list_b) = get(&app, "/api/v1/users", &admin_b).await;
    assert!(
        !list_b
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u["email"] == "only-a@acme.cl"),
        "B must not see A's users"
    );

    // …nor patch it (404, not 403 — don't leak existence).
    let (st, _) = send(
        &app,
        "PATCH",
        &format!("/api/v1/users/{uid}"),
        &admin_b,
        json!({ "active": false }),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND, "cross-tenant patch → 404");
}

#[tokio::test]
async fn self_deactivate_blocked() {
    let t = spawn_test_db().await;
    let (_tenant, admin) = seed_user(&t.db, &jwt_cfg(), "acme", &["admin"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (_, list) = get(&app, "/api/v1/users", &admin).await;
    let self_id = id_of_email(&list, "u@acme.cl");

    let (st, err) = send(
        &app,
        "PATCH",
        &format!("/api/v1/users/{self_id}"),
        &admin,
        json!({ "active": false }),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "cannot deactivate self: {err}");
}

#[tokio::test]
async fn last_admin_cannot_self_demote() {
    let t = spawn_test_db().await;
    let (_tenant, admin) = seed_user(&t.db, &jwt_cfg(), "acme", &["admin"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (_, list) = get(&app, "/api/v1/users", &admin).await;
    let self_id = id_of_email(&list, "u@acme.cl");

    // Demoting the only admin to cashier would lock the business out.
    let (st, err) = send(
        &app,
        "PATCH",
        &format!("/api/v1/users/{self_id}"),
        &admin,
        json!({ "roles": ["cashier"] }),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::CONFLICT,
        "last admin self-demote blocked: {err}"
    );

    // With a second admin present, demoting the first is allowed.
    let (st, _) = send(
        &app,
        "POST",
        "/api/v1/users",
        &admin,
        json!({ "email": "admin2@acme.cl", "password": "Admin2pass", "roles": ["admin"] }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, body) = send(
        &app,
        "PATCH",
        &format!("/api/v1/users/{self_id}"),
        &admin,
        json!({ "roles": ["pharmacist"] }),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "non-last admin demote ok: {body}");
    assert_eq!(body["roles"], json!(["pharmacist"]));
}

#[tokio::test]
async fn deactivated_user_cannot_login() {
    let t = spawn_test_db().await;
    let (_tenant, admin) = seed_user(&t.db, &jwt_cfg(), "acme", &["admin"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (st, created) = send(
        &app,
        "POST",
        "/api/v1/users",
        &admin,
        json!({ "email": "temp@acme.cl", "password": "Cajero123", "roles": ["cashier"] }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let uid = created["id"].as_str().unwrap().to_string();

    // Logs in while active.
    let (st, _) = post_anon(
        &app,
        "/api/v1/login",
        json!({ "tenant": "acme", "email": "temp@acme.cl", "password": "Cajero123" }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    // Deactivate → login now rejected.
    let (st, _) = send(
        &app,
        "PATCH",
        &format!("/api/v1/users/{uid}"),
        &admin,
        json!({ "active": false }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = post_anon(
        &app,
        "/api/v1/login",
        json!({ "tenant": "acme", "email": "temp@acme.cl", "password": "Cajero123" }),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED, "deactivated user blocked");
}

#[tokio::test]
async fn password_reset_changes_login() {
    let t = spawn_test_db().await;
    let (_tenant, admin) = seed_user(&t.db, &jwt_cfg(), "acme", &["admin"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    let (st, created) = send(
        &app,
        "POST",
        "/api/v1/users",
        &admin,
        json!({ "email": "reset@acme.cl", "password": "OldPass123", "roles": ["cashier"] }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let uid = created["id"].as_str().unwrap().to_string();

    // Reset the password.
    let (st, patched) = send(
        &app,
        "PATCH",
        &format!("/api/v1/users/{uid}"),
        &admin,
        json!({ "password": "NewPass456" }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert!(patched.get("password").is_none(), "no hash leaked on patch");

    // Old password no longer works; new one does.
    let (st, _) = post_anon(
        &app,
        "/api/v1/login",
        json!({ "tenant": "acme", "email": "reset@acme.cl", "password": "OldPass123" }),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED, "old password rejected");

    let (st, _) = post_anon(
        &app,
        "/api/v1/login",
        json!({ "tenant": "acme", "email": "reset@acme.cl", "password": "NewPass456" }),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "new password works");
}

//! Config center HTTP tests: sucursales (branch) + cajas (register) CRUD,
//! tenant isolation, role gating, and the `register.branch` ownership check.
//!
//! Real kv-surrealkv store (all migrations incl. 0032) + JWT, same harness as
//! `integration_db.rs`.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use pharma_core::config::{DbConfig, JwtConfig};
use serde_json::Value;
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

#[tokio::test]
async fn branch_and_register_crud_roundtrip() {
    let t = spawn_test_db().await;
    let (_tenant, token) = seed_user(&t.db, &jwt_cfg(), "acme", &["admin"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    // Create a sucursal.
    let (st, branch) = send(
        &app,
        "POST",
        "/api/v1/sucursales",
        &token,
        serde_json::json!({ "name": "  Centro  ", "comuna": "Coquimbo", "code": "" }),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "create branch: {branch}");
    assert_eq!(branch["name"], "Centro", "name trimmed");
    assert_eq!(branch["comuna"], "Coquimbo");
    assert!(branch["code"].is_null(), "empty code → null");
    assert_eq!(branch["active"], true);
    let branch_id = branch["id"].as_str().unwrap().to_string();

    // Create a caja linked to that sucursal.
    let (st, reg) = send(
        &app,
        "POST",
        "/api/v1/cajas",
        &token,
        serde_json::json!({ "name": "Caja 1", "branch": branch_id }),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "create register: {reg}");
    assert_eq!(reg["name"], "Caja 1");
    assert_eq!(reg["branch"], branch_id);
    let reg_id = reg["id"].as_str().unwrap().to_string();

    // List cajas filtered by branch.
    let (st, list) = get(&app, &format!("/api/v1/cajas?branch={branch_id}"), &token).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Patch the caja name.
    let (st, patched) = send(
        &app,
        "PATCH",
        &format!("/api/v1/cajas/{reg_id}"),
        &token,
        serde_json::json!({ "name": "Caja Principal" }),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "patch register: {patched}");
    assert_eq!(patched["name"], "Caja Principal");
    assert_eq!(
        patched["branch"], branch_id,
        "branch link preserved on partial patch"
    );

    // Soft-delete the caja → drops out of the default (active) list.
    let (st, _) = send(
        &app,
        "DELETE",
        &format!("/api/v1/cajas/{reg_id}"),
        &token,
        Value::Null,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (_, active_list) = get(&app, "/api/v1/cajas", &token).await;
    assert_eq!(
        active_list.as_array().unwrap().len(),
        0,
        "inactive hidden by default"
    );
    let (_, all_list) = get(&app, "/api/v1/cajas?active=false", &token).await;
    assert_eq!(
        all_list.as_array().unwrap().len(),
        1,
        "active=false reveals it"
    );
}

#[tokio::test]
async fn tenant_isolation_branches_and_registers() {
    let t = spawn_test_db().await;
    let jwt = jwt_cfg();
    let (_a, token_a) = seed_user(&t.db, &jwt, "acme", &["admin"]).await;
    let (_b, token_b) = seed_user(&t.db, &jwt, "beta", &["admin"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    // Tenant A creates a sucursal.
    let (st, branch) = send(
        &app,
        "POST",
        "/api/v1/sucursales",
        &token_a,
        serde_json::json!({ "name": "Sucursal A" }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let branch_a = branch["id"].as_str().unwrap().to_string();

    // Tenant B cannot see it in their list…
    let (_, list_b) = get(&app, "/api/v1/sucursales", &token_b).await;
    assert_eq!(
        list_b.as_array().unwrap().len(),
        0,
        "B sees none of A's branches"
    );

    // …nor fetch it by id (404, not another tenant's data).
    let (st, _) = get(&app, &format!("/api/v1/sucursales/{branch_a}"), &token_b).await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // …nor link a caja to A's branch (ownership check → 404).
    let (st, _) = send(
        &app,
        "POST",
        "/api/v1/cajas",
        &token_b,
        serde_json::json!({ "name": "Caja B", "branch": branch_a }),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::NOT_FOUND,
        "cross-tenant branch link rejected"
    );

    // …nor patch A's branch.
    let (st, _) = send(
        &app,
        "PATCH",
        &format!("/api/v1/sucursales/{branch_a}"),
        &token_b,
        serde_json::json!({ "name": "hijacked" }),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cashier_cannot_mutate_but_can_read() {
    let t = spawn_test_db().await;
    let jwt = jwt_cfg();
    let (_tenant, admin) = seed_user(&t.db, &jwt, "acme", &["admin"]).await;
    let (_t2, cashier) = seed_user(&t.db, &jwt, "acme2", &["cashier"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    // Cashier read is allowed (POS lists cajas).
    let (st, _) = get(&app, "/api/v1/cajas", &cashier).await;
    assert_eq!(st, StatusCode::OK, "cashier may read cajas");

    // Cashier create is forbidden (config is back-office).
    let (st, _) = send(
        &app,
        "POST",
        "/api/v1/sucursales",
        &cashier,
        serde_json::json!({ "name": "nope" }),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "cashier cannot create branches");

    // Admin create works.
    let (st, _) = send(
        &app,
        "POST",
        "/api/v1/sucursales",
        &admin,
        serde_json::json!({ "name": "ok" }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
}

#[tokio::test]
async fn rejects_empty_name_and_bad_branch_id() {
    let t = spawn_test_db().await;
    let (_tenant, token) = seed_user(&t.db, &jwt_cfg(), "acme", &["admin"]).await;
    let app = api::build_router(state_with_db(t.db.clone()));

    // Empty sucursal name → 400.
    let (st, _) = send(
        &app,
        "POST",
        "/api/v1/sucursales",
        &token,
        serde_json::json!({ "name": "   " }),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "empty name rejected");

    // Caja with a non-branch id → 400 (invalid), not 404.
    let (st, _) = send(
        &app,
        "POST",
        "/api/v1/cajas",
        &token,
        serde_json::json!({ "name": "Caja", "branch": "product:abc" }),
    )
    .await;
    assert_eq!(
        st,
        StatusCode::BAD_REQUEST,
        "non-branch id rejected as invalid"
    );
}

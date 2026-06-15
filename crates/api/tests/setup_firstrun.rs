//! First-run setup against a FRESH DB — the real install-day journey, driven
//! over HTTP end-to-end: empty install → create the first account in-app →
//! logged straight in → seed demo → catalog populated (first-sale data path
//! reachable). Proves the operator never needs the CLI to get from a blank
//! install to a working app.
//!
//! Also pins the fail-closed guard: once an account exists, status flips and a
//! second setup is rejected (not a backdoor to mint extra owners).

mod e2e_common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

use e2e_common::{spawn_db, state_free};

/// Build a fresh router over the same state (oneshot consumes the router).
fn app(db: &Arc<db::Db>) -> axum::Router {
    api::build_router(state_free(db.clone()))
}

async fn body_json(res: axum::response::Response) -> serde_json::Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

fn post(uri: &str, json: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&json).unwrap()))
        .unwrap()
}

fn get_bearer(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn fresh_install_full_first_run_without_cli() {
    let tdb = spawn_db().await;
    let db = tdb.db;

    // 1. Fresh DB → setup is needed.
    let res = app(&db)
        .oneshot(
            Request::builder()
                .uri("/api/v1/setup/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["needs_setup"], true);

    // 2. Create the first account in-app (no CLI).
    let res = app(&db)
        .oneshot(post(
            "/api/v1/setup",
            serde_json::json!({
                "business_name": "Mi Almacén Don José",
                "email": "Dueno@MiNegocio.cl",
                "password": "clave-segura-1",
                "vertical": "minimarket",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let setup = body_json(res).await;
    let token = setup["token"].as_str().expect("token").to_string();
    assert_eq!(setup["token_type"], "Bearer");
    // Slug derived from the business name (login "Sucursal" pre-fill).
    assert_eq!(setup["tenant_slug"], "mi-almacen-don-jose");

    // 3. The returned token is a real session: /me resolves owner identity.
    let res = app(&db)
        .oneshot(get_bearer("/api/v1/me", &token))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let me = body_json(res).await;
    let roles: Vec<String> = me["roles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap().to_string())
        .collect();
    assert!(roles.contains(&"owner".to_string()));
    assert!(roles.contains(&"admin".to_string()));

    // 4. The chosen rubro was persisted (admin_setting business.vertical).
    let res = app(&db)
        .oneshot(get_bearer("/api/v1/settings/business.vertical", &token))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["value"], "minimarket");

    // 5. Fail-closed: status now says no setup needed.
    let res = app(&db)
        .oneshot(
            Request::builder()
                .uri("/api/v1/setup/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_json(res).await["needs_setup"], false);

    // 6. Fail-closed: a second setup is rejected (not a backdoor).
    let res = app(&db)
        .oneshot(post(
            "/api/v1/setup",
            serde_json::json!({
                "business_name": "Intruso",
                "email": "intruso@x.cl",
                "password": "otra-clave-1",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(body_json(res).await["error"]["code"], "SETUP_ALREADY_DONE");

    // 7. The operator can now log in with the credentials they just chose
    //    (email normalised to lowercase on the way in).
    let res = app(&db)
        .oneshot(post(
            "/api/v1/login",
            serde_json::json!({
                "tenant": "mi-almacen-don-jose",
                "email": "dueno@minegocio.cl",
                "password": "clave-segura-1",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body_json(res).await["token"].is_string());

    // 8. First-sale data path reachable: seed demo, then catalog is populated.
    let res = app(&db)
        .oneshot({
            let mut r = post(
                "/api/v1/admin/seed-demo",
                serde_json::json!({ "vertical": "minimarket" }),
            );
            r.headers_mut()
                .insert("authorization", format!("Bearer {token}").parse().unwrap());
            r
        })
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(body_json(res).await["products_created"].as_u64().unwrap() > 0);

    let res = app(&db)
        .oneshot(get_bearer("/api/v1/products?limit=5", &token))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(!body_json(res).await.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn setup_rejects_bad_input() {
    let tdb = spawn_db().await;
    let db = tdb.db;

    // short password
    let res = app(&db)
        .oneshot(post(
            "/api/v1/setup",
            serde_json::json!({
                "business_name": "X",
                "email": "a@b.cl",
                "password": "short",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // malformed email
    let res = app(&db)
        .oneshot(post(
            "/api/v1/setup",
            serde_json::json!({
                "business_name": "X",
                "email": "no-arroba",
                "password": "clave-segura-1",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // empty business name
    let res = app(&db)
        .oneshot(post(
            "/api/v1/setup",
            serde_json::json!({
                "business_name": "   ",
                "email": "a@b.cl",
                "password": "clave-segura-1",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Nothing was created — still needs setup.
    let res = app(&db)
        .oneshot(
            Request::builder()
                .uri("/api/v1/setup/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body_json(res).await["needs_setup"], true);
}

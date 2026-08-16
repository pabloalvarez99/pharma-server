//! `POST /api/v1/products/ensure` — producto simple de feria (nombre + precio).

mod e2e_common;

use axum::http::StatusCode;
use e2e_common::{req_json, seed_tenant_admin, spawn_db, state_free, token_for};

fn app(db: std::sync::Arc<db::Db>) -> axum::Router {
    api::build_router(state_free(db))
}

#[tokio::test]
async fn ensure_crea_y_get_ok() {
    let tdb = spawn_db().await;
    let db = tdb.db;
    let (tenant_id, user_id, roles) = seed_tenant_admin(&db, "feria-a", "a@feria.cl").await;
    let token = token_for(&user_id, &tenant_id, roles);
    let router = app(db);

    let (st, body) = req_json(
        &router,
        "POST",
        "/api/v1/products/ensure",
        Some(&token),
        Some(serde_json::json!({ "name": "Tomates", "price": "2000" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "ensure: {body}");
    let id = body["id"].as_str().expect("id");
    assert_eq!(body["name"], "Tomates");
    assert_eq!(body["price"], "2000");
    assert_eq!(body["physical_stock"], false);
    assert_eq!(body["stock"], 0);

    let (st, got) = req_json(
        &router,
        "GET",
        &format!("/api/v1/products/{id}"),
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "get: {got}");
    assert_eq!(got["id"], id);
    assert_eq!(got["name"], "Tomates");
}

#[tokio::test]
async fn segundo_ensure_mismo_nombre_mismo_id() {
    let tdb = spawn_db().await;
    let db = tdb.db;
    let (tenant_id, user_id, roles) = seed_tenant_admin(&db, "feria-b", "b@feria.cl").await;
    let token = token_for(&user_id, &tenant_id, roles);
    let router = app(db);

    let (st1, a) = req_json(
        &router,
        "POST",
        "/api/v1/products/ensure",
        Some(&token),
        Some(serde_json::json!({ "name": "Cilantro", "price": "500" })),
        &[],
    )
    .await;
    assert_eq!(st1, StatusCode::OK, "{a}");
    let (st2, b) = req_json(
        &router,
        "POST",
        "/api/v1/products/ensure",
        Some(&token),
        Some(serde_json::json!({ "name": "cilantro", "price": "9999" })),
        &[],
    )
    .await;
    assert_eq!(st2, StatusCode::OK, "{b}");
    assert_eq!(a["id"], b["id"]);
    assert_eq!(b["price"], "500");
}

#[tokio::test]
async fn sin_token_401() {
    let tdb = spawn_db().await;
    let router = app(tdb.db);
    let (st, _) = req_json(
        &router,
        "POST",
        "/api/v1/products/ensure",
        None,
        Some(serde_json::json!({ "name": "Tomates", "price": "2000" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

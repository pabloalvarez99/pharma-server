//! GET /api/v1/me debe nombrar el puesto (slug + name), no solo el JWT.
//!
//! Tras restaurar sesión o entrar con Google el teléfono pega `/me`; sin
//! estos campos la barra de Hoy y «Pídele a $nombre» quedan vacías.

mod e2e_common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

use e2e_common::{spawn_db, state_free};

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

async fn alta_marta(db: &Arc<db::Db>) -> String {
    let res = app(db)
        .oneshot(post(
            "/api/v1/alta",
            serde_json::json!({
                "business_name": "Huevos de Marta",
                "email": "marta@feria.cl",
                "password": "clave-segura-2",
                "vertical": "feria",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_json(res).await;
    assert_eq!(body["tenant_slug"], "huevos-de-marta");
    body["token"].as_str().expect("token").to_string()
}

#[tokio::test]
async fn me_nombra_el_puesto() {
    let tdb = spawn_db().await;
    let db = tdb.db;
    let token = alta_marta(&db).await;

    let res = app(&db)
        .oneshot(get_bearer("/api/v1/me", &token))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let me = body_json(res).await;

    assert_eq!(me["tenant_slug"], "huevos-de-marta");
    assert_eq!(me["tenant_name"], "Huevos de Marta");
    assert!(me["sub"].as_str().unwrap().len() > 3);
    assert!(me["tenant_id"].as_str().unwrap().starts_with("tenant:"));
    assert!(me["roles"].as_array().unwrap().iter().any(|r| r == "owner"));
    assert!(me["exp"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn me_sin_token_es_401() {
    let tdb = spawn_db().await;
    let db = tdb.db;
    let _ = alta_marta(&db).await;

    let res = app(&db)
        .oneshot(
            Request::builder()
                .uri("/api/v1/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn token_de_a_no_ve_nombre_de_b() {
    let tdb = spawn_db().await;
    let db = tdb.db;

    // Primer puesto (setup) — Juan.
    let res = app(&db)
        .oneshot(post(
            "/api/v1/setup",
            serde_json::json!({
                "business_name": "Puesto de Juan",
                "email": "juan@feria.cl",
                "password": "clave-segura-1",
                "vertical": "feria",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let juan_token = body_json(res).await["token"]
        .as_str()
        .expect("token juan")
        .to_string();

    // Segundo puesto en la misma nube — Marta.
    let marta_token = alta_marta(&db).await;

    let me_juan = body_json(
        app(&db)
            .oneshot(get_bearer("/api/v1/me", &juan_token))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(me_juan["tenant_slug"], "puesto-de-juan");
    assert_eq!(me_juan["tenant_name"], "Puesto de Juan");

    let me_marta = body_json(
        app(&db)
            .oneshot(get_bearer("/api/v1/me", &marta_token))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(me_marta["tenant_slug"], "huevos-de-marta");
    assert_eq!(me_marta["tenant_name"], "Huevos de Marta");

    assert_ne!(me_juan["tenant_id"], me_marta["tenant_id"]);
    assert_ne!(me_juan["tenant_name"], me_marta["tenant_name"]);
}

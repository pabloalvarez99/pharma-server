//! Alta pública de un negocio en una nube que **ya tiene** otros.
//!
//! `/api/v1/setup` se cierra para siempre cuando existe un usuario. Eso está
//! bien en un nodo de un solo puesto. En la nube compartida de feria el segundo
//! feriante no puede recibir 409 "este servidor ya tiene una cuenta": tiene
//! que poder crear *su* negocio, con otro slug y otro correo, sin admin y sin
//! clave de provisioning.
//!
//! `/api/v1/setup` no se toca. Este archivo pide `POST /api/v1/alta`.

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

async fn primer_negocio(db: &Arc<db::Db>) {
    let res = app(db)
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
}

#[tokio::test]
async fn el_segundo_feriante_crea_su_negocio_en_la_misma_nube() {
    let tdb = spawn_db().await;
    let db = tdb.db;
    primer_negocio(&db).await;

    let res = app(&db)
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
    let alta = body_json(res).await;
    let token = alta["token"].as_str().expect("token").to_string();
    assert_eq!(alta["tenant_slug"], "huevos-de-marta");

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

    // El primero sigue vivo: login con su slug.
    let res = app(&db)
        .oneshot(post(
            "/api/v1/login",
            serde_json::json!({
                "tenant": "puesto-de-juan",
                "email": "juan@feria.cl",
                "password": "clave-segura-1",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // setup sigue cerrado: no es un backdoor.
    let res = app(&db)
        .oneshot(post(
            "/api/v1/setup",
            serde_json::json!({
                "business_name": "Intruso",
                "email": "otro@x.cl",
                "password": "clave-segura-3",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn alta_rechaza_el_mismo_nombre_corto() {
    let tdb = spawn_db().await;
    let db = tdb.db;
    primer_negocio(&db).await;

    let res = app(&db)
        .oneshot(post(
            "/api/v1/alta",
            serde_json::json!({
                "business_name": "Puesto de Juan",
                "email": "otro@feria.cl",
                "password": "clave-segura-2",
                "vertical": "feria",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(body_json(res).await["error"]["code"], "SLUG_TOMADO");
}

#[tokio::test]
async fn alta_rechaza_el_mismo_correo_en_otro_puesto() {
    let tdb = spawn_db().await;
    let db = tdb.db;
    primer_negocio(&db).await;

    let res = app(&db)
        .oneshot(post(
            "/api/v1/alta",
            serde_json::json!({
                "business_name": "Otro puesto",
                "email": "juan@feria.cl",
                "password": "clave-segura-2",
                "vertical": "feria",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(body_json(res).await["error"]["code"], "CORREO_TOMADO");
}

#[tokio::test]
async fn alta_tambien_sirve_con_la_base_vacia() {
    let tdb = spawn_db().await;
    let db = tdb.db;

    let res = app(&db)
        .oneshot(post(
            "/api/v1/alta",
            serde_json::json!({
                "business_name": "Solo yo",
                "email": "yo@feria.cl",
                "password": "clave-segura-1",
                "vertical": "feria",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["tenant_slug"], "solo-yo");
}

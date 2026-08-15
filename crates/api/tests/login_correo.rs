//! En la nube la feriante no conoce el slug. Correo + clave alcanzan
//! cuando ese correo es de un solo puesto.

mod e2e_common;

use axum::{body::Body, http::Request, http::StatusCode};
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

#[tokio::test]
async fn entra_con_correo_y_clave_sin_nombre_corto() {
    let tdb = spawn_db().await;
    let db = tdb.db;
    let alta = app(&db)
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
    assert_eq!(alta.status(), StatusCode::OK);

    let res = app(&db)
        .oneshot(post(
            "/api/v1/login",
            serde_json::json!({
                "email": "marta@feria.cl",
                "password": "clave-segura-2",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_json(res).await;
    assert!(body["token"].as_str().unwrap().len() > 10);
    assert_eq!(body["tenant_slug"], "huevos-de-marta");
    assert_eq!(body["tenant_name"], "Huevos de Marta");
}

#[tokio::test]
async fn el_slug_sigue_sirviendo() {
    let tdb = spawn_db().await;
    let db = tdb.db;
    app(&db)
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

    let res = app(&db)
        .oneshot(post(
            "/api/v1/login",
            serde_json::json!({
                "tenant": "huevos-de-marta",
                "email": "marta@feria.cl",
                "password": "clave-segura-2",
            }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

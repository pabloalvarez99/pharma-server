//! **El gate del carril, del lado del server** (ADR-0022).
//!
//! Un despliegue que no configuró `PHARMA_GOOGLE_CLIENT_ID` se comporta
//! *exactamente* como antes de que este carril existiera: la ruta contesta 501
//! y no toca la base ni sale a la red. Es lo que permite mergear esto antes de
//! que existan las credenciales en la consola de Google.
//!
//! Es un binario aparte de `google_sign_in.rs` a propósito. La configuración
//! entra por variable de entorno, que es estado del **proceso**: dos tests que
//! la prenden y la apagan en el mismo binario se pisan según el orden en que el
//! runtime los agende, y el que falla no es el que tiene el bug. Un binario que
//! nunca la define no puede tener esa carrera.

mod e2e_common;

use axum::http::StatusCode;
use e2e_common::{req_json, seed_tenant_admin, spawn_db, state_free};
use serde_json::json;

const RUTA: &str = "/api/v1/auth/google";

/// Sin client id configurado, la ruta existe y contesta 501 — igual que el stub
/// que había antes. No es un 404 (la ruta está montada) ni un 500 (no es una
/// falla): es "esto todavía no está cableado acá".
#[tokio::test]
async fn sin_client_id_la_ruta_sigue_siendo_501() {
    assert!(
        std::env::var("PHARMA_GOOGLE_CLIENT_ID").is_err(),
        "este binario no puede tener client id configurado",
    );

    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;
    let app = api::build_router(state_free(tdb.db.clone()));

    let (status, _) = req_json(
        &app,
        "POST",
        RUTA,
        None,
        Some(json!({ "id_token": "lo-que-sea", "tenant": "puesto-rosa" })),
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
}

/// Y no liga a nadie por el camino. Si un 501 dejara rastro en la base, el día
/// que se configure el client id habría vínculos que nadie creó.
#[tokio::test]
async fn sin_client_id_no_se_liga_a_nadie() {
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;
    let app = api::build_router(state_free(tdb.db.clone()));

    let (_, _) = req_json(
        &app,
        "POST",
        RUTA,
        None,
        Some(json!({ "id_token": "lo-que-sea", "tenant": "puesto-rosa" })),
        &[],
    )
    .await;

    #[derive(serde::Deserialize)]
    struct Conteo {
        count: i64,
    }
    let mut q = tdb
        .db
        .query("SELECT count() FROM google_identity GROUP ALL")
        .await
        .expect("contar");
    let fila: Option<Conteo> = q.take(0).expect("decode");
    assert_eq!(
        fila.map(|c| c.count).unwrap_or(0),
        0,
        "un 501 no puede dejar vínculos atrás",
    );
}

/// El camino que sí funciona hoy sigue funcionando, que es de lo que se trata
/// todo el gate: correo y clave entran igual, con Google apagado.
#[tokio::test]
async fn con_google_apagado_se_entra_con_correo_y_clave() {
    let tdb = spawn_db().await;
    seed_tenant_admin(&tdb.db, "puesto-rosa", "rosa@gmail.com").await;
    let app = api::build_router(state_free(tdb.db.clone()));

    let (status, body) = req_json(
        &app,
        "POST",
        "/api/v1/login",
        None,
        // `seed_tenant_admin` siembra esta clave; no es una credencial de
        // ningún sistema real (Regla 3).
        Some(json!({ "tenant": "puesto-rosa", "email": "rosa@gmail.com", "password": "pw-x" })),
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["token"].as_str().is_some_and(|t| !t.is_empty()));
}

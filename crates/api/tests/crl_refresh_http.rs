//! Integration test del refresh CRL sobre HTTP real (ADR-0006).
//!
//! El unit test de `apply_crl_chain` (en `src/lib.rs`) inyecta el `fetch`, así
//! que NO ejercita el cliente reqwest, la construcción de URL
//! (`{base}/crl-v{n}.json`) ni la detección de 404. Acá levantamos un server
//! axum local que sirve una cadena firmada de CRLs y verificamos el camino
//! HTTP completo de `refresh_crl_once_with_keys`:
//! * recorre la cadena v1 → v2 sobre sockets reales,
//! * para limpio cuando el server responde 404 (cabeza de la cadena),
//! * persiste el cache local + idempotencia en una segunda pasada.
//!
//! Firma con un keypair efímero (`agent::Identity`) + tabla de claves inyectada
//! (la prod usa `refresh_crl_once`, que pasa las claves embebidas).

use std::collections::HashMap;
use std::sync::Arc;

use agent::canonical::canonical_bytes;
use agent::identity::Identity;
use axum::{extract::Path, http::StatusCode, response::IntoResponse, routing::get, Router};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Value};

const KEY_ID: &str = "lk-http-test";

/// Firma un `crl-v{N}.json` (canonical-JSON sin `signature` → Ed25519 → b64),
/// mismo esquema que el licenser real.
fn sign_crl(id: &Identity, version: u64, prev: Option<u64>, added: &[&str]) -> Vec<u8> {
    let mut v = json!({
        "schema_version": 1,
        "crl_version": version,
        "previous_version": prev,
        "published_at": "2026-06-13T00:00:00Z",
        "diff": {
            "added": added.iter().map(|l| json!({
                "license_id": l,
                "revoked_at": "2026-06-12T00:00:00Z",
            })).collect::<Vec<_>>(),
            "removed": [],
        },
        "issuer_did": id.did(),
        "key_id": KEY_ID,
        "signature": "",
    });
    let unsigned = {
        let mut u = v.clone();
        u.as_object_mut().unwrap().remove("signature");
        canonical_bytes(&u).unwrap()
    };
    let sig = id.sign(&unsigned);
    v["signature"] = Value::String(B64.encode(sig.to_bytes()));
    serde_json::to_vec(&v).unwrap()
}

/// Server local que sirve `/crl/crl-v{n}.json` desde un mapa; 404 para el resto.
async fn spawn_cdn(cdn: HashMap<String, Vec<u8>>) -> String {
    let cdn = Arc::new(cdn);
    let app = Router::new().route(
        "/crl/{file}",
        get(move |Path(file): Path<String>| {
            let cdn = cdn.clone();
            async move {
                match cdn.get(&file) {
                    Some(bytes) => (StatusCode::OK, bytes.clone()).into_response(),
                    None => StatusCode::NOT_FOUND.into_response(),
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}/crl")
}

#[tokio::test]
async fn refresh_walks_signed_chain_over_http_and_stops_on_404() {
    let id = Identity::generate();
    let did = id.did();
    let keys: &[(&str, &str)] = &[(KEY_ID, &did)];

    let mut cdn = HashMap::new();
    cdn.insert(
        "crl-v1.json".to_string(),
        sign_crl(&id, 1, None, &["lic_x"]),
    );
    cdn.insert(
        "crl-v2.json".to_string(),
        sign_crl(&id, 2, Some(1), &["lic_y"]),
    );
    let base = spawn_cdn(cdn).await;

    let dir = tempfile::tempdir().unwrap();

    // Primera pasada: GET crl-v1 + crl-v2 (200), crl-v3 (404) ⇒ aplica 2.
    let applied = api::refresh_crl_once_with_keys(&base, dir.path(), keys)
        .await
        .expect("refresh over http");
    assert_eq!(applied, 2, "debe aplicar v1 + v2 y parar en el 404 de v3");

    let state = license::load_crl_state(&license::default_crl_state_path(dir.path())).unwrap();
    assert_eq!(state.last_seen_version, 2);
    assert!(state.is_revoked("lic_x") && state.is_revoked("lic_y"));

    // Segunda pasada: crl-v3 sigue 404 ⇒ noop idempotente.
    let again = api::refresh_crl_once_with_keys(&base, dir.path(), keys)
        .await
        .expect("idempotent refresh");
    assert_eq!(again, 0);
}

#[tokio::test]
async fn refresh_noop_when_cdn_has_no_crl_yet() {
    // Nodo fresco contra un CDN sin CRLs (todo 404) ⇒ 0 aplicadas, sin error.
    let id = Identity::generate();
    let did = id.did();
    let keys: &[(&str, &str)] = &[(KEY_ID, &did)];

    let base = spawn_cdn(HashMap::new()).await;
    let dir = tempfile::tempdir().unwrap();

    let applied = api::refresh_crl_once_with_keys(&base, dir.path(), keys)
        .await
        .expect("noop refresh");
    assert_eq!(applied, 0);
    // Sin nada aplicado, el cache no se escribe (queda en default al leer).
    let state = license::load_crl_state(&license::default_crl_state_path(dir.path())).unwrap();
    assert_eq!(state.last_seen_version, 0);
}

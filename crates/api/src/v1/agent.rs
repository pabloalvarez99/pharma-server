//! Agent federation transport (Fase 11 step 2).
//!
//! Node-to-node messaging — NOT tenant-scoped and NOT JWT-authenticated:
//! authenticity comes from the Ed25519 signature on the [`agent::Envelope`].
//! Any peer holding a valid keypair can reach `POST /agent/inbox`; the node
//! verifies the signature, records the interaction (local-only trust graph),
//! dispatches by `topic`, and replies with its own signed envelope.
//!
//! Topics implemented this step:
//! * `ping` → `pong` (liveness + identity proof).
//! * `catalog.lookup` → `catalog.match` — peer sends `{barcodes:[...]}`,
//!   node answers which it recognizes from the GLOBAL `barcode_catalog`
//!   (shared Chile vocabulary; no tenant data leaks).
//!
//! Reputation is appended to `agent_interaction` (node-level, never
//! centralized — see docs/ecosystem-roadmap.md).

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde_json::json;

use crate::AppState;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/agent/did", get(did))
        .route("/agent/inbox", axum::routing::post(inbox))
        .with_state(state)
}

async fn did(State(state): State<AppState>) -> impl IntoResponse {
    match &state.node_identity {
        Some(id) => (StatusCode::OK, Json(json!({ "did": id.did() }))).into_response(),
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "node identity not configured" })),
        )
            .into_response(),
    }
}

async fn record_interaction(
    state: &AppState,
    peer_did: &str,
    topic: &str,
    msg_id: &str,
    outcome: &str,
) {
    let Some(db) = state.db.clone() else { return };
    let _ = db
        .query("CREATE agent_interaction SET peer_did=$p, topic=$t, msg_id=$m, outcome=$o")
        .bind(("p", peer_did.to_string()))
        .bind(("t", topic.to_string()))
        .bind(("m", msg_id.to_string()))
        .bind(("o", outcome.to_string()))
        .await;
}

async fn inbox(State(state): State<AppState>, body: String) -> axum::response::Response {
    let Some(node) = state.node_identity.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "node identity not configured" })),
        )
            .into_response();
    };

    let env = match agent::Envelope::from_json(&body) {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("envelope inválido: {e}") })),
            )
                .into_response();
        }
    };

    // Authenticity: the signature over canonical(envelope sans sig) must
    // verify against the key embedded in `from`.
    if env.verify().is_err() {
        record_interaction(&state, &env.from, &env.topic, &env.msg_id, "rejected").await;
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "firma de envelope inválida" })),
        )
            .into_response();
    }

    // Optional addressing check: if `to` is set and is a DID, it must be ours.
    if env.to.starts_with("did:pharma:") && env.to != node.did() {
        record_interaction(&state, &env.from, &env.topic, &env.msg_id, "rejected").await;
        return (
            StatusCode::MISDIRECTED_REQUEST,
            Json(json!({ "error": "envelope dirigido a otro nodo" })),
        )
            .into_response();
    }

    let reply_body = match env.topic.as_str() {
        "ping" => json!({ "echo": env.msg_id }),
        "catalog.lookup" => match catalog_lookup(&state, &env.body).await {
            Ok(v) => v,
            Err(resp) => {
                record_interaction(&state, &env.from, &env.topic, &env.msg_id, "error").await;
                return resp;
            }
        },
        other => {
            record_interaction(&state, &env.from, &env.topic, &env.msg_id, "rejected").await;
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("topic no soportado: {other}") })),
            )
                .into_response();
        }
    };

    let reply_topic = match env.topic.as_str() {
        "ping" => "pong",
        "catalog.lookup" => "catalog.match",
        _ => "ack",
    };
    let reply = agent::Envelope::create(
        node.as_ref(),
        env.from.clone(),
        uuid::Uuid::new_v4().to_string(),
        reply_topic,
        reply_body,
    );
    record_interaction(&state, &env.from, &env.topic, &env.msg_id, "ok").await;

    match reply.to_json() {
        Ok(j) => (StatusCode::OK, [("content-type", "application/json")], j).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("serialización respuesta: {e}") })),
        )
            .into_response(),
    }
}

/// Resolve `{ "barcodes": ["..."] }` against the GLOBAL `barcode_catalog`.
/// Returns `{ "matches": [ { "barcode", "external_id" } ] }`. No tenant data.
async fn catalog_lookup(
    state: &AppState,
    body: &serde_json::Value,
) -> Result<serde_json::Value, axum::response::Response> {
    let barcodes: Vec<String> = body
        .get("barcodes")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if barcodes.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "body.barcodes requerido (array no vacío)" })),
        )
            .into_response());
    }
    let Some(db) = state.db.clone() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "db no disponible" })),
        )
            .into_response());
    };

    let mut matches = Vec::new();
    let mut r = match db
        .query("SELECT barcode, external_id FROM barcode_catalog WHERE barcode IN $codes")
        .bind(("codes", barcodes.clone()))
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("consulta catálogo: {e}") })),
            )
                .into_response())
        }
    };

    #[derive(serde::Deserialize)]
    struct Row {
        barcode: String,
        external_id: String,
    }
    let rows: Vec<Row> = r.take(0).unwrap_or_default();
    for row in rows {
        matches.push(json!({ "barcode": row.barcode, "external_id": row.external_id }));
    }
    Ok(json!({ "matches": matches }))
}

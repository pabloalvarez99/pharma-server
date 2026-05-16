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
        "quote.request" => match quote_request(&state, &env.body).await {
            Ok(v) => v,
            Err(resp) => {
                record_interaction(&state, &env.from, &env.topic, &env.msg_id, "rejected").await;
                return resp;
            }
        },
        "po.create" => match po_create(&state, &env.from, &env.body).await {
            Ok(v) => v,
            Err(resp) => {
                record_interaction(&state, &env.from, &env.topic, &env.msg_id, "rejected").await;
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
        "quote.request" => "quote.response",
        "po.create" => "po.ack",
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

/// Resolve a tenant by slug and enforce the federation opt-in: the tenant
/// MUST have `admin_setting` key `federation_enabled` == "true". This is the
/// gate that keeps tenant pricing/stock private unless the operator opts in
/// (locked decision — see docs/ecosystem-roadmap.md).
async fn resolve_federation_tenant(
    state: &AppState,
    slug: &str,
) -> Result<(std::sync::Arc<db::Db>, surrealdb::sql::Thing), axum::response::Response> {
    let Some(db) = state.db.clone() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "db no disponible" })),
        )
            .into_response());
    };
    #[derive(serde::Deserialize)]
    struct TRow {
        id: surrealdb::sql::Thing,
    }
    let mut r = db
        .query("SELECT id FROM tenant WHERE slug = $s LIMIT 1")
        .bind(("s", slug.to_string()))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("lookup tenant: {e}") })),
            )
                .into_response()
        })?;
    let trow: Option<TRow> = r.take(0).unwrap_or(None);
    let Some(trow) = trow else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("tenant '{slug}' no existe") })),
        )
            .into_response());
    };
    let tenant = trow.id;

    #[derive(serde::Deserialize)]
    struct SRow {
        value: String,
    }
    let mut r2 = db
        .query(
            "SELECT * FROM admin_setting WHERE tenant = $t AND key = 'federation_enabled' LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("lookup setting: {e}") })),
            )
                .into_response()
        })?;
    let srow: Option<SRow> = r2.take(0).unwrap_or(None);
    let enabled = srow.map(|s| s.value == "true").unwrap_or(false);
    if !enabled {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": format!("tenant '{slug}' no habilitó federación (admin_setting federation_enabled)")
            })),
        )
            .into_response());
    }
    Ok((db, tenant))
}

/// `quote.request` → `quote.response`. Body:
/// `{ "tenant": "<slug>", "items": [ { "barcode": "..", "qty": N } ] }`.
/// Prices/stock come from the named tenant's `product` (joined via
/// `product_barcode`). CLP is integer pesos so `f64` is exact here.
async fn quote_request(
    state: &AppState,
    body: &serde_json::Value,
) -> Result<serde_json::Value, axum::response::Response> {
    let slug = body.get("tenant").and_then(|v| v.as_str()).unwrap_or("");
    if slug.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "body.tenant requerido" })),
        )
            .into_response());
    }
    let items = body
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if items.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "body.items requerido (array no vacío)" })),
        )
            .into_response());
    }
    let (db, tenant) = resolve_federation_tenant(state, slug).await?;

    #[derive(serde::Deserialize)]
    struct PRow {
        name: String,
        price: f64,
        stock: i64,
    }
    let mut lines = Vec::new();
    let mut total = 0_f64;
    for it in &items {
        let barcode = it.get("barcode").and_then(|v| v.as_str()).unwrap_or("");
        let qty = it.get("qty").and_then(|v| v.as_i64()).unwrap_or(0).max(0);
        if barcode.is_empty() || qty == 0 {
            continue;
        }
        let mut rb = db
            .query(
                "SELECT VALUE product FROM product_barcode \
                 WHERE tenant = $t AND barcode = $b LIMIT 1",
            )
            .bind(("t", tenant.clone()))
            .bind(("b", barcode.to_string()))
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("barcode lookup: {e}") })),
                )
                    .into_response()
            })?;
        let pids: Vec<surrealdb::sql::Thing> = rb.take(0).unwrap_or_default();
        let pid = pids.into_iter().next();
        let prow: Option<PRow> = match pid {
            None => None,
            Some(pid) => {
                let mut r = db
                    .query(
                        "SELECT name, <float> price AS price, stock FROM product \
                         WHERE id = $p AND tenant = $t AND active = true LIMIT 1",
                    )
                    .bind(("p", pid))
                    .bind(("t", tenant.clone()))
                    .await
                    .map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({ "error": format!("quote lookup: {e}") })),
                        )
                            .into_response()
                    })?;
                r.take(0).unwrap_or(None)
            }
        };
        match prow {
            Some(p) => {
                let line_total = p.price * qty as f64;
                total += line_total;
                lines.push(json!({
                    "barcode": barcode,
                    "product_name": p.name,
                    "unit_price": p.price,
                    "qty": qty,
                    "available": p.stock,
                    "line_total": line_total,
                    "in_stock": p.stock >= qty,
                }));
            }
            None => lines.push(json!({
                "barcode": barcode,
                "qty": qty,
                "found": false,
            })),
        }
    }
    Ok(json!({
        "tenant": slug,
        "currency": "CLP",
        "lines": lines,
        "total": total,
    }))
}

/// `po.create` → `po.ack`. Body:
/// `{ "tenant": "<slug>", "lines": [ { "barcode", "qty", "unit_price" } ],
///    "buyer_note"? }`. Records an inbound `agent_order` (status=received)
/// and returns its id. Fulfillment/settlement is a later step; this closes
/// the offer→order handshake so two nodes can transact e2e.
async fn po_create(
    state: &AppState,
    peer_did: &str,
    body: &serde_json::Value,
) -> Result<serde_json::Value, axum::response::Response> {
    let slug = body.get("tenant").and_then(|v| v.as_str()).unwrap_or("");
    if slug.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "body.tenant requerido" })),
        )
            .into_response());
    }
    let lines = body
        .get("lines")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if lines.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "body.lines requerido (array no vacío)" })),
        )
            .into_response());
    }
    let (db, tenant) = resolve_federation_tenant(state, slug).await?;

    let mut total = 0_f64;
    for l in &lines {
        let qty = l.get("qty").and_then(|v| v.as_i64()).unwrap_or(0).max(0);
        let up = l.get("unit_price").and_then(|v| v.as_f64()).unwrap_or(0.0);
        total += up * qty as f64;
    }
    let buyer_note = body
        .get("buyer_note")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let lines_json = serde_json::to_string(&lines).unwrap_or_else(|_| "[]".into());

    #[derive(serde::Deserialize)]
    struct ORow {
        id: surrealdb::sql::Thing,
    }
    let mut r = db
        .query(
            "CREATE agent_order SET tenant=$t, peer_did=$p, lines_json=$l, \
             total=$tot, status='received', buyer_note=$note RETURN AFTER",
        )
        .bind(("t", tenant))
        .bind(("p", peer_did.to_string()))
        .bind(("l", lines_json))
        .bind(("tot", total))
        .bind(("note", buyer_note))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("crear orden: {e}") })),
            )
                .into_response()
        })?;
    let orow: Option<ORow> = r.take(0).unwrap_or(None);
    let order_id = orow.map(|o| o.id.to_string()).ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "orden no creada" })),
        )
            .into_response()
    })?;
    Ok(json!({
        "order_id": order_id,
        "status": "received",
        "currency": "CLP",
        "total": total,
    }))
}

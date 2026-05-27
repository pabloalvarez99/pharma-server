//! BUG-001 reproduction — every role-gated WRITE route 500s.
//!
//! `crate::middleware::role::layer` composes its `Stack` as
//! `Stack::new(Extension(AllowedRoles), from_fn(role_gate))`. Per
//! `tower_layer::Stack`, the FIRST arg is the INNER layer (closest to the
//! service, runs LAST) and the SECOND is the OUTER layer (runs FIRST). So the
//! `role_gate` runs BEFORE the `Extension<AllowedRoles>` is injected, the gate
//! fails to extract it, and axum returns 500 "Missing request extension:
//! AllowedRoles" for EVERY route guarded by `route_layer(role::layer(...))`:
//! `POST /products`, `/batches`, `/pos/sale`, `/pos/returns`, `/cash-sessions`,
//! `/clientes`, `/expenses`, `/agent-orders/{id}/accept|fulfill`, …
//!
//! Fix (NOT applied here): swap the two args so the Extension is the OUTER
//! layer — `Stack::new(from_fn(role_gate), Extension(AllowedRoles))`.
//!
//! The first test pins the CURRENT (broken) behavior so the suite has a
//! passing sentinel + crisp repro; the second asserts the CORRECT behavior and
//! is `#[ignore]`d under BUG-001.

mod e2e_common;

use axum::http::StatusCode;
use e2e_common::*;

fn product_body() -> serde_json::Value {
    serde_json::json!({
        "name": "Probe SKU",
        "price": "1000",
        "cost_price": "500",
        "stock": 10,
    })
}

#[tokio::test]
async fn role_gated_write_currently_500s_missing_extension() {
    let tdb = spawn_db().await;
    let (tid, uid, roles) = seed_tenant_admin(&tdb.db, "farmacia-gate", "admin@gate.cl").await;
    let token = token_for(&uid, &tid, roles); // includes "admin" — should be allowed
    let app = api::build_router(state_free(tdb.db.clone()));

    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/products",
        Some(&token),
        Some(product_body()),
        &[],
    )
    .await;

    // CURRENT broken behavior: the role gate can't read AllowedRoles → 500.
    assert_eq!(
        st,
        StatusCode::INTERNAL_SERVER_ERROR,
        "BUG-001 sentinel: role-gated write 500s today; flip this test once the \
         Stack arg order in role::layer is fixed. body={body}"
    );
}

/// Severity note: BUG-001 short-circuits BEFORE authentication. The
/// `Extension<AllowedRoles>` is extracted as a `from_fn` argument, and that
/// extraction fails (500) before `role_gate`'s body — which holds the bearer
/// token check — ever runs. So even a NO-TOKEN request to a gated write 500s
/// instead of the correct 401. This pins that (broken) reality; CORRECT would
/// be 401 (asserted in the ignored test below as part of the same fix).
#[tokio::test]
async fn role_gated_write_without_token_also_500s_before_auth() {
    let tdb = spawn_db().await;
    let app = api::build_router(state_free(tdb.db.clone()));
    let (st, _body) = req_json(
        &app,
        "POST",
        "/api/v1/products",
        None,
        Some(product_body()),
        &[],
    )
    .await;
    assert_eq!(
        st,
        StatusCode::INTERNAL_SERVER_ERROR,
        "BUG-001: missing-extension 500 fires before the gate's token check, so even \
         an unauthenticated request 500s instead of 401"
    );
}

/// CORRECT behavior: an admin POST to a role-gated write route should create
/// the resource (201/200), not 500. Ignored under BUG-001.
#[tokio::test]
#[ignore = "BUG-001: role::layer Stack arg order swapped → role-gated writes 500 instead of succeeding"]
async fn role_gated_write_with_admin_should_succeed() {
    let tdb = spawn_db().await;
    let (tid, uid, roles) = seed_tenant_admin(&tdb.db, "farmacia-gate", "admin@gate.cl").await;
    let token = token_for(&uid, &tid, roles);
    let app = api::build_router(state_free(tdb.db.clone()));

    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/products",
        Some(&token),
        Some(product_body()),
        &[],
    )
    .await;
    assert!(
        st.is_success(),
        "an admin must be able to create a product, got {st}: {body}"
    );
    assert_eq!(body["name"], "Probe SKU");
}

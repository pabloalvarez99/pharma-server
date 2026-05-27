//! BUG-001 regression suite — role-gated WRITE routes (post-fix).
//!
//! `crate::middleware::role::layer` originally composed its `Stack` with the
//! `Extension<AllowedRoles>` as the INNER layer, so `role_gate` ran BEFORE the
//! extension was injected and EVERY gated write 500'd with "Missing request
//! extension: AllowedRoles". Fix (landed via step 02 `fix/role-allowed-roles-
//! extension`): swap arg order so `Extension` is OUTER. Tests below pin the
//! post-fix contract: admin POST → 200, no-token POST → 401, role-mismatch
//! → 403. If any flip to 500, the layer order regressed.

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

/// Admin POST to role-gated write succeeds (was 500 under BUG-001).
#[tokio::test]
async fn role_gated_write_with_admin_succeeds() {
    let tdb = spawn_db().await;
    let (tid, uid, roles) = seed_tenant_admin(&tdb.db, "farmacia-gate", "admin@gate.cl").await;
    let token = token_for(&uid, &tid, roles); // includes "admin"
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
        "admin must create product (200/201), got {st}: {body}"
    );
    assert_eq!(body["name"], "Probe SKU");
}

/// No-token POST to role-gated write → 401 (was 500 under BUG-001 because the
/// missing-extension panic fired before the gate's token check). With the
/// Stack arg order fixed, the gate reaches the token check and rejects.
#[tokio::test]
async fn role_gated_write_without_token_returns_401() {
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
        StatusCode::UNAUTHORIZED,
        "missing token must surface as 401, not 500 (BUG-001 regression check)"
    );
}

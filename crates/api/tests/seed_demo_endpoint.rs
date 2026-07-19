//! `POST /api/v1/admin/seed-demo` — admin gate + delegate to domain::seed.
//! The seed business logic + stock-ledger invariant are covered in
//! `crates/domain/tests/seed.rs`; here we lock the HTTP contract: admin-only,
//! tenant-from-JWT, summary body, and the 409-on-re-seed / force semantics.

mod e2e_common;

use axum::http::StatusCode;
use e2e_common::{req_json, seed_tenant_admin, spawn_db, state_free, token_for};

// Build the full app from a Free-tier state over the test db (seeding is core,
// not tier-gated).
fn build_app_for_(db: std::sync::Arc<db::Db>) -> axum::Router {
    api::build_router(state_free(db))
}

#[tokio::test]
async fn non_admin_is_forbidden() {
    let t = spawn_db().await;
    let (tid, uid, _roles) = seed_tenant_admin(&t.db, "farmacia-uno", "a@f.cl").await;
    let app = build_app_for_(t.db.clone());
    // Cashier-only token (not admin/owner).
    let token = token_for(&uid, &tid, vec!["cashier".into()]);
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/admin/seed-demo",
        Some(&token),
        Some(serde_json::json!({ "vertical": "pharmacy" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_seeds_then_reseed_conflicts_then_force_ok() {
    let t = spawn_db().await;
    let (tid, uid, roles) = seed_tenant_admin(&t.db, "farmacia-dos", "b@f.cl").await;
    let app = build_app_for_(t.db.clone());
    let token = token_for(&uid, &tid, roles);

    // First seed → 200 + summary.
    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/admin/seed-demo",
        Some(&token),
        Some(serde_json::json!({ "vertical": "pharmacy", "force": false })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "body={body}");
    assert_eq!(body["vertical"], "pharmacy");
    assert!(body["products_created"].as_u64().unwrap() >= 5);
    assert_eq!(body["wiped"].as_u64().unwrap(), 0);

    // Second seed without force → 409.
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/admin/seed-demo",
        Some(&token),
        Some(serde_json::json!({ "vertical": "pharmacy", "force": false })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);

    // With force → 200 + wiped == prior pack size.
    let (st, body) = req_json(
        &app,
        "POST",
        "/api/v1/admin/seed-demo",
        Some(&token),
        Some(serde_json::json!({ "vertical": "minimarket", "force": true })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "body={body}");
    assert_eq!(body["vertical"], "minimarket");
    assert!(body["wiped"].as_u64().unwrap() >= 5);
}

#[tokio::test]
async fn unauthenticated_is_rejected() {
    let t = spawn_db().await;
    let app = build_app_for_(t.db.clone());
    let (st, _b) = req_json(
        &app,
        "POST",
        "/api/v1/admin/seed-demo",
        None,
        Some(serde_json::json!({})),
        &[],
    )
    .await;
    assert!(st == StatusCode::UNAUTHORIZED || st == StatusCode::FORBIDDEN);
}

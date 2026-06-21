//! Config Center backend — user/role management + payment methods + business
//! profile. Closes the CLI→UI gap: today creating users/roles only exists in
//! `pharma user-create`, so the owner can't manage their team without a
//! terminal. All mutations are admin/owner-gated and tenant-scoped; reads of
//! payment-methods/business are open to cashier+ (the POS needs the accepted
//! methods + the receipt header).

mod e2e_common;

use axum::http::StatusCode;
use e2e_common::*;

/// Seed a user row directly (bypasses the API) for owner-lockout fixtures.
async fn seed_user_raw(
    db: &db::Db,
    tenant_id: &str,
    email: &str,
    roles: &[&str],
    active: bool,
) -> String {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let t = tid_thing(tenant_id);
    let hash = auth::password::hash("pw-seed-1").expect("hash");
    let roles_v: Vec<String> = roles.iter().map(|s| s.to_string()).collect();
    let mut q = db
        .query("CREATE user SET tenant=$t, email=$e, password=$p, roles=$r, active=$a RETURN AFTER")
        .bind(("t", t))
        .bind(("e", email.to_string()))
        .bind(("p", hash))
        .bind(("r", roles_v))
        .bind(("a", active))
        .await
        .expect("seed user");
    q.take::<Option<Row>>(0)
        .expect("decode")
        .expect("user row")
        .id
        .to_string()
}

// ---------------------------------------------------------------------------
// Users — list + create
// ---------------------------------------------------------------------------

#[tokio::test]
async fn admin_lists_and_creates_users() {
    let tdb = spawn_db().await;
    let (tid, uid, _r) = seed_tenant_admin(&tdb.db, "farma", "admin@farma.cl").await;
    let token = token_for(&uid, &tid, vec!["admin".into()]);
    let app = api::build_router(state_free(tdb.db.clone()));

    // List starts with the seeded admin only; the hash is NEVER returned.
    let (st, list) = req_json(&app, "GET", "/api/v1/config/users", Some(&token), None, &[]).await;
    assert_eq!(st, StatusCode::OK, "{list}");
    let arr = list.as_array().expect("array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["email"], "admin@farma.cl");
    assert_eq!(arr[0]["active"], true);
    assert!(arr[0].get("password").is_none(), "password must never leak");

    // Create a cashier.
    let body = serde_json::json!({ "email": "caja@farma.cl", "password": "secreto8x", "roles": ["cashier"] });
    let (st, created) = req_json(
        &app,
        "POST",
        "/api/v1/config/users",
        Some(&token),
        Some(body),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{created}");
    assert_eq!(created["email"], "caja@farma.cl");
    assert_eq!(created["roles"][0], "cashier");
    assert_eq!(created["active"], true);
    assert!(created.get("password").is_none());
    assert!(created["id"].as_str().unwrap().starts_with("user:"));

    let (_st, list) = req_json(&app, "GET", "/api/v1/config/users", Some(&token), None, &[]).await;
    assert_eq!(list.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn cashier_cannot_manage_users() {
    let tdb = spawn_db().await;
    let (tid, uid, _r) = seed_tenant_admin(&tdb.db, "farma", "admin@farma.cl").await;
    let token = token_for(&uid, &tid, vec!["cashier".into()]);
    let app = api::build_router(state_free(tdb.db.clone()));

    let (st, _) = req_json(&app, "GET", "/api/v1/config/users", Some(&token), None, &[]).await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    let body =
        serde_json::json!({ "email": "x@y.cl", "password": "secreto8x", "roles": ["cashier"] });
    let (st, _) = req_json(
        &app,
        "POST",
        "/api/v1/config/users",
        Some(&token),
        Some(body),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn create_user_validation() {
    let tdb = spawn_db().await;
    let (tid, uid, _r) = seed_tenant_admin(&tdb.db, "farma", "admin@farma.cl").await;
    let token = token_for(&uid, &tid, vec!["admin".into()]);
    let app = api::build_router(state_free(tdb.db.clone()));

    let cases = [
        serde_json::json!({ "email": "no-arroba", "password": "secreto8x", "roles": ["cashier"] }),
        serde_json::json!({ "email": "a@b.cl", "password": "corta", "roles": ["cashier"] }),
        serde_json::json!({ "email": "a@b.cl", "password": "secreto8x", "roles": ["superuser"] }),
        serde_json::json!({ "email": "a@b.cl", "password": "secreto8x", "roles": [] }),
    ];
    for body in cases {
        let (st, j) = req_json(
            &app,
            "POST",
            "/api/v1/config/users",
            Some(&token),
            Some(body.clone()),
            &[],
        )
        .await;
        assert_eq!(
            st,
            StatusCode::BAD_REQUEST,
            "expected 400 for {body}, got {st} {j}"
        );
    }
}

#[tokio::test]
async fn duplicate_email_conflicts() {
    let tdb = spawn_db().await;
    let (tid, uid, _r) = seed_tenant_admin(&tdb.db, "farma", "admin@farma.cl").await;
    let token = token_for(&uid, &tid, vec!["admin".into()]);
    let app = api::build_router(state_free(tdb.db.clone()));

    let body = serde_json::json!({ "email": "dup@farma.cl", "password": "secreto8x", "roles": ["cashier"] });
    let (st, _) = req_json(
        &app,
        "POST",
        "/api/v1/config/users",
        Some(&token),
        Some(body.clone()),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let (st, _) = req_json(
        &app,
        "POST",
        "/api/v1/config/users",
        Some(&token),
        Some(body),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
}

// ---------------------------------------------------------------------------
// Users — edit roles + disable/enable
// ---------------------------------------------------------------------------

#[tokio::test]
async fn edit_user_roles() {
    let tdb = spawn_db().await;
    let (tid, uid, _r) = seed_tenant_admin(&tdb.db, "farma", "admin@farma.cl").await;
    let token = token_for(&uid, &tid, vec!["admin".into()]);
    let app = api::build_router(state_free(tdb.db.clone()));

    let new_id = seed_user_raw(&tdb.db, &tid, "edita@farma.cl", &["cashier"], true).await;
    let (st, body) = req_json(
        &app,
        "PATCH",
        &format!("/api/v1/config/users/{new_id}"),
        Some(&token),
        Some(serde_json::json!({ "roles": ["pharmacist", "cashier"] })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let roles = body["roles"].as_array().unwrap();
    assert!(roles.iter().any(|v| v == "pharmacist"));
    assert!(roles.iter().any(|v| v == "cashier"));
}

#[tokio::test]
async fn disable_and_enable_user() {
    let tdb = spawn_db().await;
    let (tid, uid, _r) = seed_tenant_admin(&tdb.db, "farma", "admin@farma.cl").await;
    let token = token_for(&uid, &tid, vec!["admin".into()]);
    let app = api::build_router(state_free(tdb.db.clone()));

    let id = seed_user_raw(&tdb.db, &tid, "temp@farma.cl", &["cashier"], true).await;

    let (st, body) = req_json(
        &app,
        "POST",
        &format!("/api/v1/config/users/{id}/disable"),
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["active"], false);

    let (st, body) = req_json(
        &app,
        "POST",
        &format!("/api/v1/config/users/{id}/enable"),
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["active"], true);
}

// ---------------------------------------------------------------------------
// Owner-lockout invariant (owner "cannot be locked out")
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cannot_disable_or_demote_last_owner() {
    let tdb = spawn_db().await;
    let (tid, _uid, _r) = seed_tenant_admin(&tdb.db, "farma", "admin@farma.cl").await; // admin+cashier, NOT owner
    let owner_id = seed_user_raw(&tdb.db, &tid, "dueno@farma.cl", &["owner"], true).await;
    let owner_token = token_for(&owner_id, &tid, vec!["owner".into()]);
    let app = api::build_router(state_free(tdb.db.clone()));

    // Disabling the sole active owner is refused.
    let (st, _) = req_json(
        &app,
        "POST",
        &format!("/api/v1/config/users/{owner_id}/disable"),
        Some(&owner_token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);

    // Demoting the sole active owner (dropping the owner role) is refused.
    let (st, _) = req_json(
        &app,
        "PATCH",
        &format!("/api/v1/config/users/{owner_id}"),
        Some(&owner_token),
        Some(serde_json::json!({ "roles": ["admin"] })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);

    // With a SECOND owner present, disabling the first is allowed.
    let _owner2 = seed_user_raw(&tdb.db, &tid, "dueno2@farma.cl", &["owner"], true).await;
    let (st, body) = req_json(
        &app,
        "POST",
        &format!("/api/v1/config/users/{owner_id}/disable"),
        Some(&owner_token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["active"], false);
}

#[tokio::test]
async fn only_owner_can_grant_owner_role() {
    let tdb = spawn_db().await;
    let (tid, uid, _r) = seed_tenant_admin(&tdb.db, "farma", "admin@farma.cl").await;
    let admin_token = token_for(&uid, &tid, vec!["admin".into()]);
    let app = api::build_router(state_free(tdb.db.clone()));

    // An admin cannot mint an owner (privilege-escalation guard).
    let body = serde_json::json!({ "email": "boss@farma.cl", "password": "secreto8x", "roles": ["owner"] });
    let (st, _) = req_json(
        &app,
        "POST",
        "/api/v1/config/users",
        Some(&admin_token),
        Some(body),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    // An owner can.
    let owner_id = seed_user_raw(&tdb.db, &tid, "dueno@farma.cl", &["owner"], true).await;
    let owner_token = token_for(&owner_id, &tid, vec!["owner".into()]);
    let body = serde_json::json!({ "email": "boss@farma.cl", "password": "secreto8x", "roles": ["owner"] });
    let (st, created) = req_json(
        &app,
        "POST",
        "/api/v1/config/users",
        Some(&owner_token),
        Some(body),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{created}");
    assert_eq!(created["roles"][0], "owner");
}

// ---------------------------------------------------------------------------
// Tenant isolation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tenant_isolation_users() {
    let tdb = spawn_db().await;
    let (t1, u1, _r1) = seed_tenant_admin(&tdb.db, "uno", "admin@uno.cl").await;
    let (_t2, u2, _r2) = seed_tenant_admin(&tdb.db, "dos", "admin@dos.cl").await;
    let token1 = token_for(&u1, &t1, vec!["admin".into()]);
    let app = api::build_router(state_free(tdb.db.clone()));

    // Tenant-1 admin sees only tenant-1 users.
    let (_st, list) = req_json(
        &app,
        "GET",
        "/api/v1/config/users",
        Some(&token1),
        None,
        &[],
    )
    .await;
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["email"], "admin@uno.cl");

    // Tenant-1 admin cannot touch a tenant-2 user — scoped out → 404, not 200.
    let (st, _) = req_json(
        &app,
        "POST",
        &format!("/api/v1/config/users/{u2}/disable"),
        Some(&token1),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = req_json(
        &app,
        "PATCH",
        &format!("/api/v1/config/users/{u2}"),
        Some(&token1),
        Some(serde_json::json!({ "roles": ["cashier"] })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Payment methods
// ---------------------------------------------------------------------------

#[tokio::test]
async fn payment_methods_crud() {
    let tdb = spawn_db().await;
    let (tid, uid, _r) = seed_tenant_admin(&tdb.db, "farma", "admin@farma.cl").await;
    let token = token_for(&uid, &tid, vec!["admin".into()]);
    let app = api::build_router(state_free(tdb.db.clone()));

    // GET exposes the full catalog under `available`.
    let (st, body) = req_json(
        &app,
        "GET",
        "/api/v1/config/payment-methods",
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let available = body["available"].as_array().unwrap();
    for m in ["efectivo", "debito", "credito", "convenio"] {
        assert!(available.iter().any(|v| v == m), "catalog missing {m}");
    }
    assert!(body["methods"].is_array());

    // PUT a subset (with a duplicate that must be deduped, order preserved).
    let (st, body) = req_json(
        &app,
        "PUT",
        "/api/v1/config/payment-methods",
        Some(&token),
        Some(serde_json::json!({ "methods": ["efectivo", "convenio", "efectivo"] })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let methods = body["methods"].as_array().unwrap();
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0], "efectivo");
    assert_eq!(methods[1], "convenio");

    // GET reflects the persisted set.
    let (_st, body) = req_json(
        &app,
        "GET",
        "/api/v1/config/payment-methods",
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(body["methods"].as_array().unwrap().len(), 2);

    // Invalid method → 400.
    let (st, _) = req_json(
        &app,
        "PUT",
        "/api/v1/config/payment-methods",
        Some(&token),
        Some(serde_json::json!({ "methods": ["bitcoin"] })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // Empty set → 400.
    let (st, _) = req_json(
        &app,
        "PUT",
        "/api/v1/config/payment-methods",
        Some(&token),
        Some(serde_json::json!({ "methods": [] })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Business profile
// ---------------------------------------------------------------------------

#[tokio::test]
async fn business_profile_persist_read() {
    let tdb = spawn_db().await;
    let (tid, uid, _r) = seed_tenant_admin(&tdb.db, "farma", "admin@farma.cl").await;
    let token = token_for(&uid, &tid, vec!["admin".into()]);
    let app = api::build_router(state_free(tdb.db.clone()));

    // Unset → fields null.
    let (st, body) = req_json(
        &app,
        "GET",
        "/api/v1/config/business",
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert!(body["razon_social"].is_null());

    // Set.
    let (st, body) = req_json(
        &app,
        "PUT",
        "/api/v1/config/business",
        Some(&token),
        Some(serde_json::json!({
            "razon_social": "Farmacia Ñuñoa SpA",
            "giro": "Venta de medicamentos",
            "direccion": "Av. Siempre Viva 123",
            "telefono": "+56912345678",
            "email": "contacto@farma.cl"
        })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    assert_eq!(body["razon_social"], "Farmacia Ñuñoa SpA");

    // GET reflects.
    let (_st, body) = req_json(
        &app,
        "GET",
        "/api/v1/config/business",
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(body["giro"], "Venta de medicamentos");
    assert_eq!(body["direccion"], "Av. Siempre Viva 123");

    // razon_social is required.
    let (st, _) = req_json(
        &app,
        "PUT",
        "/api/v1/config/business",
        Some(&token),
        Some(serde_json::json!({ "giro": "sin razon social" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn config_reads_open_writes_gated_for_cashier() {
    let tdb = spawn_db().await;
    let (tid, uid, _r) = seed_tenant_admin(&tdb.db, "farma", "admin@farma.cl").await;
    let cashier = token_for(&uid, &tid, vec!["cashier".into()]);
    let app = api::build_router(state_free(tdb.db.clone()));

    // Cashier CAN read (POS needs accepted methods + receipt header).
    let (st, _) = req_json(
        &app,
        "GET",
        "/api/v1/config/payment-methods",
        Some(&cashier),
        None,
        &[],
    )
    .await;
    assert_ne!(st, StatusCode::FORBIDDEN);
    let (st, _) = req_json(
        &app,
        "GET",
        "/api/v1/config/business",
        Some(&cashier),
        None,
        &[],
    )
    .await;
    assert_ne!(st, StatusCode::FORBIDDEN);

    // Cashier CANNOT write.
    let (st, _) = req_json(
        &app,
        "PUT",
        "/api/v1/config/payment-methods",
        Some(&cashier),
        Some(serde_json::json!({ "methods": ["efectivo"] })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, _) = req_json(
        &app,
        "PUT",
        "/api/v1/config/business",
        Some(&cashier),
        Some(serde_json::json!({ "razon_social": "x" })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}
||||||| ae17e6f

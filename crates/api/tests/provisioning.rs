//! SP2 — `POST /admin/v1/tenants` (SaaS signup provisioning) + cross-tenant
//! isolation gate.
//!
//! Contract under test (spec `docs/product/saas-web-prompts/sp2-provisioning-api.md`):
//! * no key configured → 404 (route invisible, on-prem default)
//! * key configured + wrong header → 401 `PROVISIONING_KEY_INVALID`
//! * happy path → 201 `{tenant_id, slug}` and the admin can log in via
//!   `/api/v1/login` with the right `tenant_id` claim
//! * duplicate slug/RUT → 409 `TENANT_EXISTS`
//! * invalid vertical / RUT / short password → 422
//! * isolation: two endpoint-provisioned tenants never see each other's
//!   products, orders, or settings (public-launch gate).

mod e2e_common;

use axum::http::StatusCode;
use e2e_common::*;
use serde_json::json;

const KEY: &str = "sp2-secret-key";

/// App with the provisioning key wired (as `PHARMA__PROVISIONING__KEY` would).
fn app_with_key(db: std::sync::Arc<db::Db>) -> axum::Router {
    let mut state = state_free(db);
    state.provisioning_key = Some(KEY.into());
    api::build_router(state)
}

fn body(
    slug: Option<&str>,
    name: &str,
    rut: &str,
    vertical: &str,
    email: &str,
) -> serde_json::Value {
    let mut b = json!({
        "business_name": name,
        "rut": rut,
        "vertical": vertical,
        "admin_email": email,
        "admin_password": "super-secreta-9",
    });
    if let Some(s) = slug {
        b["slug"] = json!(s);
    }
    b
}

async fn provision(
    app: &axum::Router,
    key: Option<&str>,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let headers: Vec<(&str, &str)> = match key {
        Some(k) => vec![("x-provisioning-key", k)],
        None => vec![],
    };
    req_json(app, "POST", "/admin/v1/tenants", None, Some(body), &headers).await
}

#[tokio::test]
async fn sin_key_configurada_responde_404() {
    let tdb = spawn_db().await;
    // state_free ⇒ provisioning_key = None (on-prem default).
    let app = api::build_router(state_free(tdb.db.clone()));
    let (st, _) = provision(
        &app,
        Some(KEY),
        body(None, "Mi Negocio", "76.543.210-3", "minimarket", "a@b.cl"),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn key_mala_responde_401() {
    let tdb = spawn_db().await;
    let app = app_with_key(tdb.db.clone());
    for wrong in [Some("otra-key"), None] {
        let (st, b) = provision(
            &app,
            wrong,
            body(None, "Mi Negocio", "76.543.210-3", "minimarket", "a@b.cl"),
        )
        .await;
        assert_eq!(st, StatusCode::UNAUTHORIZED, "{b}");
        assert_eq!(b["error"]["code"], "PROVISIONING_KEY_INVALID");
    }
}

#[tokio::test]
async fn key_ok_201_y_login_admin_funciona() {
    let tdb = spawn_db().await;
    let app = app_with_key(tdb.db.clone());

    let (st, b) = provision(
        &app,
        Some(KEY),
        body(
            None,
            "Mi Negocio",
            "76.543.210-3",
            "minimarket",
            "Dueno@Mail.cl",
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{b}");
    let tenant_id = b["tenant_id"].as_str().unwrap().to_string();
    let slug = b["slug"].as_str().unwrap().to_string();
    assert!(tenant_id.starts_with("tenant:"), "{tenant_id}");
    assert_eq!(slug, "mi-negocio", "slug derivado del business_name");

    // Login del admin recién creado (email normalizado a minúsculas).
    let (st, login) = req_json(
        &app,
        "POST",
        "/api/v1/login",
        None,
        Some(json!({
            "tenant": slug,
            "email": "dueno@mail.cl",
            "password": "super-secreta-9",
        })),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{login}");
    let token = login["token"].as_str().unwrap().to_string();

    // El JWT lleva el tenant_id del tenant creado.
    let (st, me) = req_json(&app, "GET", "/api/v1/me", Some(&token), None, &[]).await;
    assert_eq!(st, StatusCode::OK, "{me}");
    assert_eq!(me["tenant_id"].as_str().unwrap(), tenant_id);
    let roles: Vec<&str> = me["roles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap())
        .collect();
    assert!(
        roles.contains(&"admin") && roles.contains(&"owner"),
        "{roles:?}"
    );

    // El vertical quedó guardado como admin_setting `business.vertical`.
    let (st, setting) = req_json(
        &app,
        "GET",
        "/api/v1/settings/business.vertical",
        Some(&token),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{setting}");
    assert_eq!(setting["value"], "minimarket");
}

#[tokio::test]
async fn slug_y_rut_duplicados_responden_409() {
    let tdb = spawn_db().await;
    let app = app_with_key(tdb.db.clone());

    let (st, _) = provision(
        &app,
        Some(KEY),
        body(
            Some("negocio-a"),
            "Negocio A",
            "76.543.210-3",
            "farmacia",
            "a@a.cl",
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);

    // Mismo slug, RUT distinto.
    let (st, b) = provision(
        &app,
        Some(KEY),
        body(
            Some("negocio-a"),
            "Otro",
            "11.111.111-1",
            "farmacia",
            "b@b.cl",
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{b}");
    assert_eq!(b["error"]["code"], "TENANT_EXISTS");

    // Slug distinto, mismo RUT (con otro formato — normaliza igual).
    let (st, b) = provision(
        &app,
        Some(KEY),
        body(
            Some("negocio-b"),
            "Otro",
            "76543210-3",
            "farmacia",
            "b@b.cl",
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{b}");
    assert_eq!(b["error"]["code"], "TENANT_EXISTS");
}

#[tokio::test]
async fn validaciones_responden_422() {
    let tdb = spawn_db().await;
    let app = app_with_key(tdb.db.clone());

    // Vertical fuera de catálogo.
    let (st, b) = provision(
        &app,
        Some(KEY),
        body(None, "X SpA", "76.543.210-3", "astronave", "a@b.cl"),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{b}");

    // RUT con dígito verificador malo.
    let (st, b) = provision(
        &app,
        Some(KEY),
        body(None, "X SpA", "12.345.678-0", "farmacia", "a@b.cl"),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{b}");

    // Password corta.
    let mut short = body(None, "X SpA", "76.543.210-3", "farmacia", "a@b.cl");
    short["admin_password"] = json!("corta");
    let (st, b) = provision(&app, Some(KEY), short).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{b}");

    // Nada quedó creado por los intentos inválidos.
    let (st, _) = req_json(
        &app,
        "POST",
        "/api/v1/login",
        None,
        Some(json!({"tenant": "x-spa", "email": "a@b.cl", "password": "corta"})),
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

/// Gate de salida a público: dos tenants creados vía endpoint NO se ven entre
/// sí en products, sales/orders ni settings.
#[tokio::test]
async fn aislamiento_cross_tenant_products_orders_settings() {
    let tdb = spawn_db().await;
    let app = app_with_key(tdb.db.clone());
    let db = tdb.db.clone();

    let (st, a) = provision(
        &app,
        Some(KEY),
        body(
            Some("tienda-a"),
            "Tienda A",
            "76.543.210-3",
            "farmacia",
            "a@a.cl",
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{a}");
    let (st, b) = provision(
        &app,
        Some(KEY),
        body(
            Some("tienda-b"),
            "Tienda B",
            "11.111.111-1",
            "minimarket",
            "b@b.cl",
        ),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{b}");

    let ta = tid_thing(a["tenant_id"].as_str().unwrap());

    async fn login(app: &axum::Router, slug: &str, email: &str) -> String {
        let (st, r) = req_json(
            app,
            "POST",
            "/api/v1/login",
            None,
            Some(json!({"tenant": slug, "email": email, "password": "super-secreta-9"})),
            &[],
        )
        .await;
        assert_eq!(st, StatusCode::OK, "{r}");
        r["token"].as_str().unwrap().to_string()
    }
    let tok_a = login(&app, "tienda-a", "a@a.cl").await;
    let tok_b = login(&app, "tienda-b", "b@b.cl").await;

    // Producto + venta SOLO en A (seed vía domain, patrón e2e_common).
    let pa = seed_product(&db, &ta, "IBUPROFENO-400", "1500", "800", 20, None).await;
    seed_sale(
        &db,
        &ta,
        None,
        "pos_cash",
        Some("3000"),
        None,
        None,
        &[SaleLine {
            product: &pa,
            name: "IBUPROFENO-400",
            qty: 2,
            unit_price: "1500",
        }],
    )
    .await;

    // 1) products: B no ve el producto de A.
    let (st, prods_b) = req_json(&app, "GET", "/api/v1/products", Some(&tok_b), None, &[]).await;
    assert_eq!(st, StatusCode::OK, "{prods_b}");
    assert_eq!(
        prods_b.as_array().unwrap().len(),
        0,
        "B ve products de A: {prods_b}"
    );
    let (st, prods_a) = req_json(&app, "GET", "/api/v1/products", Some(&tok_a), None, &[]).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(
        prods_a.as_array().unwrap().len(),
        1,
        "A ve su propio producto"
    );

    // 2) sales/POS: B no ve la venta de A.
    let (st, orders_b) = req_json(&app, "GET", "/api/v1/orders", Some(&tok_b), None, &[]).await;
    assert_eq!(st, StatusCode::OK, "{orders_b}");
    assert_eq!(
        orders_b.as_array().unwrap().len(),
        0,
        "B ve orders de A: {orders_b}"
    );
    let (st, orders_a) = req_json(&app, "GET", "/api/v1/orders", Some(&tok_a), None, &[]).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(
        orders_a.as_array().unwrap().len(),
        1,
        "A ve su propia venta"
    );

    // 3) settings: cada tenant lee SU vertical/RUT, nunca el del otro.
    let (st, va) = req_json(
        &app,
        "GET",
        "/api/v1/settings/business.vertical",
        Some(&tok_a),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{va}");
    assert_eq!(va["value"], "farmacia");
    let (st, vb) = req_json(
        &app,
        "GET",
        "/api/v1/settings/business.vertical",
        Some(&tok_b),
        None,
        &[],
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{vb}");
    assert_eq!(vb["value"], "minimarket");

    let (_, ra) = req_json(
        &app,
        "GET",
        "/api/v1/settings/business.rut",
        Some(&tok_a),
        None,
        &[],
    )
    .await;
    let (_, rb) = req_json(
        &app,
        "GET",
        "/api/v1/settings/business.rut",
        Some(&tok_b),
        None,
        &[],
    )
    .await;
    assert_eq!(ra["value"], "765432103");
    assert_eq!(rb["value"], "111111111");
}

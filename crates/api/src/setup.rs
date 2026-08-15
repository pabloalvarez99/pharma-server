//! First-run setup — UNAUTHENTICATED bootstrap of the first tenant + owner user.
//!
//! Solves the install-day chicken-and-egg: a fresh DB has no tenant and no user,
//! so `/api/v1/login` can never succeed and the lone operator is forced into the
//! terminal (`pharma tenant-create` + `pharma user-create`) before they can even
//! see the app. For the freemium single-RUT install that means "open the app →
//! dead end" unless someone hands them CLI instructions. These two routes let the
//! operator create their own account from the login screen, no terminal.
//!
//! FAIL-CLOSED for `/setup`: that pair is gated on the WHOLE DB being userless.
//! The moment any `user` row exists, `GET /setup/status` returns
//! `needs_setup=false` and `POST /setup` returns 409. That is correct for a
//! single-puesto node (lab, teléfono, MSI). It is **not** the gate for the
//! shared feria cloud: there `POST /api/v1/alta` creates another tenant+owner
//! when the slug and the email are free. `/alta` is public (same as `/setup`)
//! and sits behind the existing per-IP limiter.
//!
//! On success it returns the same `{ token, token_type, expires_in }` shape as
//! `/login`, so the client logs the new owner straight in.

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::AppState;

const MIN_PASSWORD_LEN: usize = 8;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/setup/status", get(setup_status))
        .route("/api/v1/setup", post(setup))
        .route("/api/v1/alta", post(alta))
        // Público: el mismo token-bucket por IP que el catálogo y el rescate.
        // En tests `state.rate_limit` es `None` y el layer se hace a un lado.
        .route_layer(crate::rate_limit::ip_layer(state))
}

#[derive(Serialize)]
struct SetupStatus {
    /// `true` when the DB has zero users → first-run account creation is offered.
    needs_setup: bool,
}

#[derive(Deserialize)]
struct SetupRequest {
    /// Display name of the business (used for the tenant name + sidebar).
    business_name: String,
    /// Optional branch slug (login "Sucursal"). Defaults to a slug of the
    /// business name, or `principal` if that is empty.
    #[serde(default)]
    tenant_slug: Option<String>,
    email: String,
    password: String,
    /// Optional rubro stored as `business.vertical` (e.g. `farmacia`,
    /// `minimarket`, `otro`). The client maps it; the server stores it verbatim.
    #[serde(default)]
    vertical: Option<String>,
}

#[derive(Serialize)]
struct SetupResponse {
    token: String,
    token_type: &'static str,
    expires_in: u64,
    /// Echoed back so the client can pre-fill the "Sucursal" field on the next
    /// login without guessing.
    tenant_slug: String,
}

#[derive(Debug, Deserialize)]
struct IdRow {
    id: surrealdb::sql::Thing,
}

/// `true` if NO user exists anywhere in the DB (install-wide, not tenant-scoped).
async fn db_is_userless(db: &db::Db) -> Result<bool, ApiError> {
    let mut q = db.query("SELECT id FROM user LIMIT 1").await.map_err(|e| {
        tracing::warn!(error = %e, "setup: user existence probe failed");
        ApiError::service_unavailable()
    })?;
    let row: Option<IdRow> = q.take(0).map_err(|e| {
        tracing::warn!(error = %e, "setup: user probe decode failed");
        ApiError::service_unavailable()
    })?;
    Ok(row.is_none())
}

/// GET /api/v1/setup/status — does this install still need first-run setup?
async fn setup_status(State(s): State<AppState>) -> Result<Json<SetupStatus>, ApiError> {
    let db = s.db.as_ref().ok_or_else(ApiError::service_unavailable)?;
    Ok(Json(SetupStatus {
        needs_setup: db_is_userless(db).await?,
    }))
}

/// POST /api/v1/setup — create the first tenant + owner user, then log them in.
///
/// 409 (`SETUP_ALREADY_DONE`) once any user exists. 400 on invalid input.
async fn setup(
    State(s): State<AppState>,
    Json(req): Json<SetupRequest>,
) -> Result<Json<SetupResponse>, ApiError> {
    let db = s.db.as_ref().ok_or_else(ApiError::service_unavailable)?;

    // Fail-closed: refuse if the install already has an account.
    if !db_is_userless(db).await? {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "SETUP_ALREADY_DONE",
            "Este servidor ya tiene una cuenta. Pide a tu administrador que cree usuarios nuevos.",
        ));
    }

    // --- validate input (Spanish, user-facing) ---
    let business_name = req.business_name.trim();
    if business_name.is_empty() {
        return Err(ApiError::invalid("Indica el nombre de tu negocio."));
    }
    let email = req.email.trim().to_lowercase();
    if email.is_empty() {
        return Err(ApiError::invalid("Indica tu correo."));
    }
    if !email.contains('@') || !email.contains('.') || email.contains(' ') {
        return Err(ApiError::invalid(
            "El correo no tiene un formato válido (ej: dueno@minegocio.cl).",
        ));
    }
    if req.password.len() < MIN_PASSWORD_LEN {
        return Err(ApiError::invalid(
            "La contraseña debe tener al menos 8 caracteres.",
        ));
    }

    // Slug: explicit > slug(name) > "principal" (never empty — login needs it).
    let raw_slug = req
        .tenant_slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(domain::catalog::service::slugify)
        .unwrap_or_else(|| domain::catalog::service::slugify(business_name));
    let slug = if raw_slug.is_empty() {
        "principal".to_string()
    } else {
        raw_slug
    };

    let hash = auth::password::hash(&req.password).map_err(|e| {
        tracing::error!(error = %e, "setup: password hash failed");
        ApiError::service_unavailable()
    })?;

    // --- create tenant ---
    let mut tq = db
        .query("CREATE tenant SET name = $name, slug = $slug RETURN AFTER")
        .bind(("name", business_name.to_string()))
        .bind(("slug", slug.clone()))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "setup: create tenant failed");
            ApiError::service_unavailable()
        })?;
    let tenant: IdRow = tq
        .take::<Option<IdRow>>(0)
        .map_err(|e| {
            tracing::error!(error = %e, "setup: tenant decode failed");
            ApiError::service_unavailable()
        })?
        .ok_or_else(ApiError::service_unavailable)?;

    // --- create owner user (owner = highest role; also admin for gated routes) ---
    let roles = vec!["owner".to_string(), "admin".to_string()];
    let mut uq = db
        .query(
            "CREATE user SET tenant = $tenant, email = $email, \
             password = $password, roles = $roles RETURN AFTER",
        )
        .bind(("tenant", tenant.id.clone()))
        .bind(("email", email.clone()))
        .bind(("password", hash))
        .bind(("roles", roles.clone()))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "setup: create user failed");
            ApiError::service_unavailable()
        })?;
    let user: IdRow = uq
        .take::<Option<IdRow>>(0)
        .map_err(|e| {
            tracing::error!(error = %e, "setup: user decode failed");
            ApiError::service_unavailable()
        })?
        .ok_or_else(ApiError::service_unavailable)?;

    // --- optional: store the chosen rubro + business name (best-effort) ---
    if let Some(v) = req
        .vertical
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if let Err(e) =
            domain::sales::service::set_setting(db, &tenant.id, "business.vertical", v).await
        {
            tracing::warn!(error = %e, "setup: store business.vertical failed (non-fatal)");
        }
    }
    if let Err(e) =
        domain::sales::service::set_setting(db, &tenant.id, "business.name", business_name).await
    {
        tracing::warn!(error = %e, "setup: store business.name failed (non-fatal)");
    }

    // --- issue a JWT so the operator is logged straight in (mirrors /login) ---
    let sub = user.id.to_string();
    let tenant_id = tenant.id.to_string();
    let token = auth::issue(&s.jwt, &sub, &tenant_id, roles).map_err(|e| {
        tracing::error!(error = %e, "setup: issue jwt failed");
        ApiError::service_unavailable()
    })?;

    tracing::info!(tenant = %tenant_id, %email, %slug, "first-run setup complete");

    Ok(Json(SetupResponse {
        token,
        token_type: "Bearer",
        expires_in: s.jwt.ttl_seconds,
        tenant_slug: slug,
    }))
}

/// POST /api/v1/alta — otro negocio en una nube que ya tiene dueños.
///
/// `/setup` se cierra cuando existe un usuario. Esta ruta no: el segundo
/// feriante tiene que poder abrir su puesto sin admin. Rechaza slug o correo
/// ya usados. El cuerpo y la respuesta son los de `/setup`.
async fn alta(
    State(s): State<AppState>,
    Json(req): Json<SetupRequest>,
) -> Result<Json<SetupResponse>, ApiError> {
    let db = s.db.as_ref().ok_or_else(ApiError::service_unavailable)?;

    let business_name = req.business_name.trim();
    if business_name.is_empty() {
        return Err(ApiError::invalid("Indica el nombre de tu negocio."));
    }
    let email = req.email.trim().to_lowercase();
    if email.is_empty() {
        return Err(ApiError::invalid("Indica tu correo."));
    }
    if !email.contains('@') || !email.contains('.') || email.contains(' ') {
        return Err(ApiError::invalid(
            "El correo no tiene un formato válido (ej: dueno@minegocio.cl).",
        ));
    }
    if req.password.len() < MIN_PASSWORD_LEN {
        return Err(ApiError::invalid(
            "La contraseña debe tener al menos 8 caracteres.",
        ));
    }

    let raw_slug = req
        .tenant_slug
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(domain::catalog::service::slugify)
        .unwrap_or_else(|| domain::catalog::service::slugify(business_name));
    let slug = if raw_slug.is_empty() {
        "principal".to_string()
    } else {
        raw_slug
    };

    if slug_tomado(db, &slug).await? {
        return Err(error_slug_tomado());
    }
    if correo_tomado(db, &email).await? {
        return Err(error_correo_tomado());
    }

    let hash = auth::password::hash(&req.password).map_err(|e| {
        tracing::error!(error = %e, "alta: password hash failed");
        ApiError::service_unavailable()
    })?;

    let mut tq = db
        .query("CREATE tenant SET name = $name, slug = $slug RETURN AFTER")
        .bind(("name", business_name.to_string()))
        .bind(("slug", slug.clone()))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "alta: create tenant failed");
            ApiError::service_unavailable()
        })?;
    let tenant: IdRow = match tq.take::<Option<IdRow>>(0) {
        Ok(Some(row)) => row,
        Ok(None) | Err(_) => {
            // Carrera: otro POST pasó el probe y el UNIQUE de slug ganó.
            if slug_tomado(db, &slug).await.unwrap_or(false) {
                return Err(error_slug_tomado());
            }
            return Err(ApiError::service_unavailable());
        }
    };

    let roles = vec!["owner".to_string(), "admin".to_string()];
    let mut uq = db
        .query(
            "CREATE user SET tenant = $tenant, email = $email, \
             password = $password, roles = $roles RETURN AFTER",
        )
        .bind(("tenant", tenant.id.clone()))
        .bind(("email", email.clone()))
        .bind(("password", hash))
        .bind(("roles", roles.clone()))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "alta: create user failed");
            ApiError::service_unavailable()
        })?;
    let user: IdRow = match uq.take::<Option<IdRow>>(0) {
        Ok(Some(row)) => row,
        Ok(None) | Err(_) => {
            if correo_tomado(db, &email).await.unwrap_or(false) {
                return Err(error_correo_tomado());
            }
            return Err(ApiError::service_unavailable());
        }
    };

    if let Some(v) = req
        .vertical
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if let Err(e) =
            domain::sales::service::set_setting(db, &tenant.id, "business.vertical", v).await
        {
            tracing::warn!(error = %e, "alta: store business.vertical failed (non-fatal)");
        }
    }
    if let Err(e) =
        domain::sales::service::set_setting(db, &tenant.id, "business.name", business_name).await
    {
        tracing::warn!(error = %e, "alta: store business.name failed (non-fatal)");
    }

    let sub = user.id.to_string();
    let tenant_id = tenant.id.to_string();
    let token = auth::issue(&s.jwt, &sub, &tenant_id, roles).map_err(|e| {
        tracing::error!(error = %e, "alta: issue jwt failed");
        ApiError::service_unavailable()
    })?;

    tracing::info!(tenant = %tenant_id, %email, %slug, "alta pública");

    Ok(Json(SetupResponse {
        token,
        token_type: "Bearer",
        expires_in: s.jwt.ttl_seconds,
        tenant_slug: slug,
    }))
}

fn error_slug_tomado() -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        "SLUG_TOMADO",
        "Ese nombre corto ya lo usa otro puesto. Probá con otro nombre.",
    )
}

fn error_correo_tomado() -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        "CORREO_TOMADO",
        "Ese correo ya tiene un puesto. Entrá con tu clave, no crees otro.",
    )
}

async fn slug_tomado(db: &db::Db, slug: &str) -> Result<bool, ApiError> {
    let mut q = db
        .query("SELECT id FROM tenant WHERE slug = $slug LIMIT 1")
        .bind(("slug", slug.to_string()))
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "alta: slug probe failed");
            ApiError::service_unavailable()
        })?;
    let row: Option<IdRow> = q.take(0).map_err(|e| {
        tracing::warn!(error = %e, "alta: slug decode failed");
        ApiError::service_unavailable()
    })?;
    Ok(row.is_some())
}

async fn correo_tomado(db: &db::Db, email: &str) -> Result<bool, ApiError> {
    let mut q = db
        .query("SELECT id FROM user WHERE email = $email LIMIT 1")
        .bind(("email", email.to_string()))
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "alta: email probe failed");
            ApiError::service_unavailable()
        })?;
    let row: Option<IdRow> = q.take(0).map_err(|e| {
        tracing::warn!(error = %e, "alta: email decode failed");
        ApiError::service_unavailable()
    })?;
    Ok(row.is_some())
}

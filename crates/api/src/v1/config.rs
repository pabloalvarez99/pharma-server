//! Config Center backend — closes the CLI→UI gap so the owner can manage their
//! team and business settings without a terminal (`pharma user-create` was the
//! only path before).
//!
//! Surface (all tenant-scoped via the JWT `tenant_id` claim):
//! * `GET/POST   /api/v1/config/users`            — list / create users (admin+)
//! * `PATCH      /api/v1/config/users/{id}`        — edit roles (admin+)
//! * `POST       /api/v1/config/users/{id}/disable|enable` — toggle `active` (admin+)
//! * `GET/PUT    /api/v1/config/payment-methods`   — accepted tender (GET any auth, PUT admin+)
//! * `GET/PUT    /api/v1/config/business`          — business profile (GET any auth, PUT admin+)
//!
//! Two safety invariants beyond the role gate:
//! * **Owner-grant**: only an `owner` caller may assign the `owner` role
//!   (prevents an admin escalating itself/others to owner).
//! * **Last-owner**: the sole active `owner` cannot be disabled nor stripped of
//!   the owner role — the tenant must never be left ownerless (matches the
//!   "owner cannot be locked out" rule in [`crate::role`]).
//!
//! User-management talks to the DB directly (same as [`crate::setup`]) rather
//! than a domain service — there is no `domain::users` and the queries are
//! thin. Payment methods + business profile reuse the existing `admin_setting`
//! store via `domain::sales::service::{get,set}_setting`. SII emisor identity
//! (RUT/comuna for DTE) lives separately under `dte.emisor`; this profile is the
//! general business card for the UI/receipts.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, patch, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::role::admin_plus;
use crate::AppState;

const MIN_PASSWORD_LEN: usize = 8;

/// Canonical role strings (English; the UI maps to Spanish labels). Mirrors the
/// set parsed in [`crate::role::RoleSet::from_claim`].
const VALID_ROLES: [&str; 4] = ["cashier", "pharmacist", "admin", "owner"];

/// Accepted payment-method catalog (machine keys; UI renders labels).
const PAYMENT_METHODS: [&str; 4] = ["efectivo", "debito", "credito", "convenio"];
const PAYMENT_METHODS_KEY: &str = "business.payment_methods";
const BUSINESS_PROFILE_KEY: &str = "business.profile";

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(state: AppState) -> Router<AppState> {
    // User management: back-office, admin/owner only (never lower this gate).
    let users = Router::new()
        .route("/api/v1/config/users", get(list_users).post(create_user))
        .route("/api/v1/config/users/{id}", patch(update_user_roles))
        .route("/api/v1/config/users/{id}/disable", post(disable_user))
        .route("/api/v1/config/users/{id}/enable", post(enable_user))
        .route_layer(crate::role::layer(state.clone(), admin_plus()));

    // Reads are open to any authenticated user (the POS needs accepted tender +
    // the receipt header) — consistent with the generic `GET /settings/{key}`.
    let reads = Router::new()
        .route("/api/v1/config/payment-methods", get(get_payment_methods))
        .route("/api/v1/config/business", get(get_business));

    // Writes are admin/owner only.
    let writes = Router::new()
        .route("/api/v1/config/payment-methods", put(set_payment_methods))
        .route("/api/v1/config/business", put(set_business))
        .route_layer(crate::role::layer(state, admin_plus()));

    users.merge(reads).merge(writes)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

fn db_err(e: surrealdb::Error) -> ApiError {
    tracing::warn!(error = %e, "config: db query failed");
    ApiError::service_unavailable()
}

fn caller_is_owner(claims: &auth::Claims) -> bool {
    claims.roles.iter().any(|r| r == "owner")
}

fn validate_roles(roles: &[String]) -> Result<(), ApiError> {
    if roles.is_empty() {
        return Err(ApiError::invalid("Asigna al menos un rol al usuario."));
    }
    for r in roles {
        if !VALID_ROLES.contains(&r.as_str()) {
            return Err(ApiError::invalid(format!(
                "Rol desconocido: '{r}'. Válidos: cajero, químico, admin, dueño."
            )));
        }
    }
    Ok(())
}

/// Normalize + sanity-check an email (mirrors `crate::setup`).
fn validate_email(raw: &str) -> Result<String, ApiError> {
    let email = raw.trim().to_lowercase();
    if email.is_empty() {
        return Err(ApiError::invalid("Indica el correo del usuario."));
    }
    if !email.contains('@') || !email.contains('.') || email.contains(' ') {
        return Err(ApiError::invalid(
            "El correo no tiene un formato válido (ej: usuario@minegocio.cl).",
        ));
    }
    Ok(email)
}

/// Count active users holding the `owner` role in this tenant.
async fn active_owner_count(db: &db::Db, tenant: &Thing) -> Result<usize, ApiError> {
    let mut q = db
        .query("SELECT VALUE id FROM user WHERE tenant = $t AND active = true AND roles CONTAINS 'owner'")
        .bind(("t", tenant.clone()))
        .await
        .map_err(db_err)?;
    let ids: Vec<Thing> = q.take(0).map_err(db_err)?;
    Ok(ids.len())
}

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

/// Row as decoded from `user` (the `password` + `tenant` columns are dropped by
/// serde — they are never serialized back to the client).
#[derive(Debug, Deserialize)]
struct UserRow {
    id: Thing,
    email: String,
    roles: Vec<String>,
    active: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct UserDto {
    id: String,
    email: String,
    roles: Vec<String>,
    active: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<UserRow> for UserDto {
    fn from(r: UserRow) -> Self {
        UserDto {
            id: r.id.to_string(),
            email: r.email,
            roles: r.roles,
            active: r.active,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Deserialize)]
struct NewUserRequest {
    email: String,
    password: String,
    roles: Vec<String>,
}

#[derive(Deserialize)]
struct UpdateRolesRequest {
    roles: Vec<String>,
}

/// Current role/active snapshot for the owner-lockout guards.
#[derive(Deserialize)]
struct RoleActiveRow {
    roles: Vec<String>,
    active: bool,
}

const USER_FIELDS: &str = "id, email, roles, active, created_at, updated_at";

async fn list_users(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<UserDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let mut q = db
        .query(format!(
            "SELECT {USER_FIELDS} FROM user WHERE tenant = $t ORDER BY created_at"
        ))
        .bind(("t", t))
        .await
        .map_err(db_err)?;
    let rows: Vec<UserRow> = q.take(0).map_err(db_err)?;
    Ok(Json(rows.into_iter().map(UserDto::from).collect()))
}

async fn create_user(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<NewUserRequest>,
) -> Result<(StatusCode, Json<UserDto>), ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;

    let email = validate_email(&req.email)?;
    if req.password.len() < MIN_PASSWORD_LEN {
        return Err(ApiError::invalid(
            "La contraseña debe tener al menos 8 caracteres.",
        ));
    }
    validate_roles(&req.roles)?;
    if req.roles.iter().any(|r| r == "owner") && !caller_is_owner(&claims) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Sólo un dueño puede asignar el rol de dueño.",
        ));
    }

    // Pre-check the unique (tenant, email) so the common case returns a clean
    // 409 instead of an opaque index error.
    let mut dq = db
        .query("SELECT VALUE id FROM user WHERE tenant = $t AND email = $e LIMIT 1")
        .bind(("t", t.clone()))
        .bind(("e", email.clone()))
        .await
        .map_err(db_err)?;
    let existing: Vec<Thing> = dq.take(0).map_err(db_err)?;
    if !existing.is_empty() {
        return Err(ApiError::conflict(
            "Ya existe un usuario con ese correo en este negocio.",
        ));
    }

    let hash = auth::password::hash(&req.password).map_err(|e| {
        tracing::error!(error = %e, "config: password hash failed");
        ApiError::service_unavailable()
    })?;

    let mut uq = db
        .query(format!(
            "CREATE user SET tenant = $t, email = $e, password = $p, roles = $r \
             RETURN {USER_FIELDS}"
        ))
        .bind(("t", t))
        .bind(("e", email))
        .bind(("p", hash))
        .bind(("r", req.roles))
        .await
        .map_err(db_err)?;
    let row: UserRow = uq
        .take::<Option<UserRow>>(0)
        .map_err(db_err)?
        .ok_or_else(ApiError::service_unavailable)?;
    Ok((StatusCode::CREATED, Json(row.into())))
}

async fn update_user_roles(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateRolesRequest>,
) -> Result<Json<UserDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let uid = surrealdb::sql::thing(&id).map_err(|_| ApiError::not_found())?;

    validate_roles(&req.roles)?;
    let will_be_owner = req.roles.iter().any(|r| r == "owner");
    if will_be_owner && !caller_is_owner(&claims) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Sólo un dueño puede asignar el rol de dueño.",
        ));
    }

    let cur = current_user(&db, &uid, &t).await?;
    let was_active_owner = cur.active && cur.roles.iter().any(|r| r == "owner");
    if was_active_owner && !will_be_owner && active_owner_count(&db, &t).await? <= 1 {
        return Err(ApiError::conflict(
            "No puedes dejar el negocio sin un dueño activo. Asigna otro dueño primero.",
        ));
    }

    let mut uq = db
        .query(format!(
            "UPDATE user SET roles = $r WHERE id = $id AND tenant = $t RETURN {USER_FIELDS}"
        ))
        .bind(("r", req.roles))
        .bind(("id", uid))
        .bind(("t", t))
        .await
        .map_err(db_err)?;
    let row: UserRow = uq
        .take::<Option<UserRow>>(0)
        .map_err(db_err)?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(row.into()))
}

async fn disable_user(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<UserDto>, ApiError> {
    set_active(&s, &claims, &id, false).await
}

async fn enable_user(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<UserDto>, ApiError> {
    set_active(&s, &claims, &id, true).await
}

async fn set_active(
    s: &AppState,
    claims: &auth::Claims,
    id: &str,
    active: bool,
) -> Result<Json<UserDto>, ApiError> {
    let db = db_of(s)?;
    let t = tenant_of(claims)?;
    let uid = surrealdb::sql::thing(id).map_err(|_| ApiError::not_found())?;

    let cur = current_user(&db, &uid, &t).await?;
    if !active {
        let is_active_owner = cur.active && cur.roles.iter().any(|r| r == "owner");
        if is_active_owner && active_owner_count(&db, &t).await? <= 1 {
            return Err(ApiError::conflict(
                "No puedes deshabilitar al único dueño activo del negocio.",
            ));
        }
    }

    let mut uq = db
        .query(format!(
            "UPDATE user SET active = $a WHERE id = $id AND tenant = $t RETURN {USER_FIELDS}"
        ))
        .bind(("a", active))
        .bind(("id", uid))
        .bind(("t", t))
        .await
        .map_err(db_err)?;
    let row: UserRow = uq
        .take::<Option<UserRow>>(0)
        .map_err(db_err)?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(row.into()))
}

/// Fetch a tenant-scoped user's role/active snapshot; 404 if it is not this
/// tenant's user (the `tenant = $t` clause is the isolation boundary).
async fn current_user(db: &db::Db, uid: &Thing, t: &Thing) -> Result<RoleActiveRow, ApiError> {
    let mut cq = db
        .query("SELECT roles, active FROM user WHERE id = $id AND tenant = $t LIMIT 1")
        .bind(("id", uid.clone()))
        .bind(("t", t.clone()))
        .await
        .map_err(db_err)?;
    cq.take::<Option<RoleActiveRow>>(0)
        .map_err(db_err)?
        .ok_or_else(ApiError::not_found)
}

// ---------------------------------------------------------------------------
// Payment methods
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct PaymentMethodsResponse {
    methods: Vec<String>,
    /// Full catalog the UI can offer as checkboxes.
    available: Vec<String>,
}

#[derive(Deserialize)]
struct PaymentMethodsRequest {
    methods: Vec<String>,
}

fn available_methods() -> Vec<String> {
    PAYMENT_METHODS.iter().map(|s| s.to_string()).collect()
}

fn default_payment_methods() -> Vec<String> {
    vec!["efectivo".into(), "debito".into(), "credito".into()]
}

/// Validate + normalize a requested method set: lowercase, known-only, deduped
/// (order preserved), non-empty.
fn clean_payment_methods(input: Vec<String>) -> Result<Vec<String>, ApiError> {
    let mut out: Vec<String> = Vec::new();
    for m in input {
        let m = m.trim().to_lowercase();
        if !PAYMENT_METHODS.contains(&m.as_str()) {
            return Err(ApiError::invalid(format!(
                "Medio de pago no válido: '{m}'. Válidos: efectivo, debito, credito, convenio."
            )));
        }
        if !out.contains(&m) {
            out.push(m);
        }
    }
    if out.is_empty() {
        return Err(ApiError::invalid("Selecciona al menos un medio de pago."));
    }
    Ok(out)
}

async fn get_payment_methods(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<PaymentMethodsResponse>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let methods =
        match domain::sales::service::get_setting(db.as_ref(), &t, PAYMENT_METHODS_KEY).await? {
            Some(s) => serde_json::from_str::<Vec<String>>(&s.value)
                .unwrap_or_else(|_| default_payment_methods()),
            None => default_payment_methods(),
        };
    Ok(Json(PaymentMethodsResponse {
        methods,
        available: available_methods(),
    }))
}

async fn set_payment_methods(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<PaymentMethodsRequest>,
) -> Result<Json<PaymentMethodsResponse>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let cleaned = clean_payment_methods(req.methods)?;
    let encoded = serde_json::to_string(&cleaned).map_err(|e| {
        tracing::error!(error = %e, "config: encode payment methods failed");
        ApiError::service_unavailable()
    })?;
    domain::sales::service::set_setting(db.as_ref(), &t, PAYMENT_METHODS_KEY, &encoded).await?;
    Ok(Json(PaymentMethodsResponse {
        methods: cleaned,
        available: available_methods(),
    }))
}

// ---------------------------------------------------------------------------
// Business profile
// ---------------------------------------------------------------------------

/// General business identity for the UI/receipts. SII emisor (RUT/comuna) is a
/// separate concern stored under `dte.emisor`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct BusinessProfile {
    #[serde(default)]
    razon_social: Option<String>,
    #[serde(default)]
    giro: Option<String>,
    #[serde(default)]
    direccion: Option<String>,
    #[serde(default)]
    telefono: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

impl BusinessProfile {
    /// Trim every field (empty → `None`); `razon_social` is required.
    fn normalized(self) -> Result<BusinessProfile, ApiError> {
        let trim = |o: Option<String>| o.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let razon_social = trim(self.razon_social);
        if razon_social.is_none() {
            return Err(ApiError::invalid("Indica la razón social del negocio."));
        }
        Ok(BusinessProfile {
            razon_social,
            giro: trim(self.giro),
            direccion: trim(self.direccion),
            telefono: trim(self.telefono),
            email: trim(self.email),
        })
    }
}

async fn get_business(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<BusinessProfile>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let profile =
        match domain::sales::service::get_setting(db.as_ref(), &t, BUSINESS_PROFILE_KEY).await? {
            Some(s) => serde_json::from_str::<BusinessProfile>(&s.value).unwrap_or_default(),
            None => BusinessProfile::default(),
        };
    Ok(Json(profile))
}

async fn set_business(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<BusinessProfile>,
) -> Result<Json<BusinessProfile>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let profile = req.normalized()?;
    let encoded = serde_json::to_string(&profile).map_err(|e| {
        tracing::error!(error = %e, "config: encode business profile failed");
        ApiError::service_unavailable()
    })?;
    domain::sales::service::set_setting(db.as_ref(), &t, BUSINESS_PROFILE_KEY, &encoded).await?;
    Ok(Json(profile))
}

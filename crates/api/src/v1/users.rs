//! Gestión de usuarios (credenciales) — admin/owner, tenant-scoped.
//!
//! Cierra el hueco que `setup.rs` deja explícito ("admins create further users
//! via the CLI"): una vez creada la primera cuenta (first-run, `POST /setup`),
//! este módulo permite a un admin/owner **crear, listar y editar las
//! credenciales de su negocio desde la app** — sin terminal, sin volver a la
//! DB. Es lo que reemplaza el `CREDENCIALES-TEST.txt` + `pharma user-create`.
//!
//! Invariantes de seguridad:
//! * TODAS las rutas exigen rol `admin`/`owner` (`role::layer` + `admin_plus`).
//! * El `tenant` SIEMPRE sale del JWT, nunca del body/URL → aislación
//!   multi-tenant (un admin jamás ve ni toca usuarios de otro negocio; cross
//!   tenant → 404, no 403, para no filtrar existencia).
//! * El hash de la contraseña NUNCA se serializa de vuelta (`UserDto` no tiene
//!   campo `password`; los SELECT lo excluyen explícitamente).
//! * Anti-lockout: no puedes desactivar tu propia cuenta ni dejar al negocio
//!   sin ningún administrador activo.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::{get, patch},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::role::admin_plus;
use crate::AppState;

/// Roles asignables. Espeja el modelo `RoleSet` de `middleware/role.rs`:
/// cualquier otro string sería descartado silenciosamente por el gate, dejando
/// un usuario sin permisos efectivos. Lo rechazamos temprano con 400.
const VALID_ROLES: &[&str] = &["cashier", "pharmacist", "admin", "owner"];
/// Roles que cuentan como "administrador" para el guard anti-lockout.
const ADMIN_ROLES: &[&str] = &["admin", "owner"];
const MIN_PASSWORD_LEN: usize = 8;

// --- helpers compartidos (mismo patrón que branches.rs / audit.rs) ---------

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

pub fn router(state: AppState) -> Router<AppState> {
    // Todas las rutas admin/owner: listar/crear credenciales es back-office
    // sensible (un cajero no enumera ni crea cuentas).
    Router::new()
        .route("/api/v1/users", get(list_users).post(create_user))
        .route("/api/v1/users/{id}", patch(update_user))
        .route_layer(crate::role::layer(state, admin_plus()))
}

// --- DTOs ------------------------------------------------------------------

/// Vista pública de un usuario. **Sin** el hash de contraseña, por diseño.
#[derive(Debug, Serialize)]
pub struct UserDto {
    pub id: String,
    pub email: String,
    pub roles: Vec<String>,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Fila leída de la DB. No declara `password`, así que serde lo descarta al
/// decodificar el record (incluso cuando el SQL hace `RETURN AFTER`).
#[derive(Debug, Deserialize)]
struct UserRow {
    id: Thing,
    email: String,
    roles: Vec<String>,
    active: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl From<UserRow> for UserDto {
    fn from(r: UserRow) -> Self {
        UserDto {
            id: r.id.to_string(),
            email: r.email,
            roles: r.roles,
            active: r.active,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
struct IdRow {
    #[allow(dead_code)]
    id: Thing,
}

#[derive(Debug, Deserialize)]
struct CountRow {
    c: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    email: String,
    password: String,
    #[serde(default)]
    roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    #[serde(default)]
    active: Option<bool>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    roles: Option<Vec<String>>,
}

// --- validación pura (unit-testable) ---------------------------------------

/// Normaliza + valida un correo. Mismo criterio user-facing que `setup.rs`.
fn normalize_email(raw: &str) -> Result<String, ApiError> {
    let email = raw.trim().to_lowercase();
    if email.is_empty() {
        return Err(ApiError::invalid("Indica el correo del usuario."));
    }
    if !email.contains('@') || !email.contains('.') || email.contains(' ') {
        return Err(ApiError::invalid(
            "El correo no tiene un formato válido (ej: cajero@minegocio.cl).",
        ));
    }
    Ok(email)
}

fn validate_password(pw: &str) -> Result<(), ApiError> {
    if pw.len() < MIN_PASSWORD_LEN {
        return Err(ApiError::invalid(
            "La contraseña debe tener al menos 8 caracteres.",
        ));
    }
    Ok(())
}

/// Limpia, deduplica y valida la lista de roles contra [`VALID_ROLES`]. Vacía
/// o con un rol desconocido → 400 (no dejamos crear cuentas sin permisos).
fn validate_roles(roles: Vec<String>) -> Result<Vec<String>, ApiError> {
    let mut out: Vec<String> = Vec::new();
    for r in roles {
        let r = r.trim().to_lowercase();
        if r.is_empty() {
            continue;
        }
        if !VALID_ROLES.contains(&r.as_str()) {
            return Err(ApiError::invalid(format!(
                "Rol inválido: '{r}'. Válidos: cashier, pharmacist, admin, owner."
            )));
        }
        if !out.contains(&r) {
            out.push(r);
        }
    }
    if out.is_empty() {
        return Err(ApiError::invalid(
            "Asigna al menos un rol (cashier, pharmacist, admin u owner).",
        ));
    }
    Ok(out)
}

fn has_admin_role(roles: &[String]) -> bool {
    roles.iter().any(|r| ADMIN_ROLES.contains(&r.as_str()))
}

// --- queries ---------------------------------------------------------------

async fn user_exists(db: &db::Db, tenant: &Thing, email: &str) -> Result<bool, ApiError> {
    let mut q = db
        .query("SELECT id FROM user WHERE tenant = $t AND email = $email LIMIT 1")
        .bind(("t", tenant.clone()))
        .bind(("email", email.to_string()))
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "users: existence probe failed");
            ApiError::service_unavailable()
        })?;
    let row: Option<IdRow> = q.take(0).map_err(|e| {
        tracing::warn!(error = %e, "users: existence decode failed");
        ApiError::service_unavailable()
    })?;
    Ok(row.is_some())
}

async fn fetch_user(db: &db::Db, tenant: &Thing, id: &Thing) -> Result<Option<UserRow>, ApiError> {
    let mut q = db
        .query(
            "SELECT id, email, roles, active, created_at FROM user \
             WHERE id = $id AND tenant = $t LIMIT 1",
        )
        .bind(("id", id.clone()))
        .bind(("t", tenant.clone()))
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "users: fetch failed");
            ApiError::service_unavailable()
        })?;
    q.take(0).map_err(|e| {
        tracing::warn!(error = %e, "users: fetch decode failed");
        ApiError::service_unavailable()
    })
}

/// Cuenta administradores ACTIVOS del tenant (rol admin u owner, `active`).
async fn count_active_admins(db: &db::Db, tenant: &Thing) -> Result<i64, ApiError> {
    let mut q = db
        .query(
            "SELECT count() AS c FROM user \
             WHERE tenant = $t AND active = true AND roles CONTAINSANY ['admin', 'owner'] \
             GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "users: admin count failed");
            ApiError::service_unavailable()
        })?;
    let rows: Vec<CountRow> = q.take(0).map_err(|e| {
        tracing::warn!(error = %e, "users: admin count decode failed");
        ApiError::service_unavailable()
    })?;
    Ok(rows.first().map(|r| r.c).unwrap_or(0))
}

// --- handlers --------------------------------------------------------------

/// Lista los usuarios del negocio (sin contraseñas). Requiere admin+.
#[utoipa::path(get, path = "/api/v1/users", tag = "Users",
    responses((status = 200, body = serde_json::Value),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn list_users(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<UserDto>>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;
    let mut q = db
        .query("SELECT id, email, roles, active, created_at FROM user WHERE tenant = $t ORDER BY email")
        .bind(("t", t))
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "users: list failed");
            ApiError::service_unavailable()
        })?;
    let rows: Vec<UserRow> = q.take(0).map_err(|e| {
        tracing::warn!(error = %e, "users: list decode failed");
        ApiError::service_unavailable()
    })?;
    Ok(Json(rows.into_iter().map(UserDto::from).collect()))
}

/// Crea una credencial nueva en el negocio del admin. Requiere admin+.
#[utoipa::path(post, path = "/api/v1/users", tag = "Users",
    request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 409, description = "Correo ya existe en el negocio", body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn create_user(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<UserDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;

    let email = normalize_email(&req.email)?;
    validate_password(&req.password)?;
    let roles = validate_roles(req.roles)?;

    // Pre-check para un 409 limpio; el índice único `user_tenant_email` es el
    // guard real ante carreras (mapeado abajo).
    if user_exists(&db, &t, &email).await? {
        return Err(ApiError::conflict(
            "Ya existe un usuario con ese correo en este negocio.",
        ));
    }

    let hash = auth::password::hash(&req.password).map_err(|e| {
        tracing::error!(error = %e, "users: password hash failed");
        ApiError::service_unavailable()
    })?;

    let mut q = db
        .query(
            "CREATE user SET tenant = $t, email = $email, password = $password, \
             roles = $roles RETURN AFTER",
        )
        .bind(("t", t))
        .bind(("email", email))
        .bind(("password", hash))
        .bind(("roles", roles))
        .await
        .map_err(map_create_err)?;
    let row: Option<UserRow> = q.take(0).map_err(|e| {
        tracing::error!(error = %e, "users: create decode failed");
        ApiError::service_unavailable()
    })?;
    let row = row.ok_or_else(ApiError::service_unavailable)?;
    tracing::info!(user = %row.id, "users: credencial creada");
    Ok(Json(row.into()))
}

/// Edita una credencial: desactivar/reactivar, resetear contraseña, cambiar
/// roles. Requiere admin+. Campos omitidos quedan intactos.
#[utoipa::path(patch, path = "/api/v1/users/{id}", tag = "Users",
    params(("id" = String, Path)), request_body = serde_json::Value,
    responses((status = 200, body = serde_json::Value),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
        (status = 409, description = "Dejaría al negocio sin administradores", body = crate::error::ErrorEnvelope)),
    security(("bearer_jwt" = [])))]
pub async fn update_user(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserDto>, ApiError> {
    let db = db_of(&s)?;
    let t = tenant_of(&claims)?;

    // Id malformado o de otro tenant → 404 (no filtra existencia).
    let target_id = surrealdb::sql::thing(&id).map_err(|_| ApiError::not_found())?;
    let current = fetch_user(&db, &t, &target_id)
        .await?
        .ok_or_else(ApiError::not_found)?;

    // Validaciones de input.
    let new_roles = match req.roles {
        Some(r) => Some(validate_roles(r)?),
        None => None,
    };
    if let Some(pw) = req.password.as_deref() {
        validate_password(pw)?;
    }
    if req.active.is_none() && req.password.is_none() && new_roles.is_none() {
        return Err(ApiError::invalid(
            "Nada que actualizar. Envía 'active', 'password' o 'roles'.",
        ));
    }

    // --- guards anti-lockout ---
    let is_self = claims.sub == current.id.to_string();
    let deactivating = req.active == Some(false);
    let losing_admin = new_roles
        .as_ref()
        .map(|r| !has_admin_role(r))
        .unwrap_or(false);
    let currently_active_admin = current.active && has_admin_role(&current.roles);

    if is_self && deactivating {
        return Err(ApiError::conflict("No puedes desactivar tu propia cuenta."));
    }
    if currently_active_admin && (deactivating || losing_admin) {
        // ¿Es el último admin activo? Si sí, bloquear (incluye el caso de
        // auto-degradarse a cashier siendo el único admin).
        if count_active_admins(&db, &t).await? <= 1 {
            return Err(ApiError::conflict(
                "El negocio quedaría sin administradores. Asigna otro admin antes de quitar este.",
            ));
        }
    }

    // --- construir UPDATE sólo con los campos provistos ---
    let pw_hash = match req.password.as_deref() {
        Some(pw) => Some(auth::password::hash(pw).map_err(|e| {
            tracing::error!(error = %e, "users: password hash failed");
            ApiError::service_unavailable()
        })?),
        None => None,
    };

    let mut sets: Vec<&str> = Vec::new();
    if req.active.is_some() {
        sets.push("active = $active");
    }
    if pw_hash.is_some() {
        sets.push("password = $password");
    }
    if new_roles.is_some() {
        sets.push("roles = $roles");
    }
    let sql = format!(
        "UPDATE user SET {} WHERE id = $id AND tenant = $t RETURN AFTER",
        sets.join(", ")
    );

    let mut qb = db
        .query(&sql)
        .bind(("id", current.id.clone()))
        .bind(("t", t));
    if let Some(a) = req.active {
        qb = qb.bind(("active", a));
    }
    if let Some(h) = pw_hash {
        qb = qb.bind(("password", h));
    }
    if let Some(r) = new_roles {
        qb = qb.bind(("roles", r));
    }

    let mut res = qb.await.map_err(|e| {
        tracing::error!(error = %e, "users: update failed");
        ApiError::service_unavailable()
    })?;
    let row: Option<UserRow> = res.take(0).map_err(|e| {
        tracing::error!(error = %e, "users: update decode failed");
        ApiError::service_unavailable()
    })?;
    let row = row.ok_or_else(ApiError::not_found)?;
    tracing::info!(user = %row.id, "users: credencial actualizada");
    Ok(Json(row.into()))
}

/// Mapea el error de `CREATE user` a un 409 cuando choca el índice único
/// `user_tenant_email` (carrera tras el pre-check), o a 503 en cualquier otro
/// fallo de DB.
fn map_create_err(e: surrealdb::Error) -> ApiError {
    let msg = e.to_string();
    if msg.contains("user_tenant_email") || msg.contains("already contains") {
        ApiError::conflict("Ya existe un usuario con ese correo en este negocio.")
    } else {
        tracing::error!(error = %e, "users: create failed");
        ApiError::service_unavailable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_normalized_lowercase_trimmed() {
        assert_eq!(
            normalize_email("  Admin@Demo.CL ").unwrap(),
            "admin@demo.cl"
        );
    }

    #[test]
    fn email_rejects_garbage() {
        assert!(normalize_email("").is_err());
        assert!(normalize_email("no-at-sign").is_err());
        assert!(normalize_email("a b@c.cl").is_err());
        assert!(normalize_email("missing@dotcom").is_err());
    }

    #[test]
    fn password_min_length_enforced() {
        assert!(validate_password("1234567").is_err());
        assert!(validate_password("12345678").is_ok());
    }

    #[test]
    fn roles_dedup_and_lowercase() {
        let out = validate_roles(vec!["Admin".into(), "admin".into(), "cashier".into()]).unwrap();
        assert_eq!(out, vec!["admin".to_string(), "cashier".to_string()]);
    }

    #[test]
    fn roles_reject_unknown_and_empty() {
        assert!(validate_roles(vec!["wizard".into()]).is_err());
        assert!(validate_roles(vec![]).is_err());
        assert!(validate_roles(vec!["   ".into()]).is_err());
    }

    #[test]
    fn admin_role_detection() {
        assert!(has_admin_role(&["owner".to_string()]));
        assert!(has_admin_role(&["admin".to_string()]));
        assert!(!has_admin_role(&[
            "cashier".to_string(),
            "pharmacist".to_string()
        ]));
    }
}

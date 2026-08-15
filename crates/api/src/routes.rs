use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

#[derive(Serialize)]
struct Me {
    sub: String,
    tenant_id: String,
    roles: Vec<String>,
    exp: i64,
}

#[derive(Deserialize)]
struct LoginRequest {
    /// Vacío = buscar el puesto por el correo (nube de feria).
    #[serde(default)]
    tenant: String,
    email: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    token_type: &'static str,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct TenantRow {
    id: surrealdb::sql::Thing,
}

#[derive(Debug, Deserialize)]
struct UserRow {
    id: surrealdb::sql::Thing,
    tenant: surrealdb::sql::Thing,
    password: String,
    roles: Vec<String>,
    #[serde(default)]
    active: Option<bool>,
}

pub fn router() -> Router<AppState> {
    // Canonical v1 routes. `/api/*` aliases kept one release for back-compat.
    Router::new()
        .route("/api/v1/me", get(me))
        .route("/api/v1/login", post(login))
        .route("/api/me", get(me))
        .route("/api/login", post(login))
}

async fn me(AuthUser(claims): AuthUser) -> Json<Me> {
    Json(Me {
        sub: claims.sub,
        tenant_id: claims.tenant_id,
        roles: claims.roles,
        exp: claims.exp,
    })
}

async fn login(
    State(s): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let db = s.db.as_ref().ok_or_else(ApiError::service_unavailable)?;
    let email = req.email.trim().to_lowercase();
    let slug = req.tenant.trim();

    let user = if slug.is_empty() {
        let mut uq = db
            .query(
                "SELECT id, tenant, password, roles, active FROM user \
                 WHERE email = $email LIMIT 2",
            )
            .bind(("email", email.clone()))
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "login: email lookup failed");
                ApiError::service_unavailable()
            })?;
        let users: Vec<UserRow> = uq.take(0).map_err(|e| {
            tracing::warn!(error = %e, "login: email decode failed");
            ApiError::service_unavailable()
        })?;
        match users.len() {
            0 => return Err(ApiError::bad_credentials()),
            1 => users.into_iter().next().unwrap(),
            _ => {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "NECESITA_NEGOCIO",
                    "Ese correo está en más de un puesto. Escribí el nombre corto.",
                ))
            }
        }
    } else {
        let mut tq = db
            .query("SELECT id FROM tenant WHERE slug = $slug LIMIT 1")
            .bind(("slug", slug.to_string()))
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "login: tenant lookup failed");
                ApiError::service_unavailable()
            })?;
        let tenant: Option<TenantRow> = tq.take(0).map_err(|e| {
            tracing::warn!(error = %e, "login: tenant decode failed");
            ApiError::service_unavailable()
        })?;
        let tenant = tenant.ok_or_else(ApiError::bad_credentials)?;

        let mut uq = db
            .query(
                "SELECT id, tenant, password, roles, active FROM user \
                 WHERE tenant = $tenant AND email = $email LIMIT 1",
            )
            .bind(("tenant", tenant.id.clone()))
            .bind(("email", email.clone()))
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "login: user lookup failed");
                ApiError::service_unavailable()
            })?;
        let user: Option<UserRow> = uq.take(0).map_err(|e| {
            tracing::warn!(error = %e, "login: user decode failed");
            ApiError::service_unavailable()
        })?;
        user.ok_or_else(ApiError::bad_credentials)?
    };

    if user.active == Some(false) {
        return Err(ApiError::bad_credentials());
    }

    let ok = auth::password::verify(&req.password, &user.password)
        .map_err(|_| ApiError::bad_credentials())?;
    if !ok {
        return Err(ApiError::bad_credentials());
    }

    let sub = user.id.to_string();
    let tenant_id = user.tenant.to_string();
    let token = auth::issue(&s.jwt, &sub, &tenant_id, user.roles.clone()).map_err(|e| {
        tracing::error!(error = %e, "login: issue jwt failed");
        ApiError::service_unavailable()
    })?;

    let claims = auth::verify(&s.jwt, &token).map_err(|e| {
        tracing::error!(error = %e, "login: re-verify own jwt failed");
        ApiError::service_unavailable()
    })?;
    let jti = uuid::Uuid::new_v4().to_string();
    let expires_at = chrono::DateTime::<chrono::Utc>::from_timestamp(claims.exp, 0)
        .ok_or_else(ApiError::service_unavailable)?;

    if let Err(e) = db
        .query(
            "CREATE session SET user = $user, tenant = $tenant, jti = $jti, \
             expires_at = $expires_at",
        )
        .bind(("user", user.id.clone()))
        .bind(("tenant", user.tenant.clone()))
        .bind(("jti", jti.clone()))
        .bind(("expires_at", expires_at))
        .await
    {
        tracing::warn!(error = %e, "login: session persist failed (token still issued)");
    }

    Ok(Json(LoginResponse {
        token,
        token_type: "Bearer",
        expires_in: s.jwt.ttl_seconds,
    }))
}

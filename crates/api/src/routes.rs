use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

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
    Router::new()
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
) -> Result<Json<LoginResponse>, LoginError> {
    let db = s.db.as_ref().ok_or(LoginError::Unavailable)?;

    let mut tq = db
        .query("SELECT id FROM tenant WHERE slug = $slug LIMIT 1")
        .bind(("slug", req.tenant.clone()))
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "login: tenant lookup failed");
            LoginError::Unavailable
        })?;
    let tenant: Option<TenantRow> = tq.take(0).map_err(|e| {
        tracing::warn!(error = %e, "login: tenant decode failed");
        LoginError::Unavailable
    })?;
    let tenant = tenant.ok_or(LoginError::BadCreds)?;

    let mut uq = db
        .query(
            "SELECT id, tenant, password, roles, active FROM user \
             WHERE tenant = $tenant AND email = $email LIMIT 1",
        )
        .bind(("tenant", tenant.id.clone()))
        .bind(("email", req.email.clone()))
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "login: user lookup failed");
            LoginError::Unavailable
        })?;
    let user: Option<UserRow> = uq.take(0).map_err(|e| {
        tracing::warn!(error = %e, "login: user decode failed");
        LoginError::Unavailable
    })?;
    let user = user.ok_or(LoginError::BadCreds)?;

    if user.active == Some(false) {
        return Err(LoginError::BadCreds);
    }

    let ok =
        auth::password::verify(&req.password, &user.password).map_err(|_| LoginError::BadCreds)?;
    if !ok {
        return Err(LoginError::BadCreds);
    }

    let sub = user.id.to_string();
    let tenant_id = user.tenant.to_string();
    let token = auth::issue(&s.jwt, &sub, &tenant_id, user.roles.clone()).map_err(|e| {
        tracing::error!(error = %e, "login: issue jwt failed");
        LoginError::Unavailable
    })?;

    let claims = auth::verify(&s.jwt, &token).map_err(|e| {
        tracing::error!(error = %e, "login: re-verify own jwt failed");
        LoginError::Unavailable
    })?;
    let jti = uuid::Uuid::new_v4().to_string();
    let expires_at = chrono::DateTime::<chrono::Utc>::from_timestamp(claims.exp, 0)
        .ok_or(LoginError::Unavailable)?;

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

#[derive(Debug)]
enum LoginError {
    BadCreds,
    Unavailable,
}

impl IntoResponse for LoginError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match self {
            LoginError::BadCreds => (StatusCode::UNAUTHORIZED, "invalid credentials"),
            LoginError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "service unavailable"),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

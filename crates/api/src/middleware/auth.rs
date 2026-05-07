use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser(pub auth::Claims);

impl std::ops::Deref for AuthUser {
    type Target = auth::Claims;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct AuthError {
    pub status: StatusCode,
    pub message: &'static str,
}

impl AuthError {
    pub const MISSING: Self = Self {
        status: StatusCode::UNAUTHORIZED,
        message: "missing bearer token",
    };
    pub const INVALID: Self = Self {
        status: StatusCode::UNAUTHORIZED,
        message: "invalid or expired token",
    };
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

impl<S> FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .ok_or(AuthError::MISSING)?;
        let claims = auth::verify(&app_state.jwt, token).map_err(|e| {
            tracing::debug!(error = %e, "jwt verification failed");
            AuthError::INVALID
        })?;
        Ok(AuthUser(claims))
    }
}

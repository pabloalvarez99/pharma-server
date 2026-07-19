use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts},
    response::{IntoResponse, Response},
};

use crate::error::ApiError;
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
pub struct AuthError(ApiError);

impl AuthError {
    pub fn missing() -> Self {
        Self(ApiError::unauthorized_missing_token())
    }
    pub fn invalid() -> Self {
        Self(ApiError::unauthorized_invalid_token())
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        self.0.into_response()
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
            .ok_or_else(AuthError::missing)?;
        let claims = auth::verify(&app_state.jwt, token).map_err(|e| {
            tracing::debug!(error = %e, "jwt verification failed");
            AuthError::invalid()
        })?;
        Ok(AuthUser(claims))
    }
}

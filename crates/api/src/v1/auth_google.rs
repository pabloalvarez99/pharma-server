//! Google Sign-In exchange (ADR-0022) — **stub**.
//!
//! Accepts the documented body shape and returns 501 until OAuth client ids
//! and JWKS verification are configured by ops. Never logs the `id_token`.

use axum::{extract::State, routing::post, Json, Router};

use crate::error::ApiError;
use crate::AppState;

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new().route(
        domain::google_identity::GOOGLE_SIGN_IN_PATH,
        post(exchange_stub),
    )
}

async fn exchange_stub(
    State(_s): State<AppState>,
    Json(body): Json<domain::google_identity::GoogleSignInRequest>,
) -> Result<Json<domain::google_identity::GoogleSignInResponse>, ApiError> {
    // Do not log body.id_token.
    let _ = body.id_token.len(); // touch so unused is intentional
    let _ = body.tenant;
    Err(ApiError::not_implemented())
}

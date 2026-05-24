//! Role-gating middleware.
//!
//! Usage on a router/route:
//! ```ignore
//! .route_layer(role::layer(state.clone(), &["admin", "owner"]))
//! ```
//!
//! It verifies the bearer JWT (same logic as [`super::auth::AuthUser`]) and
//! checks the token `roles` intersect the allowed set. Failures use the
//! standard error envelope.

use axum::{
    extract::{Request, State},
    http::header,
    middleware::{from_fn_with_state, Next},
    response::{IntoResponse, Response},
    Extension,
};
use tower::layer::util::Stack;

use crate::{error::ApiError, AppState};

/// Allowed-role list, injected as a request extension so the shared async
/// middleware fn can read it without monomorphizing per call site.
#[derive(Clone, Copy)]
pub struct AllowedRoles(pub &'static [&'static str]);

/// Build the stacked layer: inject `AllowedRoles`, then run the gate.
#[allow(clippy::type_complexity)]
pub fn layer(
    state: AppState,
    allowed: &'static [&'static str],
) -> Stack<
    Extension<AllowedRoles>,
    axum::middleware::FromFnLayer<
        fn(State<AppState>, Extension<AllowedRoles>, Request, Next) -> RoleGateFuture,
        AppState,
        (State<AppState>, Extension<AllowedRoles>, Request),
    >,
> {
    Stack::new(
        Extension(AllowedRoles(allowed)),
        from_fn_with_state(state, role_gate as fn(_, _, _, _) -> RoleGateFuture),
    )
}

type RoleGateFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>;

fn role_gate(
    State(s): State<AppState>,
    Extension(AllowedRoles(allowed)): Extension<AllowedRoles>,
    req: Request,
    next: Next,
) -> RoleGateFuture {
    Box::pin(async move {
        match check(&s, &req, allowed) {
            Ok(()) => next.run(req).await,
            Err(e) => e.into_response(),
        }
    })
}

fn check(state: &AppState, req: &Request, allowed: &[&str]) -> Result<(), ApiError> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(ApiError::unauthorized_missing_token)?;

    let claims = auth::verify(&state.jwt, token).map_err(|e| {
        tracing::debug!(error = %e, "role gate: jwt verify failed");
        ApiError::unauthorized_invalid_token()
    })?;

    if claims.roles.iter().any(|r| allowed.contains(&r.as_str())) {
        Ok(())
    } else {
        Err(ApiError::forbidden())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    fn state() -> AppState {
        AppState {
            started_at: chrono::Utc::now(),
            jwt: pharma_core::config::JwtConfig {
                secret: "test-secret".into(),
                issuer: "pharma-server".into(),
                ttl_seconds: 60,
            },
            db: None,
            metrics_token: None,
            node_identity: None,
            data_dir: None,
            license: std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(
                license::License::free_default(uuid::Uuid::nil()),
            )),
            license_path: None,
            rate_limit: None,
        }
    }

    fn req_with(token: Option<&str>) -> Request {
        let mut b = Request::builder().uri("/api/v1/x");
        if let Some(t) = token {
            b = b.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        b.body(Body::empty()).unwrap()
    }

    #[test]
    fn missing_token_rejected() {
        let err = check(&state(), &req_with(None), &["admin"]).unwrap_err();
        assert_eq!(err.code, "MISSING_TOKEN");
    }

    #[test]
    fn role_match_allows() {
        let s = state();
        let tok = auth::issue(&s.jwt, "user:1", "tenant:1", vec!["admin".into()]).unwrap();
        check(&s, &req_with(Some(&tok)), &["admin", "owner"]).expect("allowed");
    }

    #[test]
    fn role_mismatch_forbidden() {
        let s = state();
        let tok = auth::issue(&s.jwt, "user:1", "tenant:1", vec!["cashier".into()]).unwrap();
        let err = check(&s, &req_with(Some(&tok)), &["admin"]).unwrap_err();
        assert_eq!(err.code, "FORBIDDEN");
    }

    #[test]
    fn bad_token_invalid() {
        let s = state();
        let err = check(&s, &req_with(Some("garbage")), &["admin"]).unwrap_err();
        assert_eq!(err.code, "INVALID_TOKEN");
    }
}

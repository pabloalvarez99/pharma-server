//! Token-bucket rate limiting (per-tenant + per-IP).
//!
//! Two limiters live in [`RateLimitState`]:
//! * **Per-IP** — applied at the outermost layer (before auth), keyed by
//!   the `X-Forwarded-For` first hop or the remote socket address. Protects
//!   unauth endpoints (`/api/v1/login`, `/agent/inbox`, etc.) against
//!   credential-stuffing / brute force from a single source.
//! * **Per-tenant** — applied AFTER auth, keyed by the JWT `tenant_id` claim.
//!   Caps the aggregate request rate of any one tenant regardless of how many
//!   client IPs they connect from.
//!
//! Both limiters are in-memory (`governor::DefaultKeyedRateLimiter`, backed by
//! a `DashMap`); they never touch the DB and never block the response path.
//!
//! `/health/*` is exempt (monitoring uptime must not be throttled).
//!
//! On rejection the middleware returns `429 Too Many Requests` with the
//! `ApiError::rate_limited` envelope and a `Retry-After: <secs>` header.

use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{
    clock::{Clock, DefaultClock},
    DefaultKeyedRateLimiter, Quota, RateLimiter,
};

use crate::{error::ApiError, AppState};
use pharma_core::config::RateLimitConfig;

/// In-memory rate-limit state — two keyed token-bucket limiters.
///
/// Cheap to clone (each field is already `Arc`-wrapped internally by
/// `RateLimiter`, but we re-`Arc` the whole thing so `AppState` clones stay
/// O(1) and the limiter buckets are shared across all clones).
pub struct RateLimitState {
    pub cfg: RateLimitConfig,
    pub ip: Arc<DefaultKeyedRateLimiter<IpAddr>>,
    pub tenant: Arc<DefaultKeyedRateLimiter<String>>,
    clock: DefaultClock,
}

impl RateLimitState {
    /// Build limiters from config. Invalid (zero) values are clamped to 1 —
    /// the type system enforces `NonZeroU32`, but config deserialization may
    /// allow zeros and we want defensible behaviour instead of a panic.
    pub fn new(cfg: RateLimitConfig) -> Self {
        let ip_burst = NonZeroU32::new(cfg.ip_burst.max(1)).unwrap();
        let ip_per_min = NonZeroU32::new(cfg.ip_per_min.max(1)).unwrap();
        let tenant_burst = NonZeroU32::new(cfg.tenant_burst.max(1)).unwrap();
        let tenant_per_min = NonZeroU32::new(cfg.tenant_per_min.max(1)).unwrap();

        let ip_quota = Quota::per_minute(ip_per_min).allow_burst(ip_burst);
        let tenant_quota = Quota::per_minute(tenant_per_min).allow_burst(tenant_burst);

        Self {
            cfg,
            ip: Arc::new(RateLimiter::keyed(ip_quota)),
            tenant: Arc::new(RateLimiter::keyed(tenant_quota)),
            clock: DefaultClock::default(),
        }
    }

    fn enabled(&self) -> bool {
        self.cfg.enabled
    }
}

/// Paths exempted from rate limiting — health/liveness probes for external
/// monitoring (must always answer to detect outages).
fn is_exempt(path: &str) -> bool {
    path.starts_with("/health")
}

/// Extract the client IP from `X-Forwarded-For` (first hop) or the connection
/// remote address. Returns `None` if neither is available (unit tests using
/// `oneshot` without `into_make_service_with_connect_info`).
fn client_ip(req: &Request) -> Option<IpAddr> {
    if let Some(v) = req.headers().get("x-forwarded-for") {
        if let Ok(s) = v.to_str() {
            if let Some(first) = s.split(',').next() {
                if let Ok(ip) = first.trim().parse::<IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }
    req.extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|c| c.0.ip())
}

/// Round a duration up to whole seconds with a minimum of 1.
fn retry_after_secs(d: Duration) -> u64 {
    let s = d.as_secs();
    if d.subsec_nanos() > 0 || s == 0 {
        s + 1
    } else {
        s
    }
}

fn throttled_response(scope: &'static str, retry_after: u64) -> Response {
    let mut resp = ApiError::rate_limited(scope, retry_after).into_response();
    // Even if the conversion failed we still want the status to be 429.
    *resp.status_mut() = StatusCode::TOO_MANY_REQUESTS;
    if let Ok(v) = HeaderValue::from_str(&retry_after.to_string()) {
        resp.headers_mut().insert(header::RETRY_AFTER, v);
    }
    resp
}

// --- per-IP middleware ------------------------------------------------------

type Fut = std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>;

/// Layer constructor for the per-IP limiter.
#[allow(clippy::type_complexity)]
pub fn ip_layer(
    state: AppState,
) -> axum::middleware::FromFnLayer<
    fn(State<AppState>, Request, Next) -> Fut,
    AppState,
    (State<AppState>, Request),
> {
    axum::middleware::from_fn_with_state(state, ip_mw as fn(_, _, _) -> Fut)
}

fn ip_mw(State(s): State<AppState>, req: Request, next: Next) -> Fut {
    Box::pin(async move {
        let Some(rl) = s.rate_limit.as_ref() else {
            return next.run(req).await;
        };
        if !rl.enabled() || is_exempt(req.uri().path()) {
            return next.run(req).await;
        }
        let Some(ip) = client_ip(&req) else {
            // No IP available (tests). Skip — per-tenant limiter still runs.
            return next.run(req).await;
        };
        match rl.ip.check_key(&ip) {
            Ok(()) => next.run(req).await,
            Err(neg) => {
                let wait = neg.wait_time_from(rl.clock.now());
                throttled_response("ip", retry_after_secs(wait))
            }
        }
    })
}

// --- per-tenant middleware --------------------------------------------------

/// Layer constructor for the per-tenant limiter.
///
/// Runs AFTER auth: extracts the bearer JWT, verifies, and keys the bucket on
/// `Claims.tenant_id`. Unauth requests (no bearer or invalid) are passed
/// through — the per-IP limiter already covered them.
#[allow(clippy::type_complexity)]
pub fn tenant_layer(
    state: AppState,
) -> axum::middleware::FromFnLayer<
    fn(State<AppState>, Request, Next) -> Fut,
    AppState,
    (State<AppState>, Request),
> {
    axum::middleware::from_fn_with_state(state, tenant_mw as fn(_, _, _) -> Fut)
}

fn tenant_mw(State(s): State<AppState>, req: Request, next: Next) -> Fut {
    Box::pin(async move {
        let Some(rl) = s.rate_limit.as_ref() else {
            return next.run(req).await;
        };
        if !rl.enabled() || is_exempt(req.uri().path()) {
            return next.run(req).await;
        }
        let tenant_id = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .and_then(|t| auth::verify(&s.jwt, t).ok())
            .map(|c| c.tenant_id);

        let Some(tenant) = tenant_id else {
            return next.run(req).await;
        };
        match rl.tenant.check_key(&tenant) {
            Ok(()) => next.run(req).await,
            Err(neg) => {
                let wait = neg.wait_time_from(rl.clock.now());
                throttled_response("tenant", retry_after_secs(wait))
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exempt_paths() {
        assert!(is_exempt("/health"));
        assert!(is_exempt("/health/live"));
        assert!(is_exempt("/health/ready"));
        assert!(!is_exempt("/api/v1/login"));
        assert!(!is_exempt("/agent/inbox"));
    }

    #[test]
    fn retry_after_rounds_up() {
        assert_eq!(retry_after_secs(Duration::from_millis(1)), 1);
        assert_eq!(retry_after_secs(Duration::from_secs(1)), 1);
        assert_eq!(retry_after_secs(Duration::from_millis(1500)), 2);
        assert_eq!(retry_after_secs(Duration::from_secs(0)), 1);
    }

    #[test]
    fn state_clamps_zero_to_one() {
        // Should not panic.
        let _ = RateLimitState::new(RateLimitConfig {
            enabled: true,
            tenant_per_min: 0,
            tenant_burst: 0,
            ip_per_min: 0,
            ip_burst: 0,
        });
    }
}

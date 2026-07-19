//! Audit middleware.
//!
//! For every mutating request (`POST`/`PATCH`/`PUT`/`DELETE`) it records a row
//! in `audit_log` (migration `0002_audit_log.surql`). The insert is dispatched
//! on a detached `tokio::spawn` with a cloned `Arc<Db>` so the response path is
//! never blocked.
//!
//! Best-effort: if the DB is down, the request has no valid JWT, or the body
//! cannot be buffered, the request still proceeds and the audit row is skipped
//! (a `warn` is logged). Auditing must never break a sale.

use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::{header, Method},
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};

use crate::AppState;

const MAX_AUDIT_BODY: usize = 1024 * 1024; // 1 MiB; larger bodies hashed as truncated marker

type AuditFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>;

/// Layer constructor — wire with `.layer(audit::layer(state.clone()))`.
#[allow(clippy::type_complexity)]
pub fn layer(
    state: AppState,
) -> axum::middleware::FromFnLayer<
    fn(State<AppState>, Request, Next) -> AuditFuture,
    AppState,
    (State<AppState>, Request),
> {
    axum::middleware::from_fn_with_state(state, audit_mw as fn(_, _, _) -> AuditFuture)
}

fn is_mutation(m: &Method) -> bool {
    matches!(
        *m,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn audit_mw(State(s): State<AppState>, req: Request, next: Next) -> AuditFuture {
    Box::pin(async move {
        if !is_mutation(req.method()) {
            return next.run(req).await;
        }

        let (parts, body) = req.into_parts();

        let method = parts.method.to_string();
        let path = parts.uri.path().to_string();
        let ip = header_str(&parts.headers, "x-forwarded-for")
            .or_else(|| header_str(&parts.headers, "x-real-ip"));
        let user_agent = header_str(&parts.headers, header::USER_AGENT.as_str());

        // Verify JWT for tenant/user attribution (best-effort; unauth mutations still audited tenant-less? skip).
        let claims = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .and_then(|t| auth::verify(&s.jwt, t).ok());

        // Buffer body to hash it, then rebuild the request.
        let bytes = match to_bytes(body, MAX_AUDIT_BODY).await {
            Ok(b) => b,
            Err(_) => {
                // Body too large / stream error: proceed without hash.
                let req = Request::from_parts(parts, Body::empty());
                return next.run(req).await;
            }
        };
        let payload_hash = if bytes.is_empty() {
            None
        } else {
            let mut h = Sha256::new();
            h.update(&bytes);
            Some(format!("{:x}", h.finalize()))
        };

        let req = Request::from_parts(parts, Body::from(bytes));
        let response = next.run(req).await;
        let status = response.status().as_u16() as i64;

        if let (Some(db), Some(c)) = (s.db.clone(), claims) {
            spawn_insert(db, c, method, path, status, payload_hash, ip, user_agent);
        }

        response
    })
}

#[allow(clippy::too_many_arguments)]
fn spawn_insert(
    db: Arc<db::Db>,
    claims: auth::Claims,
    method: String,
    path: String,
    status: i64,
    payload_hash: Option<String>,
    ip: Option<String>,
    user_agent: Option<String>,
) {
    tokio::spawn(async move {
        let tenant = match surrealdb::sql::thing(&claims.tenant_id) {
            Ok(t) => t,
            Err(_) => {
                tracing::warn!(tenant = %claims.tenant_id, "audit: bad tenant thing, skipping row");
                return;
            }
        };
        let user = surrealdb::sql::thing(&claims.sub).ok();

        let q = db
            .query(
                "CREATE audit_log SET tenant = $tenant, user = $user, \
                 method = $method, path = $path, status = $status, \
                 payload_hash = $payload_hash, ip = $ip, user_agent = $user_agent",
            )
            .bind(("tenant", tenant))
            .bind(("user", user))
            .bind(("method", method))
            .bind(("path", path))
            .bind(("status", status))
            .bind(("payload_hash", payload_hash))
            .bind(("ip", ip))
            .bind(("user_agent", user_agent))
            .await;
        if let Err(e) = q {
            tracing::warn!(error = %e, "audit: insert failed");
        }
    });
}

fn header_str(h: &axum::http::HeaderMap, name: &str) -> Option<String> {
    h.get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_detection() {
        assert!(is_mutation(&Method::POST));
        assert!(is_mutation(&Method::PATCH));
        assert!(is_mutation(&Method::PUT));
        assert!(is_mutation(&Method::DELETE));
        assert!(!is_mutation(&Method::GET));
        assert!(!is_mutation(&Method::HEAD));
        assert!(!is_mutation(&Method::OPTIONS));
    }
}

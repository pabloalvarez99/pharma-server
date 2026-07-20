//! Bearer API-key auth for the Free Web storefront write seam (PR2, ADR-0020).
//!
//! PR3 mounts [`RequireApiKey`] on the public order routes: the storefront
//! sends `Authorization: Bearer rb_live_…`, we SHA-256 the presented key and
//! look it up in `api_key` (single key table, migrations 0026 + 0037). Every
//! non-authorized path — missing header, unknown key, inactive key, DB fault —
//! answers the same 401 `INVALID_API_KEY`, so probes cannot distinguish
//! "no such key" from "revoked" (fail-closed, no enumeration).
//!
//! Tenant binding is a separate step: the extractor returns the key's own
//! tenant in [`WebApiKeyCtx`]; handlers whose path slug resolves a tenant must
//! call [`ensure_key_matches_tenant`] so tenant A's key can never act on
//! tenant B (403 `SCOPE_DENIED`).
//!
//! The plaintext key is never logged and never echoed in error messages.

use std::sync::Arc;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header, request::Parts, StatusCode},
};
use surrealdb::sql::Thing;

use crate::error::ApiError;
use crate::AppState;

/// Verified storefront credential, injected by the public-key extractor.
#[derive(Clone, Debug)]
pub struct WebApiKeyCtx {
    pub tenant: Thing,
    pub key_id: Thing,
    pub scopes: Vec<String>,
    /// Per-key shared secret for PR3 request signatures. Empty for 0026-era
    /// CLI-minted keys (which predate `hmac_secret`).
    pub hmac_secret: String,
}

/// Extractor: reads `Authorization: Bearer rb_live_…`, sha256-lookup in
/// `api_key`, rejects 401 `INVALID_API_KEY` when missing/unknown/inactive.
/// Fire-and-forget stamps `last_used_at`.
pub struct RequireApiKey(pub WebApiKeyCtx);

fn invalid_api_key() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "INVALID_API_KEY",
        "Credencial de storefront inválida.",
    )
}

fn scope_denied() -> ApiError {
    ApiError::new(
        StatusCode::FORBIDDEN,
        "SCOPE_DENIED",
        "Alcance no autorizado.",
    )
}

/// 403 `SCOPE_DENIED` when the key does not carry `scope`.
pub fn require_scope(ctx: &WebApiKeyCtx, scope: &str) -> Result<(), ApiError> {
    if crate::api_key::scopes_grant(&ctx.scopes, scope) {
        Ok(())
    } else {
        Err(scope_denied())
    }
}

/// 403 `SCOPE_DENIED` when the key belongs to a different tenant than the one
/// the request path resolved. Same code as a scope failure on purpose: a
/// cross-tenant probe learns nothing beyond "not allowed".
pub fn ensure_key_matches_tenant(ctx: &WebApiKeyCtx, tenant: &Thing) -> Result<(), ApiError> {
    if &ctx.tenant == tenant {
        Ok(())
    } else {
        Err(scope_denied())
    }
}

impl<S> FromRequestParts<S> for RequireApiKey
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let db: Arc<db::Db> = app_state
            .db
            .clone()
            .ok_or_else(ApiError::service_unavailable)?;

        let presented = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(invalid_api_key)?;

        let hash = crate::api_key::hash_key(presented);
        let found = domain::web_keys::find_by_hash(&db, &hash)
            .await
            .map_err(|e| {
                // Fail closed on DB faults; never log the presented key.
                tracing::warn!(error = %e, "public auth: key lookup failed; denying");
                ApiError::service_unavailable()
            })?;
        let key = found.ok_or_else(invalid_api_key)?;

        let stamp_db = db.clone();
        let stamp_id = key.id.clone();
        tokio::spawn(async move {
            if let Err(e) = domain::web_keys::touch_last_used(&stamp_db, &stamp_id).await {
                tracing::debug!(error = %e, "public auth: last_used_at stamp failed");
            }
        });

        Ok(RequireApiKey(WebApiKeyCtx {
            tenant: key.tenant,
            key_id: key.id,
            scopes: key.scopes,
            hmac_secret: key.hmac_secret.unwrap_or_default(),
        }))
    }
}

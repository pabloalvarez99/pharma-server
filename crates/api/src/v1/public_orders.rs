//! Public web-push order ingest (ADR-0012 pattern 2).
//!
//! `POST /api/v1/public/orders/web`
//!
//! Receives orders placed on the external Tu Farmacia website and writes them
//! into the on-prem ERP with `channel='web'` (status `reserved`, no stock
//! decrement). **No JWT** — the caller authenticates by signing the raw
//! request body with the shared HMAC-SHA256 secret.
//!
//! ## Wire contract
//!
//! | Header                | Required | Meaning                                  |
//! |-----------------------|----------|------------------------------------------|
//! | `X-Pharma-Signature`  | yes      | `sha256=<hex>` = HMAC-SHA256(secret, raw body) |
//! | `X-Pharma-Timestamp`  | yes      | RFC3339 send time; rejected if drift > 5 min |
//! | `X-Pharma-Tenant`     | yes      | Tenant slug the order belongs to         |
//!
//! ## Security model
//!
//! 1. **Opt-in.** `enabled = false` (default) OR an empty `hmac_secret` makes
//!    the route return a uniform 404 — a probe cannot tell the feature exists
//!    (ADR-0005 "no surprise public exposure").
//! 2. **Authenticity.** The body is signed; an attacker without the secret
//!    cannot forge or mutate a request (any body change breaks the MAC).
//!    Comparison is constant-time (`hmac::Mac::verify_slice`).
//! 3. **Replay safety** comes from `external_order_id` idempotency in
//!    [`domain::sales::web_order::ingest`]: a captured request replayed verbatim
//!    re-resolves to the *same* order with no second write. The timestamp drift
//!    check is freshness defense-in-depth (bounds clock skew / very delayed
//!    delivery), not the primary replay defense.
//! 4. **Bounded body.** 64 KiB cap (≈ hundreds of line items) → 413 over limit.
//! 5. **Rate limited.** Per-IP token bucket (reuses [`crate::rate_limit`]).

use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use surrealdb::sql::Thing;

use domain::sales::web_order::{self, WebOrderRequest, WebOrderStatus};

use crate::error::ApiError;
use crate::AppState;

type HmacSha256 = Hmac<Sha256>;

/// Max signed body. Orders with ~hundreds of line items stay well under this.
const MAX_BODY_BYTES: usize = 64 * 1024;
const SIG_HEADER: &str = "x-pharma-signature";
const TS_HEADER: &str = "x-pharma-timestamp";
const TENANT_HEADER: &str = "x-pharma-tenant";
/// Accept timestamps within ±5 min of now (ADR-0012/0013 drift window).
const MAX_SKEW_SECS: i64 = 300;

/// Build the public web-orders sub-router. Layers (innermost → outermost):
/// 1. handler — 404 when disabled, 401 on bad signature, 2xx on success.
/// 2. body limit — 413 above [`MAX_BODY_BYTES`].
/// 3. per-IP rate limit — falls through when `state.rate_limit` is `None`.
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/public/orders/web", post(ingest_web_order))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .route_layer(crate::rate_limit::ip_layer(state))
}

async fn ingest_web_order(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // 1. Opt-in gate. Disabled or no secret → uniform 404 (hide the feature).
    let cfg = &state.public_orders;
    if !cfg.enabled || cfg.hmac_secret.is_empty() {
        return ApiError::not_found().into_response();
    }
    let Some(db) = state.db.clone() else {
        return ApiError::service_unavailable().into_response();
    };

    // 2. Required headers.
    let (Some(sig), Some(ts_raw), Some(tenant_slug)) = (
        header_str(&headers, SIG_HEADER),
        header_str(&headers, TS_HEADER),
        header_str(&headers, TENANT_HEADER),
    ) else {
        return signature_error("SIGNATURE_MISSING", "Falta firma o cabeceras requeridas.");
    };

    // 3. Authenticity: HMAC-SHA256 over the raw body, constant-time compare.
    if !verify_signature(cfg.hmac_secret.as_bytes(), &body, sig) {
        return signature_error("SIGNATURE_INVALID", "Firma inválida.");
    }

    // 4. Freshness (defense-in-depth; replay safety is idempotency, see step 7).
    match drift_secs(ts_raw) {
        Some(skew) if skew.abs() <= MAX_SKEW_SECS => {}
        _ => {
            return signature_error(
                "SIGNATURE_REPLAY_REJECTED",
                "Marca de tiempo fuera del rango permitido.",
            )
        }
    }

    // 5. Resolve tenant slug → record id. The signature already proved the
    //    caller holds the secret, so an unknown slug is a plain 404.
    let tenant = match resolve_tenant(&db, tenant_slug).await {
        Ok(Some(t)) => t,
        Ok(None) => return ApiError::not_found().into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "public orders: tenant resolve failed");
            return ApiError::service_unavailable().into_response();
        }
    };

    // 5b. Optional scoped API-key gate (ADR-0014 L1), on TOP of the HMAC body
    //     signature. Fail-closed: when required, no valid `orders:write` key for
    //     this tenant → reject (401/403), never falls open.
    if cfg.require_api_key {
        if let Some(deny) =
            crate::api_key::guard(&db, &tenant, &headers, crate::api_key::SCOPE_ORDERS_WRITE).await
        {
            return deny;
        }
    }

    // 6. Parse the (already authenticated) body.
    let req: WebOrderRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return ApiError::invalid(format!("JSON inválido: {e}")).into_response(),
    };

    // 7. Ingest. Idempotent on (tenant, channel='web', external_order_id):
    //    a replay returns the existing order with no second write.
    match web_order::ingest(db.as_ref(), &tenant, req).await {
        Ok(resp) => {
            let status = match resp.status {
                WebOrderStatus::Created => StatusCode::CREATED,
                WebOrderStatus::IdempotentReplay => StatusCode::OK,
            };
            (status, Json(resp)).into_response()
        }
        Err(e) => ApiError::from(e).into_response(),
    }
}

/// Trimmed, non-empty header value or `None`.
fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn signature_error(code: &'static str, message: &str) -> Response {
    ApiError::new(StatusCode::UNAUTHORIZED, code, message.to_string()).into_response()
}

/// Verify `supplied` (`sha256=<hex>` or bare hex) against HMAC-SHA256 of `body`.
/// Constant-time: `hmac::Mac::verify_slice` compares MACs without early-out.
fn verify_signature(secret: &[u8], body: &[u8], supplied: &str) -> bool {
    let hex_sig = supplied.strip_prefix("sha256=").unwrap_or(supplied);
    let Ok(sig_bytes) = hex::decode(hex_sig) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&sig_bytes).is_ok()
}

/// `now - ts` in seconds for an RFC3339 timestamp, or `None` if unparseable.
fn drift_secs(ts: &str) -> Option<i64> {
    let parsed = chrono::DateTime::parse_from_rfc3339(ts).ok()?;
    Some((chrono::Utc::now() - parsed.with_timezone(&chrono::Utc)).num_seconds())
}

async fn resolve_tenant(db: &Arc<db::Db>, slug: &str) -> Result<Option<Thing>, surrealdb::Error> {
    #[derive(serde::Deserialize)]
    struct TenantRow {
        id: Thing,
    }
    let mut r = db
        .query("SELECT id FROM tenant WHERE slug = $slug LIMIT 1")
        .bind(("slug", slug.to_string()))
        .await?;
    let row: Option<TenantRow> = r.take(0)?;
    Ok(row.map(|r| r.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign(secret: &[u8], body: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(secret).unwrap();
        mac.update(body);
        format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
    }

    #[test]
    fn verify_accepts_matching_signature() {
        let body = br#"{"hello":"world"}"#;
        let sig = sign(b"secret", body);
        assert!(verify_signature(b"secret", body, &sig));
    }

    #[test]
    fn verify_accepts_bare_hex_without_prefix() {
        let body = b"abc";
        let full = sign(b"k", body);
        let bare = full.strip_prefix("sha256=").unwrap();
        assert!(verify_signature(b"k", body, bare));
    }

    #[test]
    fn verify_rejects_wrong_secret() {
        let body = b"abc";
        let sig = sign(b"right", body);
        assert!(!verify_signature(b"wrong", body, &sig));
    }

    #[test]
    fn verify_rejects_tampered_body() {
        let sig = sign(b"k", b"original");
        assert!(!verify_signature(b"k", b"tampered", &sig));
    }

    #[test]
    fn verify_rejects_non_hex() {
        assert!(!verify_signature(b"k", b"x", "sha256=not-hex!!"));
    }

    #[test]
    fn drift_within_window() {
        let now = chrono::Utc::now().to_rfc3339();
        assert!(drift_secs(&now).unwrap().abs() <= MAX_SKEW_SECS);
    }

    #[test]
    fn drift_rejects_stale_timestamp() {
        let old = (chrono::Utc::now() - chrono::Duration::seconds(MAX_SKEW_SECS + 60)).to_rfc3339();
        assert!(drift_secs(&old).unwrap() > MAX_SKEW_SECS);
    }

    #[test]
    fn drift_none_for_garbage() {
        assert!(drift_secs("not-a-timestamp").is_none());
    }
}

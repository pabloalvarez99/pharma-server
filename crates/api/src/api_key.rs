//! Scoped API-key auth for the public DSS storefront seam (ADR-0014 L1).
//!
//! The two public endpoints — `GET /api/v1/public/catalog` (Patrón A) and
//! `POST /api/v1/public/orders/web` (Patrón C) — are opt-in. When the operator
//! additionally turns on `require_api_key`, every request must present a
//! tenant-scoped key in the `X-API-Key` header carrying the scope the route
//! needs:
//!
//! | Route                       | Required scope  |
//! |-----------------------------|-----------------|
//! | `GET  /public/catalog`      | [`SCOPE_CATALOG_READ`] |
//! | `POST /public/orders/web`   | [`SCOPE_ORDERS_WRITE`] (on top of the HMAC body signature) |
//!
//! ## Fail-closed by construction
//!
//! [`guard`] returns `Some(error_response)` for **every** non-authorized path —
//! missing header, unknown key, wrong tenant, missing scope, AND any DB error.
//! A handler that wires it as `if let Some(deny) = guard(..).await { return deny }`
//! cannot accidentally fall open. The key is matched under the already-resolved
//! tenant (`api_key.tenant = $t`), so one tenant's key can never read another
//! tenant's catalog.
//!
//! ## Storage
//!
//! Only `SHA-256(plaintext)` hex is persisted (migration 0026). The plaintext is
//! shown once at mint time (`pharma api-key create`) and never stored — a leaked
//! DB cannot be replayed against the seam.

use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};
use surrealdb::sql::Thing;

use crate::error::ApiError;

/// Scope that grants reading the scrubbed public catalog.
pub const SCOPE_CATALOG_READ: &str = "catalog:read";
/// Scope that grants pushing a web order.
pub const SCOPE_ORDERS_WRITE: &str = "orders:write";

/// Header carrying the plaintext API key.
pub const HEADER: &str = "x-api-key";

/// Hash a plaintext key the way it is stored / looked up: lowercase hex of
/// SHA-256. Pure — the single source of truth shared by mint (CLI) and verify.
pub fn hash_key(plaintext: &str) -> String {
    let mut h = Sha256::new();
    h.update(plaintext.as_bytes());
    hex::encode(h.finalize())
}

/// Whether `granted` scopes satisfy `required`. Plain membership today; kept as
/// a function so a future wildcard (`catalog:*`) lives in one place. Pure.
pub fn scopes_grant(granted: &[String], required: &str) -> bool {
    granted.iter().any(|s| s == required)
}

/// Outcome of matching a presented key against the store. Pure data; the HTTP
/// mapping lives in [`guard`].
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Key found for this tenant, active, and carries the required scope.
    Authorized,
    /// No `X-API-Key` header (or empty).
    Missing,
    /// Header present but no active key for this tenant matches the hash.
    Unknown,
    /// Key matched but lacks the required scope.
    MissingScope,
}

#[derive(serde::Deserialize)]
struct KeyRow {
    scopes: Vec<String>,
}

/// Match a presented key for `tenant` and decide whether it carries
/// `required_scope`. Returns a [`Decision`]; the only `Err` is a real DB fault
/// (mapped to 503 by [`guard`], never falling open).
pub async fn authorize(
    db: &db::Db,
    tenant: &Thing,
    presented: Option<&str>,
    required_scope: &str,
) -> Result<Decision, surrealdb::Error> {
    let Some(key) = presented.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(Decision::Missing);
    };
    let hash = hash_key(key);
    let mut r = db
        .query(
            "SELECT scopes FROM api_key \
             WHERE tenant = $t AND key_hash = $h AND active = true LIMIT 1",
        )
        .bind(("t", tenant.clone()))
        .bind(("h", hash))
        .await?
        .check()?;
    let row: Option<KeyRow> = r.take(0)?;
    let Some(row) = row else {
        return Ok(Decision::Unknown);
    };
    if scopes_grant(&row.scopes, required_scope) {
        Ok(Decision::Authorized)
    } else {
        Ok(Decision::MissingScope)
    }
}

/// Fail-closed gate for a public handler. Returns `None` when the caller is
/// authorized, or `Some(response)` to short-circuit:
///   · missing / unknown key → 401 `API_KEY_REQUIRED` / `API_KEY_INVALID`
///   · key without the scope  → 403 `API_KEY_SCOPE`
///   · DB error               → 503 (never falls open)
///
/// Errors are uniform Spanish messages; codes are English for devs.
pub async fn guard(
    db: &db::Db,
    tenant: &Thing,
    headers: &HeaderMap,
    required_scope: &str,
) -> Option<Response> {
    let presented = headers.get(HEADER).and_then(|v| v.to_str().ok());
    match authorize(db, tenant, presented, required_scope).await {
        Ok(Decision::Authorized) => None,
        Ok(Decision::Missing) => Some(
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "API_KEY_REQUIRED",
                "Falta la clave de API (cabecera X-API-Key).",
            )
            .into_response(),
        ),
        Ok(Decision::Unknown) => Some(
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "API_KEY_INVALID",
                "Clave de API inválida.",
            )
            .into_response(),
        ),
        Ok(Decision::MissingScope) => Some(
            ApiError::new(
                StatusCode::FORBIDDEN,
                "API_KEY_SCOPE",
                "La clave de API no tiene el permiso requerido.",
            )
            .into_response(),
        ),
        Err(e) => {
            tracing::warn!(error = %e, "api key: verify failed; denying (fail-closed)");
            Some(ApiError::service_unavailable().into_response())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_sha256_hex() {
        // SHA-256("abc") known vector.
        assert_eq!(
            hash_key("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Deterministic + 64 hex chars.
        assert_eq!(hash_key("pk_live_xyz").len(), 64);
        assert_eq!(hash_key("k"), hash_key("k"));
        assert_ne!(hash_key("a"), hash_key("b"));
    }

    #[test]
    fn scope_membership() {
        let granted = vec!["catalog:read".to_string(), "orders:write".to_string()];
        assert!(scopes_grant(&granted, SCOPE_CATALOG_READ));
        assert!(scopes_grant(&granted, SCOPE_ORDERS_WRITE));
        assert!(!scopes_grant(&granted, "admin:all"));
        assert!(!scopes_grant(&[], SCOPE_CATALOG_READ));
    }

    #[tokio::test]
    async fn authorize_flow_against_memdb() {
        use surrealdb::engine::local::Mem;
        use surrealdb::Surreal;

        let db: db::Db = Surreal::new::<Mem>(()).await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db.query("DEFINE TABLE tenant SCHEMALESS; DEFINE TABLE api_key SCHEMALESS;")
            .await
            .unwrap();
        // Two tenants; a key for t1 with catalog:read only.
        db.query("CREATE tenant:t1 SET slug='t1'; CREATE tenant:t2 SET slug='t2';")
            .await
            .unwrap();
        let plain = "pk_test_secret";
        db.query("CREATE api_key SET tenant=$t, key_hash=$h, scopes=['catalog:read'], active=true")
            .bind(("t", Thing::from(("tenant", "t1"))))
            .bind(("h", hash_key(plain)))
            .await
            .unwrap();

        let t1 = Thing::from(("tenant", "t1"));
        let t2 = Thing::from(("tenant", "t2"));

        // Happy path.
        assert_eq!(
            authorize(&db, &t1, Some(plain), SCOPE_CATALOG_READ)
                .await
                .unwrap(),
            Decision::Authorized
        );
        // Right key, wrong scope.
        assert_eq!(
            authorize(&db, &t1, Some(plain), SCOPE_ORDERS_WRITE)
                .await
                .unwrap(),
            Decision::MissingScope
        );
        // Valid key but probing another tenant → Unknown (no cross-tenant read).
        assert_eq!(
            authorize(&db, &t2, Some(plain), SCOPE_CATALOG_READ)
                .await
                .unwrap(),
            Decision::Unknown
        );
        // Wrong key.
        assert_eq!(
            authorize(&db, &t1, Some("nope"), SCOPE_CATALOG_READ)
                .await
                .unwrap(),
            Decision::Unknown
        );
        // No header.
        assert_eq!(
            authorize(&db, &t1, None, SCOPE_CATALOG_READ).await.unwrap(),
            Decision::Missing
        );
        assert_eq!(
            authorize(&db, &t1, Some("   "), SCOPE_CATALOG_READ)
                .await
                .unwrap(),
            Decision::Missing
        );
    }

    #[tokio::test]
    async fn inactive_key_is_unknown() {
        use surrealdb::engine::local::Mem;
        use surrealdb::Surreal;

        let db: db::Db = Surreal::new::<Mem>(()).await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db.query("CREATE tenant:t1 SET slug='t1'").await.unwrap();
        let plain = "pk_disabled";
        db.query(
            "CREATE api_key SET tenant=$t, key_hash=$h, scopes=['catalog:read'], active=false",
        )
        .bind(("t", Thing::from(("tenant", "t1"))))
        .bind(("h", hash_key(plain)))
        .await
        .unwrap();
        assert_eq!(
            authorize(
                &db,
                &Thing::from(("tenant", "t1")),
                Some(plain),
                SCOPE_CATALOG_READ
            )
            .await
            .unwrap(),
            Decision::Unknown
        );
    }
}

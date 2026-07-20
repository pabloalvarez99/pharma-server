//! Free Web storefront credentials (PR2, ADR-0020) — service over the
//! `api_key` table (migrations 0026 + 0037). Shares storage with the
//! CLI-minted DSS seam keys; PR2 adds `key_prefix` (listable) and
//! `hmac_secret` (PR3 request signatures) plus the admin CRUD lifecycle.
//!
//! The api layer generates and hashes the plaintext (single hash source:
//! `api::api_key::hash_key`); this module only stores/reads hashes and the
//! HMAC secret — the plaintext key never crosses into domain, so it cannot
//! leak through logs or errors here.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use utoipa::ToSchema;

use crate::errors::{DomainError, DomainResult};

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Scopes a storefront key may carry. Mirrors the seam verifier's constants
/// (`api::api_key::SCOPE_*`) and the CLI's `VALID_SCOPES`.
pub const ALLOWED_SCOPES: &[&str] = &["catalog:read", "orders:write"];

/// Default scope set for a new storefront key (migration 0026 default).
pub fn default_scopes() -> Vec<String> {
    ALLOWED_SCOPES.iter().map(|s| s.to_string()).collect()
}

/// Freshly generated credential material — already hashed at the api layer.
/// `hmac_secret` is stored verbatim: the server must recompute signatures
/// with it on every PR3 order request (it is a shared secret, not a password).
pub struct NewKeyMaterial {
    pub key_prefix: String,
    pub key_hash: String,
    pub hmac_secret: String,
}

/// Listing/CRUD row. Never carries `key_hash` nor `hmac_secret` — the SELECT
/// projections below are the enforcement point.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WebKeyDto {
    /// Full record id, e.g. `api_key:x1y2z3`.
    pub id: String,
    pub name: String,
    /// First chars of the plaintext (incl. `rb_live_`) for display. `None`
    /// for 0026-era CLI-minted keys.
    pub key_prefix: Option<String>,
    pub scopes: Vec<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct KeyRow {
    id: Thing,
    label: Option<String>,
    key_prefix: Option<String>,
    scopes: Vec<String>,
    active: bool,
    created_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

impl From<KeyRow> for WebKeyDto {
    fn from(r: KeyRow) -> Self {
        Self {
            id: r.id.to_string(),
            name: r.label.unwrap_or_default(),
            key_prefix: r.key_prefix,
            scopes: r.scopes,
            active: r.active,
            created_at: r.created_at,
            last_used_at: r.last_used_at,
        }
    }
}

/// Auth-time view of a matched key (bearer lookup by hash).
#[derive(Debug, Deserialize)]
pub struct KeyAuth {
    pub id: Thing,
    pub tenant: Thing,
    pub scopes: Vec<String>,
    pub hmac_secret: Option<String>,
}

/// Columns safe to return to the admin UI — everything except the hash and
/// the HMAC secret.
const LIST_COLS: &str = "id, label, key_prefix, scopes, active, created_at, last_used_at";

fn validate(name: &str, scopes: &[String]) -> DomainResult<()> {
    if name.trim().is_empty() {
        return Err(DomainError::Invalid("name requerido".into()));
    }
    if scopes.is_empty() {
        return Err(DomainError::Invalid("scopes requeridos".into()));
    }
    for s in scopes {
        if !ALLOWED_SCOPES.contains(&s.as_str()) {
            return Err(DomainError::Invalid(format!(
                "scope inválido '{s}'; válidos: {ALLOWED_SCOPES:?}"
            )));
        }
    }
    Ok(())
}

pub async fn create(
    db: &Db,
    tenant: &Thing,
    name: &str,
    scopes: &[String],
    mat: &NewKeyMaterial,
) -> DomainResult<WebKeyDto> {
    validate(name, scopes)?;
    let q = format!(
        "CREATE api_key SET tenant = $t, label = $name, scopes = $scopes, \
         key_prefix = $prefix, key_hash = $hash, hmac_secret = $secret, \
         active = true RETURN {LIST_COLS}"
    );
    let mut r = db
        .query(q)
        .bind(("t", tenant.clone()))
        .bind(("name", name.to_string()))
        .bind(("scopes", scopes.to_vec()))
        .bind(("prefix", mat.key_prefix.clone()))
        .bind(("hash", mat.key_hash.clone()))
        .bind(("secret", mat.hmac_secret.clone()))
        .await?
        .check()?;
    let row: Option<KeyRow> = r.take(0)?;
    row.map(Into::into).ok_or(DomainError::NotFound)
}

pub async fn list(db: &Db, tenant: &Thing) -> DomainResult<Vec<WebKeyDto>> {
    let q = format!("SELECT {LIST_COLS} FROM api_key WHERE tenant = $t ORDER BY created_at DESC");
    let mut r = db.query(q).bind(("t", tenant.clone())).await?;
    let rows: Vec<KeyRow> = r.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Deactivate one key row. `require_active` narrows to live keys (rotate);
/// revoke flips regardless so revoking twice stays a 204.
///
/// Retried on transient SurrealKv write-write conflicts: the extractor's
/// fire-and-forget `last_used_at` stamp touches the same record, so a
/// revoke/rotate racing a storefront request would otherwise surface a
/// spurious 503 (same policy as the sale write path).
async fn deactivate(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
    require_active: bool,
) -> DomainResult<Option<KeyRow>> {
    let active_cond = if require_active {
        " AND active = true"
    } else {
        ""
    };
    let q = format!(
        "UPDATE api_key SET active = false WHERE id = $id AND tenant = $t{active_cond} \
         RETURN AFTER"
    );
    let mut attempt = 0u32;
    loop {
        let res: DomainResult<Option<KeyRow>> = async {
            let mut r = db
                .query(&q)
                .bind(("id", id.clone()))
                .bind(("t", tenant.clone()))
                .await?;
            Ok(r.take(0)?)
        }
        .await;
        match res {
            Err(e) if e.is_retryable_db_conflict() && attempt < 3 => {
                attempt += 1;
                tokio::time::sleep(std::time::Duration::from_millis(15 * u64::from(attempt))).await;
            }
            r => return r,
        }
    }
}

/// Soft-revoke: `active = false`. Returns `false` when no key with that id
/// belongs to the tenant (caller maps to 404 — no cross-tenant probing).
pub async fn revoke(db: &Db, tenant: &Thing, id: &Thing) -> DomainResult<bool> {
    Ok(deactivate(db, tenant, id, false).await?.is_some())
}

/// Rotate: the old key goes inactive FIRST (fail-safe direction — a crash
/// between the two writes leaves the tenant with a revoked key, never two
/// live ones), then a fresh row is created with the same name + scopes.
pub async fn rotate(
    db: &Db,
    tenant: &Thing,
    id: &Thing,
    mat: &NewKeyMaterial,
) -> DomainResult<WebKeyDto> {
    let old = deactivate(db, tenant, id, true)
        .await?
        .ok_or(DomainError::NotFound)?;
    create(db, tenant, &old.label.unwrap_or_default(), &old.scopes, mat).await
}

/// Bearer lookup: hash → active key, across tenants (`key_hash` is globally
/// unique, index 0026). Tenant binding is enforced afterwards by the caller
/// (`ensure_key_matches_tenant`).
pub async fn find_by_hash(db: &Db, key_hash: &str) -> DomainResult<Option<KeyAuth>> {
    let mut r = db
        .query(
            "SELECT id, tenant, scopes, hmac_secret FROM api_key \
             WHERE key_hash = $h AND active = true LIMIT 1",
        )
        .bind(("h", key_hash.to_string()))
        .await?;
    let row: Option<KeyAuth> = r.take(0)?;
    Ok(row)
}

/// Best-effort usage stamp; callers fire-and-forget.
pub async fn touch_last_used(db: &Db, id: &Thing) -> DomainResult<()> {
    db.query("UPDATE api_key SET last_used_at = time::now() WHERE id = $id")
        .bind(("id", id.clone()))
        .await?;
    Ok(())
}

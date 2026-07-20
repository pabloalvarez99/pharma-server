//! Free Web admin — storefront API keys CRUD (PR2, ADR-0020). JWT + role
//! admin/owner.
//!
//! ```text
//! POST   /api/v1/admin/web/keys              → 201 plaintext key + hmac_secret (ONCE)
//! GET    /api/v1/admin/web/keys              → listing (never hash nor secret)
//! POST   /api/v1/admin/web/keys/{id}/rotate  → 200 fresh plaintext (old key dead)
//! DELETE /api/v1/admin/web/keys/{id}         → 204 (active = false)
//! ```
//!
//! The plaintext key and HMAC secret exist only in the create/rotate response
//! body — the DB stores `key_hash` = SHA-256(key) plus the secret, and neither
//! is ever logged (Law 1/2 of the Free Web plan). SHA-256 (not argon2) is
//! correct here: keys carry 256 bits of entropy and are verified per-request.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use utoipa::ToSchema;

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::role::admin_plus;
use crate::AppState;

use domain::web_keys::{self, NewKeyMaterial, WebKeyDto};

fn tenant_of(claims: &auth::Claims) -> Result<Thing, ApiError> {
    surrealdb::sql::thing(&claims.tenant_id).map_err(|_| ApiError::unauthorized_invalid_token())
}

fn db_of(s: &AppState) -> Result<Arc<db::Db>, ApiError> {
    s.db.clone().ok_or_else(ApiError::service_unavailable)
}

/// Path id → `api_key` Thing. Accepts `api_key:xyz` or the bare `xyz`.
fn key_id_of(raw: &str) -> Thing {
    surrealdb::sql::thing(raw)
        .ok()
        .filter(|t| t.tb == "api_key")
        .unwrap_or_else(|| Thing::from(("api_key", raw)))
}

/// 256-bit random hex (two UUIDv4) behind a recognizable prefix.
fn mint(prefix: &str) -> String {
    format!(
        "{prefix}{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Generate a full credential set: plaintext key `rb_live_…`, its stored
/// SHA-256 hash + 12-char display prefix, and the `whsec_…` HMAC secret.
fn mint_credentials() -> (String, NewKeyMaterial) {
    let key = mint("rb_live_");
    let mat = NewKeyMaterial {
        key_prefix: key.chars().take(12).collect(),
        key_hash: crate::api_key::hash_key(&key),
        hmac_secret: mint("whsec_"),
    };
    (key, mat)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateKeyRequest {
    pub name: String,
    /// Defaults to `["catalog:read", "orders:write"]`.
    pub scopes: Option<Vec<String>>,
}

/// Create/rotate response — the ONLY place the plaintext key and HMAC secret
/// ever appear.
#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedKeyResponse {
    pub id: String,
    pub name: String,
    /// Plaintext `rb_live_…` key. Shown once; store it now.
    pub key: String,
    /// Plaintext `whsec_…` signing secret. Shown once; store it now.
    pub hmac_secret: String,
    pub key_prefix: Option<String>,
    pub scopes: Vec<String>,
}

impl CreatedKeyResponse {
    fn from_parts(dto: WebKeyDto, key: String, hmac_secret: String) -> Self {
        Self {
            id: dto.id,
            name: dto.name,
            key,
            hmac_secret,
            key_prefix: dto.key_prefix,
            scopes: dto.scopes,
        }
    }
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/web/keys", post(create_key).get(list_keys))
        .route("/api/v1/admin/web/keys/{id}/rotate", post(rotate_key))
        .route("/api/v1/admin/web/keys/{id}", delete(revoke_key))
        .route_layer(crate::role::layer(state, admin_plus()))
}

/// Crear clave de storefront. El plaintext y el secreto HMAC se muestran UNA
/// sola vez en esta respuesta.
#[utoipa::path(post, path = "/api/v1/admin/web/keys", tag = "AdminWeb",
    request_body = CreateKeyRequest,
    responses(
        (status = 201, body = CreatedKeyResponse),
        (status = 400, body = crate::error::ErrorEnvelope),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn create_key(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<CreateKeyRequest>,
) -> Result<(StatusCode, Json<CreatedKeyResponse>), ApiError> {
    let db = db_of(&s)?;
    let tenant = tenant_of(&claims)?;
    let scopes = body.scopes.unwrap_or_else(web_keys::default_scopes);
    let (key, mat) = mint_credentials();
    let dto = web_keys::create(&db, &tenant, &body.name, &scopes, &mat).await?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedKeyResponse::from_parts(dto, key, mat.hmac_secret)),
    ))
}

/// Listar claves del tenant. Nunca incluye hash ni secreto.
#[utoipa::path(get, path = "/api/v1/admin/web/keys", tag = "AdminWeb",
    responses(
        (status = 200, body = [WebKeyDto]),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn list_keys(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<WebKeyDto>>, ApiError> {
    let db = db_of(&s)?;
    let tenant = tenant_of(&claims)?;
    Ok(Json(web_keys::list(&db, &tenant).await?))
}

/// Rotar: la clave vieja queda inactiva de inmediato; se responde una nueva
/// con el mismo nombre y scopes (plaintext una sola vez).
#[utoipa::path(post, path = "/api/v1/admin/web/keys/{id}/rotate", tag = "AdminWeb",
    params(("id" = String, Path, description = "Id de la clave (`api_key:xyz` o `xyz`)")),
    responses(
        (status = 200, body = CreatedKeyResponse),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn rotate_key(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<CreatedKeyResponse>, ApiError> {
    let db = db_of(&s)?;
    let tenant = tenant_of(&claims)?;
    let (key, mat) = mint_credentials();
    let dto = web_keys::rotate(&db, &tenant, &key_id_of(&id), &mat).await?;
    Ok(Json(CreatedKeyResponse::from_parts(
        dto,
        key,
        mat.hmac_secret,
    )))
}

/// Revocar (soft): `active = false`. 404 si la clave no existe o no es del
/// tenant.
#[utoipa::path(delete, path = "/api/v1/admin/web/keys/{id}", tag = "AdminWeb",
    params(("id" = String, Path, description = "Id de la clave (`api_key:xyz` o `xyz`)")),
    responses(
        (status = 204),
        (status = 401, body = crate::error::ErrorEnvelope),
        (status = 403, body = crate::error::ErrorEnvelope),
        (status = 404, body = crate::error::ErrorEnvelope),
    ),
    security(("bearer_jwt" = [])))]
pub async fn revoke_key(
    State(s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let db = db_of(&s)?;
    let tenant = tenant_of(&claims)?;
    if web_keys::revoke(&db, &tenant, &key_id_of(&id)).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found())
    }
}

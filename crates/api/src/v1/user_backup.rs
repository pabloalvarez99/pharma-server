//! User ciphertext backup (ADR-0022) — **opaque blob** API.
//!
//! Distinct from [`super::backup`] (admin tar.gz of the Surreal data dir).
//! This route only ever receives **already-encrypted** bytes + metadata.
//! The recovery phrase never appears on the wire.
//!
//! Storage modes:
//! - **Default:** validate upload, return `accepted: false` (bucket not wired).
//! - **Lab:** set env `RUTBUSINESS_USER_BACKUP_MEMORY=1` to keep blobs in
//!   process memory (survives only while the node runs). Useful for E2E
//!   without R2/S3. Never for production data durability.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

/// Env flag: in-process store for lab (no secrets, no disk).
const ENV_MEMORY: &str = "RUTBUSINESS_USER_BACKUP_MEMORY";

struct MemBlob {
    tenant_id: String,
    meta: domain::user_backup::EncryptedBackupMeta,
    ciphertext: Vec<u8>,
}

fn memory_enabled() -> bool {
    std::env::var(ENV_MEMORY)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
}

fn store() -> &'static Mutex<HashMap<String, MemBlob>> {
    static STORE: OnceLock<Mutex<HashMap<String, MemBlob>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            domain::user_backup::USER_BACKUP_UPLOAD_PATH,
            post(upload).get(list),
        )
        .route(
            "/api/v1/user-backup/{backup_id}",
            get(download),
        )
}

/// POST body: meta + base64 ciphertext. Validates shape; stores only if lab
/// memory mode is on, else `accepted: false`.
async fn upload(
    State(_s): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(body): Json<domain::user_backup::UploadEncryptedBackupRequest>,
) -> Result<Json<domain::user_backup::UploadEncryptedBackupResponse>, ApiError> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(body.ciphertext_base64.as_bytes())
        .map_err(|_| ApiError::invalid("ciphertext_base64 no es base64 válido"))?;

    let mut hasher = Sha256::new();
    hasher.update(&decoded);
    let sha_hex = hex::encode(hasher.finalize());

    if let Err(e) = domain::user_backup::validate_upload(&body.meta, &decoded, &sha_hex) {
        return Err(ApiError::invalid(e.message()));
    }

    // Tenant from JWT wins for storage scoping (client meta is advisory).
    let tenant = claims.tenant_id.clone();
    if tenant.is_empty() {
        return Err(ApiError::invalid("tenant vacío en la sesión"));
    }

    if !memory_enabled() {
        return Ok(Json(domain::user_backup::UploadEncryptedBackupResponse {
            accepted: false,
            reason: Some(
                "respaldo cifrado: el bucket aún no está configurado en este nodo \
                 (ADR-0022 stub). El cliente debe reintentar más tarde; la llave \
                 del cuaderno sigue siendo la única forma de recuperar. \
                 (lab: RUTBUSINESS_USER_BACKUP_MEMORY=1)"
                    .into(),
            ),
            backup_id: None,
        }));
    }

    let backup_id = format!(
        "mem-{}-{}",
        chrono_like_unix(),
        &sha_hex[..12.min(sha_hex.len())]
    );
    let mut meta = body.meta;
    meta.tenant_id = tenant.clone();
    meta.backup_id = Some(backup_id.clone());
    {
        let mut guard = store()
            .lock()
            .map_err(|_| ApiError::internal("user-backup store lock"))?;
        // Soft cap per process so lab memory cannot grow forever.
        if guard.len() >= 200 {
            return Err(ApiError::invalid(
                "lab memory store lleno (200 sobres). Reiniciá el nodo o bajá de volumen.",
            ));
        }
        guard.insert(
            backup_id.clone(),
            MemBlob {
                tenant_id: tenant,
                meta: meta.clone(),
                ciphertext: decoded,
            },
        );
    }

    Ok(Json(domain::user_backup::UploadEncryptedBackupResponse {
        accepted: true,
        reason: Some(
            "lab: guardado en memoria del proceso (no durable). \
             Bucket de producción sigue pendiente."
                .into(),
        ),
        backup_id: Some(backup_id),
    }))
}

/// GET list — metas of this tenant (empty without memory store / bucket).
async fn list(
    State(_s): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<domain::user_backup::EncryptedBackupMeta>>, ApiError> {
    if !memory_enabled() {
        return Ok(Json(vec![]));
    }
    let tenant = claims.tenant_id;
    let guard = store()
        .lock()
        .map_err(|_| ApiError::internal("user-backup store lock"))?;
    let mut out: Vec<_> = guard
        .values()
        .filter(|b| b.tenant_id == tenant)
        .map(|b| b.meta.clone())
        .collect();
    out.sort_by(|a, b| b.uploaded_at_unix.cmp(&a.uploaded_at_unix));
    Ok(Json(out))
}

/// GET one blob by id — ciphertext + meta. 404 if missing or other tenant.
async fn download(
    State(_s): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(backup_id): Path<String>,
) -> Result<Json<domain::user_backup::DownloadEncryptedBackupResponse>, ApiError> {
    if !memory_enabled() {
        return Err(ApiError::not_found());
    }
    let guard = store()
        .lock()
        .map_err(|_| ApiError::internal("user-backup store lock"))?;
    let blob = guard.get(&backup_id).ok_or_else(ApiError::not_found)?;
    if blob.tenant_id != claims.tenant_id {
        // Same as missing: no tenant enumeration.
        return Err(ApiError::not_found());
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(&blob.ciphertext);
    Ok(Json(domain::user_backup::DownloadEncryptedBackupResponse {
        meta: blob.meta.clone(),
        ciphertext_base64: b64,
        backup_id,
    }))
}

fn chrono_like_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

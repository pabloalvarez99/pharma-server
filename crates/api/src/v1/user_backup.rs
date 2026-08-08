//! User ciphertext backup (ADR-0022) — **opaque blob** API stub.
//!
//! Distinct from [`super::backup`] (admin tar.gz of the Surreal data dir).
//! This route only ever receives **already-encrypted** bytes + metadata.
//! The recovery phrase never appears on the wire.
//!
//! Until a bucket is configured, validated uploads return
//! `accepted: false` with a clear reason (not a silent 500). Shape errors
//! are 400. Auth required (any role — every tenant owner backs up).

use axum::{extract::State, routing::{get, post}, Json, Router};
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::middleware::auth::AuthUser;
use crate::AppState;

pub fn router(_state: AppState) -> Router<AppState> {
    Router::new().route(
        domain::user_backup::USER_BACKUP_UPLOAD_PATH,
        post(upload).get(list_stub),
    )
}

/// POST body: meta + base64 ciphertext. Validates shape; does **not** store
/// until bucket wiring lands (returns `accepted: false`).
async fn upload(
    State(_s): State<AppState>,
    AuthUser(_claims): AuthUser,
    Json(body): Json<domain::user_backup::UploadEncryptedBackupRequest>,
) -> Result<Json<domain::user_backup::UploadEncryptedBackupResponse>, ApiError> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(body.ciphertext_base64.as_bytes())
        .map_err(|_| ApiError::invalid("ciphertext_base64 no es base64 válido"))?;

    let mut hasher = Sha256::new();
    hasher.update(&decoded);
    let sha_hex = hex::encode(hasher.finalize());

    if let Err(e) =
        domain::user_backup::validate_upload(&body.meta, &decoded, &sha_hex)
    {
        return Err(ApiError::invalid(e.message()));
    }

    // Bucket not wired: accept the contract, refuse persistence.
    Ok(Json(domain::user_backup::UploadEncryptedBackupResponse {
        accepted: false,
        reason: Some(
            "respaldo cifrado: el bucket aún no está configurado en este nodo \
             (ADR-0022 stub). El cliente debe reintentar más tarde; la llave \
             del cuaderno sigue siendo la única forma de recuperar."
                .into(),
        ),
        backup_id: None,
    }))
}

/// GET list — empty until storage exists.
async fn list_stub(
    State(_s): State<AppState>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<Vec<domain::user_backup::EncryptedBackupMeta>>, ApiError> {
    Ok(Json(vec![]))
}

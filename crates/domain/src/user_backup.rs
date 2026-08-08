//! User-held backup key (ADR-0022) — domain shapes + validation.
//!
//! Product decision: ciphertext lives in a RutBusiness bucket; the **key never
//! leaves the user's head / notebook**. Server stores opaque blobs only.
//!
//! Crypto (Argon2id + AES-GCM) runs **on the client**. This module freezes:
//! - wire shapes for upload / list / download metadata
//! - KDF + AEAD parameter names for `format_version = 1`
//! - pure validators so API and Android never diverge

use serde::{Deserialize, Serialize};

/// Current blob format. Bump when KDF / AEAD / packing changes.
pub const BACKUP_FORMAT_VERSION: u16 = 1;

/// Inner **plaintext snapshot** version (JSON before AES-GCM).
/// Distinct from [`BACKUP_FORMAT_VERSION`] (envelope crypto params).
pub const SNAPSHOT_FORMAT_VERSION: u16 = 1;

/// Documented path (API stub returns not-implemented until bucket is wired).
pub const USER_BACKUP_UPLOAD_PATH: &str = "/api/v1/user-backup";

/// List metadata for the tenant's opaque blobs.
pub const USER_BACKUP_LIST_PATH: &str = "/api/v1/user-backup";

/// Known snapshot section keys (client packs; server never reads plaintext).
pub const SNAPSHOT_SECTION_PENDING_SALES: &str = "pending_sales";
pub const SNAPSHOT_SECTION_RUBRO: &str = "rubro";

// --- recovery phrase contract (client generates; server never sees plaintext) --

pub const RECOVERY_WORD_COUNT: usize = 12;
pub const RECOVERY_BLOCK_COUNT: usize = 8;
pub const RECOVERY_BLOCK_LEN: usize = 4;

/// Wire shape for "create recovery material" (client-local only for now).
///
/// The server must **never** receive the plaintext recovery phrase. Only a
/// verifier (hash) may be stored later if we add "did the user type the same
/// phrase" checks without holding the key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryMaterialPublic {
    pub tenant_id: String,
    pub created_at_unix: i64,
    pub label: Option<String>,
}

// --- format v1 crypto parameters (client-side; documented here for parity) ----

/// KDF algorithm name on the wire / in the envelope header.
pub const KDF_ALG: &str = "argon2id";

/// Memory cost (KiB) for Argon2id v1. ~64 MiB — phones of the floor (1–2 GB)
/// can afford a background derive; do not raise without a format bump.
pub const KDF_MEMORY_KIB: u32 = 65_536;

/// Time cost (iterations) for Argon2id v1.
pub const KDF_ITERATIONS: u32 = 3;

/// Parallelism for Argon2id v1.
pub const KDF_PARALLELISM: u32 = 1;

/// Derived key length (bytes) → AES-256-GCM.
pub const KDF_OUTPUT_LEN: usize = 32;

/// AEAD algorithm name.
pub const AEAD_ALG: &str = "aes-256-gcm";

/// Salt length (bytes) stored with the envelope (not secret).
pub const KDF_SALT_LEN: usize = 16;

/// Nonce length for AES-GCM (bytes).
pub const AEAD_NONCE_LEN: usize = 12;

// --- upload wire shapes -------------------------------------------------------

/// Opaque backup object metadata (server sees only this + ciphertext bytes).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedBackupMeta {
    pub tenant_id: String,
    /// Content version of the blob format (bump when AEAD params change).
    pub format_version: u16,
    /// SHA-256 of ciphertext (integrity, not confidentiality).
    pub ciphertext_sha256_hex: String,
    /// Byte length of ciphertext.
    pub size_bytes: u64,
    pub uploaded_at_unix: i64,
    /// Optional client-chosen label ("cuaderno 2026-08").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Client → server upload body.
///
/// `ciphertext_base64` is the **only** payload bytes. No plaintext fields.
/// Server validates shape + sha256 match, then stores opaquely.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadEncryptedBackupRequest {
    pub meta: EncryptedBackupMeta,
    /// Standard base64 of the ciphertext envelope (see [`EnvelopeHeaderV1`]).
    pub ciphertext_base64: String,
}

/// Server → client after accepting (or rejecting) an upload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadEncryptedBackupResponse {
    /// `true` only when the blob was persisted to the bucket.
    pub accepted: bool,
    /// Human reason when `accepted == false` (bucket missing, shape error…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Server-assigned id when accepted (opaque).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_id: Option<String>,
}

/// Header the client prefixes inside the ciphertext package **before** AEAD
/// encrypts the snapshot. Stored as plaintext JSON next to salt/nonce in the
/// outer envelope the server never interprets beyond length/hash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvelopeHeaderV1 {
    pub format_version: u16,
    pub kdf: String,
    pub kdf_memory_kib: u32,
    pub kdf_iterations: u32,
    pub kdf_parallelism: u32,
    pub aead: String,
    /// Hex salt (32 hex chars for 16 bytes).
    pub salt_hex: String,
    /// Hex nonce (24 hex chars for 12 bytes).
    pub nonce_hex: String,
}

impl EnvelopeHeaderV1 {
    /// Canonical v1 header skeleton (caller fills salt/nonce hex).
    pub fn template(salt_hex: String, nonce_hex: String) -> Self {
        Self {
            format_version: BACKUP_FORMAT_VERSION,
            kdf: KDF_ALG.into(),
            kdf_memory_kib: KDF_MEMORY_KIB,
            kdf_iterations: KDF_ITERATIONS,
            kdf_parallelism: KDF_PARALLELISM,
            aead: AEAD_ALG.into(),
            salt_hex,
            nonce_hex,
        }
    }
}

// --- validation (pure) --------------------------------------------------------

/// True when a typed phrase has the expected word count (whitespace-split).
pub fn phrase_shape_ok(phrase: &str) -> bool {
    let n = phrase.split_whitespace().filter(|w| !w.is_empty()).count();
    n == RECOVERY_WORD_COUNT
}

/// True when a typed block string matches `XXXX-XXXX-…` (8×4 alnum).
pub fn blocks_shape_ok(blocks: &str) -> bool {
    let parts: Vec<&str> = blocks
        .split(|c: char| c == '-' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    parts.len() == RECOVERY_BLOCK_COUNT
        && parts.iter().all(|p| {
            p.len() == RECOVERY_BLOCK_LEN && p.chars().all(|c| c.is_ascii_alphanumeric())
        })
}

/// Why an upload body is rejected (no I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadValidationError {
    BadFormatVersion,
    EmptyTenant,
    BadSha256Hex,
    EmptyCiphertext,
    SizeMismatch { meta: u64, actual: u64 },
    Sha256Mismatch,
}

impl UploadValidationError {
    pub fn message(&self) -> String {
        match self {
            Self::BadFormatVersion => format!(
                "format_version debe ser {BACKUP_FORMAT_VERSION} (v1 Argon2id+AES-GCM)"
            ),
            Self::EmptyTenant => "tenant_id vacío".into(),
            Self::BadSha256Hex => "ciphertext_sha256_hex debe ser 64 hex chars".into(),
            Self::EmptyCiphertext => "ciphertext_base64 vacío".into(),
            Self::SizeMismatch { meta, actual } => {
                format!("size_bytes={meta} no calza con ciphertext ({actual} bytes)")
            }
            Self::Sha256Mismatch => "sha256 del ciphertext no calza con meta".into(),
        }
    }
}

/// Validate upload **without** decoding crypto. `decoded_ciphertext` is the
/// raw bytes after base64 decode (caller decodes; domain stays free of base64).
pub fn validate_upload(
    meta: &EncryptedBackupMeta,
    decoded_ciphertext: &[u8],
    computed_sha256_hex: &str,
) -> Result<(), UploadValidationError> {
    if meta.format_version != BACKUP_FORMAT_VERSION {
        return Err(UploadValidationError::BadFormatVersion);
    }
    if meta.tenant_id.trim().is_empty() {
        return Err(UploadValidationError::EmptyTenant);
    }
    if !is_sha256_hex(&meta.ciphertext_sha256_hex) {
        return Err(UploadValidationError::BadSha256Hex);
    }
    if decoded_ciphertext.is_empty() {
        return Err(UploadValidationError::EmptyCiphertext);
    }
    let actual = decoded_ciphertext.len() as u64;
    if meta.size_bytes != actual {
        return Err(UploadValidationError::SizeMismatch {
            meta: meta.size_bytes,
            actual,
        });
    }
    if !eq_hex_ci(&meta.ciphertext_sha256_hex, computed_sha256_hex) {
        return Err(UploadValidationError::Sha256Mismatch);
    }
    Ok(())
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn eq_hex_ci(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

/// Payload encoded in the rescue QR / printable card (no secrets of the server).
///
/// Format: `rutbusiness-rescue:v1:<tenant_slug>:<block0>-…-<block7>`
/// The phrase words are **not** put in the QR by default (too easy to photo-
/// share); blocks are shorter for the notebook. App restore accepts either.
pub fn rescue_qr_payload(tenant_slug: &str, bloques: &[String]) -> Option<String> {
    if bloques.len() != RECOVERY_BLOCK_COUNT {
        return None;
    }
    if !bloques.iter().all(|b| b.len() == RECOVERY_BLOCK_LEN) {
        return None;
    }
    let slug = tenant_slug.trim().to_lowercase();
    if slug.is_empty() {
        return None;
    }
    Some(format!(
        "rutbusiness-rescue:v1:{}:{}",
        slug,
        bloques.join("-")
    ))
}

/// Parse a rescue QR / typed payload. Returns `(tenant_slug, blocks joined)`.
pub fn parse_rescue_qr_payload(raw: &str) -> Option<(String, String)> {
    let s = raw.trim();
    let rest = s.strip_prefix("rutbusiness-rescue:v1:")?;
    let (slug, blocks) = rest.split_once(':')?;
    if slug.is_empty() || !blocks_shape_ok(blocks) {
        return None;
    }
    Some((slug.to_lowercase(), blocks.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_needs_twelve_words() {
        assert!(!phrase_shape_ok("uno dos"));
        assert!(phrase_shape_ok(
            "uno dos tres cuatro cinco seis siete ocho nueve diez once doce"
        ));
    }

    #[test]
    fn blocks_need_eight_of_four() {
        assert!(!blocks_shape_ok("AB3K"));
        assert!(blocks_shape_ok("AB3K-9F2Q-M7NP-4RST-WXY2-HJKL-QRST-VBNM"));
        assert!(blocks_shape_ok("AB3K 9F2Q M7NP 4RST WXY2 HJKL QRST VBNM"));
    }

    #[test]
    fn format_version_is_stable_v1() {
        assert_eq!(BACKUP_FORMAT_VERSION, 1);
        assert_eq!(SNAPSHOT_FORMAT_VERSION, 1);
        assert_eq!(KDF_ALG, "argon2id");
        assert_eq!(AEAD_ALG, "aes-256-gcm");
        assert_eq!(USER_BACKUP_UPLOAD_PATH, "/api/v1/user-backup");
        assert_eq!(SNAPSHOT_SECTION_PENDING_SALES, "pending_sales");
    }

    #[test]
    fn envelope_template_carries_v1_params() {
        let h = EnvelopeHeaderV1::template("aa".repeat(16), "bb".repeat(12));
        assert_eq!(h.format_version, 1);
        assert_eq!(h.kdf_memory_kib, KDF_MEMORY_KIB);
        assert_eq!(h.aead, AEAD_ALG);
    }

    #[test]
    fn validate_upload_happy_path() {
        let ct = b"not-real-ciphertext-but-long-enough";
        let sha = "a".repeat(64);
        let meta = EncryptedBackupMeta {
            tenant_id: "tenant:abc".into(),
            format_version: 1,
            ciphertext_sha256_hex: sha.clone(),
            size_bytes: ct.len() as u64,
            uploaded_at_unix: 1,
            label: None,
        };
        assert!(validate_upload(&meta, ct, &sha).is_ok());
    }

    #[test]
    fn validate_upload_rejects_size_and_sha() {
        let ct = b"abc";
        let sha = "b".repeat(64);
        let meta = EncryptedBackupMeta {
            tenant_id: "t".into(),
            format_version: 1,
            ciphertext_sha256_hex: "a".repeat(64),
            size_bytes: 99,
            uploaded_at_unix: 1,
            label: None,
        };
        assert!(matches!(
            validate_upload(&meta, ct, &sha),
            Err(UploadValidationError::SizeMismatch { .. })
        ));
        let meta2 = EncryptedBackupMeta {
            size_bytes: 3,
            ..meta
        };
        assert_eq!(
            validate_upload(&meta2, ct, &sha),
            Err(UploadValidationError::Sha256Mismatch)
        );
    }

    #[test]
    fn rescue_qr_roundtrip() {
        let bloques = vec![
            "AB3K".into(),
            "9F2Q".into(),
            "M7NP".into(),
            "4RST".into(),
            "WXY2".into(),
            "HJKL".into(),
            "QRST".into(),
            "VBNM".into(),
        ];
        let payload = rescue_qr_payload("puesto-rosa", &bloques).expect("payload");
        assert!(payload.starts_with("rutbusiness-rescue:v1:puesto-rosa:"));
        let (slug, blocks) = parse_rescue_qr_payload(&payload).expect("parse");
        assert_eq!(slug, "puesto-rosa");
        assert!(blocks_shape_ok(&blocks));
    }
}

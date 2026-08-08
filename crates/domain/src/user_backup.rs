//! User-held backup key (ADR-0022) — domain stubs.
//!
//! Product decision: ciphertext lives in a RutBusiness bucket; the **key never
//! leaves the user's head / notebook**. Server stores opaque blobs only.
//!
//! This module defines the **shapes** and pure helpers that API/Android will
//! share. Crypto (Argon2id + AES-GCM) and the upload path are **not** wired
//! here yet — intentional stub so feria day-1 UI and docs can bind to stable
//! names without pretending encryption is done.

use serde::{Deserialize, Serialize};

/// Wire shape for "create recovery material" (client → will POST later).
///
/// The server must **never** receive the plaintext recovery phrase. Only a
/// verifier (hash) may be stored if we later add "did the user type the same
/// phrase" checks without holding the key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryMaterialPublic {
    /// Tenant that owns the key (opaque id string on the wire).
    pub tenant_id: String,
    /// Unix seconds when the material was created (client clock is fine for
    /// the notebook card; server may re-stamp on first upload).
    pub created_at_unix: i64,
    /// Optional label the owner chose ("puesto 3", "cuaderno cocina").
    pub label: Option<String>,
}

/// Opaque backup object metadata (server sees only this + ciphertext bytes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedBackupMeta {
    pub tenant_id: String,
    /// Content version of the blob format (bump when AEAD params change).
    pub format_version: u16,
    /// SHA-256 of ciphertext (integrity, not confidentiality).
    pub ciphertext_sha256_hex: String,
    /// Byte length of ciphertext.
    pub size_bytes: u64,
    pub uploaded_at_unix: i64,
}

/// Current blob format. Bump when KDF / AEAD / packing changes.
pub const BACKUP_FORMAT_VERSION: u16 = 1;

/// Placeholder: derive a display-only recovery phrase size contract.
///
/// Real generation lives on the client (CSPRNG). Domain only documents the
/// invariants so API validation and Android stay aligned.
pub const RECOVERY_WORD_COUNT: usize = 12;
pub const RECOVERY_BLOCK_COUNT: usize = 8;
pub const RECOVERY_BLOCK_LEN: usize = 4;

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
    }
}

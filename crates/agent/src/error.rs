//! Crate-wide error type for the `agent` crate.
//!
//! Centralizes the failure modes of the identity / card / envelope hot path so
//! callers can pattern-match instead of stringly comparing. Every public
//! fallible operation in this crate returns [`Result<T, AgentError>`].
//!
//! Replaces the historical `unwrap`/`expect` calls in `canonical`, `card`,
//! `envelope`, and `identity` that could blow up the federation hot path on
//! malformed wire input (Fase 11 audit, branch `feat/quality-agent-unwrap-fix`).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    /// Bad raw key material (wrong length, hex/base58 garbage, etc.).
    #[error("ed25519 key error: {0}")]
    Key(String),

    /// Ed25519 signature verification failed (tampered bytes, forged `from`).
    #[error("signature verify failed")]
    SignatureInvalid,

    /// DID failed to parse (bad prefix or undecodable pubkey).
    #[error("DID inválido: {0}")]
    BadDid(String),

    /// Canonical JSON encoding failed (unrepresentable value, key serialize).
    #[error("canonical JSON: {0}")]
    Canonical(String),

    /// Envelope JSON could not be decoded.
    #[error("envelope decode: {0}")]
    Envelope(String),

    /// Base64 or hex decode failed for an embedded byte string.
    #[error("base64 decode: {0}")]
    Base64(String),

    /// Filesystem error while persisting/loading identity material.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// `serde_json` failure outside the envelope decode path (e.g. card
    /// (de)serialization, value→json conversion).
    #[error("serialización: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, AgentError>;

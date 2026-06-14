//! Error type for the stock-sync outbox layer.

use thiserror::Error;

/// Errors raised by the sync crate. User-facing strings are Spanish; variant
/// names are English (project convention).
#[derive(Debug, Error)]
pub enum SyncError {
    /// Underlying SurrealDB failure. Boxed so `Result` stays small
    /// (`surrealdb::Error` is a large enum; otherwise `clippy::result_large_err`
    /// fires across every outbox fn — same reason as `domain::DomainError`).
    #[error("error de base de datos: {0}")]
    Db(Box<surrealdb::Error>),

    /// Envelope sign/verify failure from the reused `agent` crate.
    #[error("error de envelope firmado: {0}")]
    Agent(Box<agent::AgentError>),

    /// (De)serialization of the queued envelope JSON.
    #[error("error de serialización: {0}")]
    Serde(#[from] serde_json::Error),

    /// A tenant id string that does not parse to a `tenant:<id>` record.
    #[error("tenant inválido: {0}")]
    InvalidTenant(String),

    /// The signed envelope failed verification on the drain path — never
    /// deliver a tampered queued envelope.
    #[error("envelope inválido o adulterado")]
    TamperedEnvelope,

    /// Transport (network/peer) failure surfaced by a [`crate::PushTransport`].
    /// `retryable` decides whether the outbox keeps the row pending for another
    /// drain or marks it permanently failed.
    #[error("error de transporte hacia el peer: {message}")]
    Transport { message: String, retryable: bool },
}

impl SyncError {
    /// Build a retryable transport error (5xx / timeout / peer offline).
    pub fn transport_retryable(message: impl Into<String>) -> Self {
        Self::Transport {
            message: message.into(),
            retryable: true,
        }
    }

    /// Build a permanent transport error (bad signature / 4xx contract error).
    pub fn transport_permanent(message: impl Into<String>) -> Self {
        Self::Transport {
            message: message.into(),
            retryable: false,
        }
    }
}

// Boxed conversions so `Result<T, SyncError>` stays small (see `Db` variant).
impl From<surrealdb::Error> for SyncError {
    fn from(e: surrealdb::Error) -> Self {
        Self::Db(Box::new(e))
    }
}

impl From<agent::AgentError> for SyncError {
    fn from(e: agent::AgentError) -> Self {
        Self::Agent(Box::new(e))
    }
}

/// Crate-wide result alias.
pub type Result<T> = std::result::Result<T, SyncError>;

use thiserror::Error;

/// Domain-level errors. Crate `api` maps these to HTTP envelope responses.
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("entidad no encontrada")]
    NotFound,
    #[error("conflicto: {0}")]
    Conflict(String),
    #[error("entrada inválida: {0}")]
    Invalid(String),
    #[error("stock insuficiente")]
    InsufficientStock,
    #[error("permiso denegado")]
    Forbidden,
    #[error("idempotency replay")]
    IdempotencyReplay,
    #[error("not implemented")]
    NotImplemented,
    #[error("db: {0}")]
    Db(Box<surrealdb::Error>),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

// Boxed so `DomainResult` stays small (`surrealdb::Error` is a large enum;
// otherwise clippy::result_large_err fires across every repo fn).
impl From<surrealdb::Error> for DomainError {
    fn from(e: surrealdb::Error) -> Self {
        Self::Db(Box::new(e))
    }
}

impl DomainError {
    /// `true` when this wraps a *transient* SurrealKv MVCC write-write / busy
    /// conflict (the abort a losing concurrent transaction takes at COMMIT),
    /// as opposed to a genuine storage fault. Lets the `api` layer answer a
    /// retryable 503 ("reintente") instead of an opaque 500, and the write
    /// paths decide whether to retry. Same string match the sale write path's
    /// retry loop uses (`sales::service`) + `dte::caf`.
    pub fn is_retryable_db_conflict(&self) -> bool {
        match self {
            Self::Db(inner) => {
                let s = inner.to_string();
                s.contains("read or write conflict") || s.contains("failed transaction")
            }
            _ => false,
        }
    }

    /// SCREAMING_SNAKE code for the error envelope.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "NOT_FOUND",
            Self::Conflict(_) => "CONFLICT",
            Self::Invalid(_) => "INVALID_INPUT",
            Self::InsufficientStock => "INSUFFICIENT_STOCK",
            Self::Forbidden => "FORBIDDEN",
            Self::IdempotencyReplay => "IDEMPOTENCY_REPLAY",
            Self::NotImplemented => "NOT_IMPLEMENTED",
            Self::Db(_) => "DB_ERROR",
            Self::Other(_) => "INTERNAL_ERROR",
        }
    }
}

pub type DomainResult<T> = std::result::Result<T, DomainError>;

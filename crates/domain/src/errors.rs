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

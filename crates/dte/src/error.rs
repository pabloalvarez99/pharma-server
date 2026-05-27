//! Errores del módulo DTE. Mensajes user-facing en español, códigos enum en inglés.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DteError {
    #[error("CAF agotado para tipo DTE {tipo}: folio {folio} fuera del rango autorizado")]
    FolioExhausted { tipo: i32, folio: i64 },

    #[error("CAF inválido: {0}")]
    CafInvalid(String),

    #[error("cert digital expirado: vigencia hasta {until}")]
    CertExpired {
        until: chrono::DateTime<chrono::Utc>,
    },

    #[error("cert digital inválido: {0}")]
    CertInvalid(String),

    #[error("cert digital cifrado falló: {0}")]
    CertEncrypt(String),

    #[error("cert digital descifrado falló: {0}")]
    CertDecrypt(String),

    #[error("SII rechazó el DTE: {glosa}")]
    SiiRejected { glosa: String },

    #[error("error red SII: {0}")]
    SiiNetwork(String),

    #[error("XML inválido: {0}")]
    XmlInvalid(String),

    #[error("firma falló: {0}")]
    SignFailed(String),

    #[error("tipo DTE no soportado: {0}")]
    UnsupportedTipo(i32),

    #[error(
        "el envío automático al SII del tipo DTE {tipo} requiere tier '{required_tier}' \
         (licencia actual: '{tier}')"
    )]
    SendNotEntitled {
        /// Tier actual de la licencia (etiqueta: `free|pro|business|enterprise`).
        tier: String,
        /// Código SII del tipo DTE que se intentó enviar.
        tipo: i32,
        /// Tier mínimo requerido para enviar ese tipo (etiqueta).
        required_tier: String,
    },

    #[error("DTE en estado {actual:?} no permite la transición a {target:?}")]
    InvalidTransition {
        actual: crate::types::DteEstado,
        target: crate::types::DteEstado,
    },

    #[error("transición de estado DTE inválida: {from:?} → {to:?}")]
    InvalidStateTransition {
        from: crate::types::DteEstado,
        to: crate::types::DteEstado,
    },

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde_json: {0}")]
    Json(#[from] serde_json::Error),
}

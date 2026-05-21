//! `TimbreElectronico` (TED) — hash SHA1 + firma RSA-SHA1 sobre el `DD` (Datos
//! Documento) del DTE, usando la clave privada del CAF.
//!
//! Subtask 9.1.b pendiente. Stub define la API.

use crate::types::{Caf, Dte};
use crate::DteError;

/// Genera el bloque `<TED>` para inyectar en el XML del DTE pre-firma.
pub fn generate(_dte: &Dte, _caf: &Caf) -> Result<String, DteError> {
    Err(DteError::SignFailed(
        "timbre::generate: pendiente subtask 9.1.b".to_string(),
    ))
}

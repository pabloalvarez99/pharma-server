//! Firma del XML completo del DTE con el cert digital de la empresa
//! (RSA-SHA1 sobre el bloque `<Documento>`).
//!
//! Subtask 9.1.b pendiente.

use crate::types::{CertDigital, Dte};
use crate::DteError;

/// Firma `xml_unsigned` con `cert` (PFX decifrado en memoria) y devuelve el
/// XML con `<Signature>` enmendado.
pub fn sign_xml(
    _xml_unsigned: &str,
    _cert: &CertDigital,
    _password: &str,
) -> Result<String, DteError> {
    Err(DteError::SignFailed(
        "sign::sign_xml: pendiente subtask 9.1.b".to_string(),
    ))
}

/// Wrapper end-to-end: renderiza unsigned, inyecta TED, firma, devuelve XML
/// listo para envío SII.
pub fn build_signed_dte(
    _dte: &Dte,
    _cert: &CertDigital,
    _password: &str,
) -> Result<String, DteError> {
    Err(DteError::SignFailed(
        "sign::build_signed_dte: pendiente subtask 9.1.b".to_string(),
    ))
}

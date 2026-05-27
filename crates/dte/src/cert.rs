//! Cert digital encrypt-at-rest y carga in-memory para firma.
//!
//! Subtask 9.1.i pendiente. Plan:
//! - `encrypt_at_rest(pfx_bytes, password, tenant_id, master_key)`: cifra ambos
//!   con AES-GCM-256 con clave derivada `argon2id(tenant_id || master_key)`.
//! - `decrypt_for_sign(cert, tenant_id, master_key)`: decifra solo en memoria,
//!   devuelve `(Vec<u8>, String)` y caller debe `zeroize` después de usar.
//!
//! Por ahora stubs.

use crate::types::CertDigital;
use crate::DteError;

pub fn encrypt_at_rest(
    _pfx_bytes: &[u8],
    _password: &str,
    _tenant_id: &str,
    _master_key: &[u8; 32],
) -> Result<(Vec<u8>, Vec<u8>), DteError> {
    Err(DteError::CertInvalid(
        "cert::encrypt_at_rest: pendiente subtask 9.1.i".to_string(),
    ))
}

pub fn decrypt_for_sign(
    _cert: &CertDigital,
    _tenant_id: &str,
    _master_key: &[u8; 32],
) -> Result<(Vec<u8>, String), DteError> {
    Err(DteError::CertInvalid(
        "cert::decrypt_for_sign: pendiente subtask 9.1.i".to_string(),
    ))
}

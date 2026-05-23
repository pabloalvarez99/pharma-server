//! Cert digital encrypt-at-rest y carga in-memory para firma — subtask 9.1.i.
//!
//! El PFX (PKCS#12) y su password van cifrados en `cert_digital.pfx_blob` y
//! `cert_digital.password_encrypted` con AES-256-GCM. La clave AES se deriva
//! con argon2id usando `master_key` como secreto y `tenant_id` como salt — el
//! mismo tenant siempre obtiene la misma clave, distintos tenants obtienen
//! claves distintas (defensa lateral movement).
//!
//! Layout del payload cifrado: `nonce (12 bytes) || ciphertext+tag`. Tag GCM
//! va anexado por la implementación de `aes-gcm`, no se almacena aparte.
//!
//! Decifrado: `decrypt_for_sign` retorna `(Vec<u8>, String)` con `ZeroizeOnDrop`
//! para que el caller no tenga que limpiar manualmente — al salir de scope se
//! borran los buffers.
//!
//! ADR-0011 §"Cert digital encrypt-at-rest" autoriza este patrón. Master key
//! viene de `PHARMA__DTE__MASTER_KEY` (32 bytes hex) en producción; si falta
//! se deriva de `PHARMA__JWT__SECRET` con warning (decisión documentada).

use crate::types::CertDigital;
use crate::DteError;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use rand_core::{OsRng, RngCore};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Tamaño del nonce GCM (96 bits, recomendado por NIST SP 800-38D).
const NONCE_LEN: usize = 12;

/// Salt mínimo aceptado por Argon2 = 8 bytes. El `tenant_id` real (UUID,
/// `tenant:xxxx`, etc.) supera ese mínimo holgadamente.
const MIN_SALT_LEN: usize = 8;

/// Material descifrado del cert listo para firmar. Implementa `ZeroizeOnDrop`
/// para borrar la memoria al salir de scope: el caller obtiene `pfx` + `password`
/// y NO necesita llamar a `zeroize()` manualmente.
///
/// `Debug` redacta el contenido — el plaintext del PFX y password NUNCA deben
/// aparecer en logs/asserts accidentales.
#[derive(ZeroizeOnDrop)]
pub struct DecryptedCert {
    pub pfx: Vec<u8>,
    pub password: String,
}

impl std::fmt::Debug for DecryptedCert {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecryptedCert")
            .field("pfx", &format!("[REDACTED {} bytes]", self.pfx.len()))
            .field("password", &"[REDACTED]")
            .finish()
    }
}

/// Deriva una clave AES-256 de 32 bytes con argon2id(secret=master_key,
/// salt=tenant_id). Parámetros: 19 MiB, 2 iteraciones, paralelismo 1 — perfil
/// "interactivo" del paper; mover a "moderado" cuando deje de ser hot path
/// (encrypt/decrypt sólo ocurre 1 vez por firma DTE, no por request).
fn derive_key(tenant_id: &str, master_key: &[u8; 32]) -> Result<[u8; 32], DteError> {
    if tenant_id.len() < MIN_SALT_LEN {
        return Err(DteError::CertInvalid(format!(
            "tenant_id demasiado corto para salt argon2 ({} < {})",
            tenant_id.len(),
            MIN_SALT_LEN
        )));
    }
    let params = Params::new(19 * 1024, 2, 1, Some(32))
        .map_err(|e| DteError::CertInvalid(format!("argon2 params inválidos: {e}")))?;
    let argon = Argon2::new_with_secret(master_key, Algorithm::Argon2id, Version::V0x13, params)
        .map_err(|e| DteError::CertInvalid(format!("argon2 init: {e}")))?;
    let mut out = [0u8; 32];
    argon
        .hash_password_into(tenant_id.as_bytes(), tenant_id.as_bytes(), &mut out)
        .map_err(|e| DteError::CertInvalid(format!("argon2 derive: {e}")))?;
    Ok(out)
}

/// Cifra `plaintext` con AES-256-GCM bajo `key` con un nonce random fresco.
/// Output: `nonce || ciphertext_con_tag`.
fn aead_encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, DteError> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| DteError::CertInvalid(format!("AES-GCM encrypt: {e}")))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

fn aead_decrypt(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>, DteError> {
    if blob.len() < NONCE_LEN + 16 {
        // 16 = tag GCM mínimo. Sin nonce + tag no hay nada que decifrar.
        return Err(DteError::CertInvalid(
            "blob cifrado demasiado corto".to_string(),
        ));
    }
    let (nonce_bytes, ct) = blob.split_at(NONCE_LEN);
    let key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher.decrypt(nonce, ct).map_err(|_| {
        DteError::CertInvalid("AES-GCM decrypt: tag inválido o clave incorrecta".to_string())
    })
}

/// Cifra el PFX bytes y el password con claves derivadas por argon2id +
/// nonces independientes (GCM con misma key + mismo nonce sería catastrófico;
/// nonces random distintos son suficientes).
///
/// Retorna `(pfx_encrypted, password_encrypted)` listos para persistir en
/// `cert_digital.pfx_blob` y `cert_digital.password_encrypted`.
pub fn encrypt_at_rest(
    pfx_bytes: &[u8],
    password: &str,
    tenant_id: &str,
    master_key: &[u8; 32],
) -> Result<(Vec<u8>, Vec<u8>), DteError> {
    let mut key = derive_key(tenant_id, master_key)?;
    let pfx_enc = aead_encrypt(&key, pfx_bytes)?;
    let pw_enc = aead_encrypt(&key, password.as_bytes())?;
    key.zeroize();
    Ok((pfx_enc, pw_enc))
}

/// Decifra el PFX + password de `cert` con la misma derivación. Retorna un
/// `DecryptedCert` que limpia los buffers al salir de scope (`ZeroizeOnDrop`).
pub fn decrypt_for_sign(
    cert: &CertDigital,
    tenant_id: &str,
    master_key: &[u8; 32],
) -> Result<DecryptedCert, DteError> {
    let mut key = derive_key(tenant_id, master_key)?;
    let pfx = aead_decrypt(&key, &cert.pfx_blob)?;
    let pw_bytes = aead_decrypt(&key, &cert.password_encrypted)?;
    key.zeroize();
    let password = String::from_utf8(pw_bytes)
        .map_err(|e| DteError::CertInvalid(format!("password no es UTF-8 válido: {e}")))?;
    Ok(DecryptedCert { pfx, password })
}

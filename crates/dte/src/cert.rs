//! Cert digital encrypt-at-rest (subtask 9.1.i).
//!
//! El cert digital SII es un PFX/PKCS#12 (RSA priv key + cadena X.509). El
//! tenant lo sube vía CLI/API y la farmacia es dueña tanto del PFX como de la
//! passphrase. En disco SOLO se persiste el blob cifrado y los parámetros de
//! KDF — la passphrase NUNCA se almacena (la farmacia la teclea cada vez que
//! sube cert o re-firma, o la guarda en su propio gestor de secretos).
//!
//! ## Esquema cripto
//!
//! - **Cipher**: AES-256-GCM. Nonce 12 bytes random por encrypt.
//! - **KDF**: Argon2id derivando 32 bytes (clave AES-256). Salt 16 bytes
//!   random por encrypt. Parámetros embebidos en `KdfParams` (m, t, p, ver)
//!   para futuro tuning sin romper artefactos existentes.
//! - **Blob layout**: `salt(16) || nonce(12) || ciphertext || gcm_tag(16)`.
//!   `aes-gcm` produce ciphertext con el tag concatenado al final, así que el
//!   layout efectivo es `salt(16) || nonce(12) || (ciphertext+tag)`.
//!
//! ## Persistencia (tabla `cert_digital`, migración 0017)
//!
//! - `pfx_blob: bytes` → blob completo (salt+nonce+ciphertext+tag).
//! - `password_encrypted: bytes` → JSON-bytes serializados de `KdfParams`
//!   (no es secreto: salt va en el blob, los parámetros son públicos por
//!   diseño). Reusamos el field existente para evitar migración nueva; el
//!   nombre legacy refleja un diseño previo donde la passphrase se guardaba
//!   cifrada — abandonado al optar por passphrase user-supplied per-operación.
//!
//! ## Invariantes
//!
//! - PFX plaintext y passphrase NUNCA tocan disco/log. `tracing` solo emite
//!   metadatos (RUT, vigencia, tamaño blob).
//! - `decrypt_pfx` retorna `Zeroizing<Vec<u8>>` para wipe automático on-drop.
//! - Tag GCM mismatch (passphrase incorrecta o blob alterado) → `CertDecrypt`
//!   loud, NO se traga el error.

use crate::DteError;
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use chrono::{DateTime, Utc};
use pharma_core::tenant::TenantId;
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use surrealdb::{Connection, Surreal};
use zeroize::Zeroizing;

/// Tamaño de la salt Argon2id (bytes).
pub const SALT_LEN: usize = 16;
/// Tamaño del nonce AES-GCM (bytes).
pub const NONCE_LEN: usize = 12;
/// Tamaño de clave AES-256 (bytes).
pub const KEY_LEN: usize = 32;

/// Parámetros del KDF Argon2id usado para derivar la clave AES. Persistidos
/// junto al blob cifrado para que el decrypt sepa con qué costo regenerar la
/// clave (futuro tuning de costo sin romper artefactos existentes).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KdfParams {
    /// Memoria en KiB (Argon2 `m_cost`).
    pub m: u32,
    /// Iteraciones (Argon2 `t_cost`).
    pub t: u32,
    /// Paralelismo (Argon2 `p_cost`).
    pub p: u32,
    /// Versión Argon2 (`0x13` = 1.3, default).
    pub version: u32,
}

impl Default for KdfParams {
    /// Defaults razonables para encrypt-at-rest server-side: 64 MiB / 3 iter /
    /// 1 lane. Argon2 OWASP guidance 2023 para uso interactivo no-PWN.
    /// Tuneable a futuro sin romper artefactos: el blob trae sus propios m/t/p.
    fn default() -> Self {
        Self {
            m: 64 * 1024,
            t: 3,
            p: 1,
            version: Version::V0x13 as u32,
        }
    }
}

impl KdfParams {
    fn to_argon2(&self) -> Result<Argon2<'_>, DteError> {
        let version = match self.version {
            v if v == Version::V0x10 as u32 => Version::V0x10,
            v if v == Version::V0x13 as u32 => Version::V0x13,
            other => {
                return Err(DteError::CertEncrypt(format!(
                    "versión Argon2 no soportada: {other}"
                )))
            }
        };
        let params = Params::new(self.m, self.t, self.p, Some(KEY_LEN))
            .map_err(|e| DteError::CertEncrypt(format!("Argon2 params inválidos: {e}")))?;
        Ok(Argon2::new(Algorithm::Argon2id, version, params))
    }
}

/// PFX cifrado listo para persistir. `blob` = `salt || nonce || ciphertext+tag`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedPfx {
    pub blob: Vec<u8>,
    pub kdf: KdfParams,
}

/// `(salt, nonce, ciphertext+tag)` prestados del blob cifrado.
type PfxParts<'a> = (&'a [u8], &'a [u8], &'a [u8]);

impl EncryptedPfx {
    fn split(&self) -> Result<PfxParts<'_>, DteError> {
        if self.blob.len() < SALT_LEN + NONCE_LEN + 16 {
            return Err(DteError::CertDecrypt(format!(
                "blob demasiado corto: {} bytes",
                self.blob.len()
            )));
        }
        let (salt, rest) = self.blob.split_at(SALT_LEN);
        let (nonce, ct) = rest.split_at(NONCE_LEN);
        Ok((salt, nonce, ct))
    }
}

fn derive_key(
    passphrase: &str,
    salt: &[u8],
    kdf: &KdfParams,
) -> Result<Zeroizing<[u8; KEY_LEN]>, DteError> {
    let argon = kdf.to_argon2()?;
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon
        .hash_password_into(passphrase.as_bytes(), salt, key.as_mut_slice())
        .map_err(|e| DteError::CertEncrypt(format!("Argon2 derive: {e}")))?;
    Ok(key)
}

/// Cifra un PFX con AES-256-GCM, clave derivada de `passphrase` vía Argon2id
/// con `KdfParams::default()`. Retorna blob listo para persistir + parámetros
/// KDF que deben guardarse junto al blob.
///
/// Invariantes:
/// - Salt y nonce frescos por llamada (dos encrypts del mismo plaintext +
///   passphrase producen ciphertext distintos).
/// - Passphrase nunca se loguea ni se persiste. Si el caller necesita
///   verificar la passphrase, debe decrypt-roundtrip (NO almacenar hash).
pub fn encrypt_pfx(pfx_bytes: &[u8], passphrase: &str) -> Result<EncryptedPfx, DteError> {
    if pfx_bytes.is_empty() {
        return Err(DteError::CertEncrypt("PFX vacío".to_string()));
    }
    if passphrase.is_empty() {
        return Err(DteError::CertEncrypt("passphrase vacía".to_string()));
    }
    let kdf = KdfParams::default();
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let key = derive_key(passphrase, &salt, &kdf)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_slice()));
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(
            nonce,
            Payload {
                msg: pfx_bytes,
                aad: b"pharma-dte-cert-v1",
            },
        )
        .map_err(|e| DteError::CertEncrypt(format!("AES-GCM encrypt: {e}")))?;

    let mut blob = Vec::with_capacity(SALT_LEN + NONCE_LEN + ct.len());
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ct);

    tracing::debug!(
        pfx_size = pfx_bytes.len(),
        blob_size = blob.len(),
        argon_m = kdf.m,
        argon_t = kdf.t,
        "cert PFX cifrado"
    );
    Ok(EncryptedPfx { blob, kdf })
}

/// Descifra un `EncryptedPfx`. Pasaphrase incorrecta o blob alterado → falla
/// loud con `CertDecrypt` (GCM tag mismatch). Retorna `Zeroizing<Vec<u8>>`
/// para que el caller no acumule plaintext en heap sin wipe.
pub fn decrypt_pfx(
    encrypted: &EncryptedPfx,
    passphrase: &str,
) -> Result<Zeroizing<Vec<u8>>, DteError> {
    if passphrase.is_empty() {
        return Err(DteError::CertDecrypt("passphrase vacía".to_string()));
    }
    let (salt, nonce_bytes, ct) = encrypted.split()?;
    let key = derive_key(passphrase, salt, &encrypted.kdf)
        .map_err(|e| DteError::CertDecrypt(format!("KDF: {e}")))?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key.as_slice()));
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ct,
                aad: b"pharma-dte-cert-v1",
            },
        )
        .map_err(|_| {
            // GCM no distingue tag-mismatch de ciphertext truncado a propósito;
            // ambos casos son "passphrase incorrecta o blob corrupto". No
            // exponemos detalle interno para no facilitar oráculos.
            DteError::CertDecrypt(
                "passphrase incorrecta o blob alterado (GCM tag mismatch)".to_string(),
            )
        })?;
    Ok(Zeroizing::new(plaintext))
}

/// Persiste un cert cifrado en `cert_digital`. Marca `activo = true` y guarda
/// la vigencia + RUT del propietario para query de cert vigente.
///
/// El cert plaintext / passphrase NO se loguean. Solo metadatos no-sensibles.
pub async fn store_cert<C: Connection>(
    db: &Surreal<C>,
    tenant: TenantId,
    encrypted: &EncryptedPfx,
    vigencia: (DateTime<Utc>, DateTime<Utc>),
    rut: &str,
) -> Result<Thing, DteError> {
    let (desde, hasta) = vigencia;
    if hasta < desde {
        return Err(DteError::CertInvalid(format!(
            "vigencia invertida: {desde} > {hasta}"
        )));
    }
    let kdf_bytes = serde_json::to_vec(&encrypted.kdf)?;
    let tenant_thing = Thing::from(("tenant", tenant.as_str()));
    let res = db
        .query(
            "CREATE cert_digital SET \
                tenant = $tenant, \
                rut_propietario = $rut, \
                pfx_blob = $pfx, \
                password_encrypted = $kdf, \
                vigencia_desde = $desde, \
                vigencia_hasta = $hasta, \
                activo = true, \
                created_at = time::now() \
             RETURN id",
        )
        .bind(("tenant", tenant_thing))
        .bind(("rut", rut.to_string()))
        .bind(("pfx", surrealdb::sql::Bytes::from(encrypted.blob.clone())))
        .bind(("kdf", surrealdb::sql::Bytes::from(kdf_bytes)))
        .bind(("desde", desde))
        .bind(("hasta", hasta))
        .await
        .map_err(|e| DteError::CertEncrypt(format!("persist cert: {e}")))?;
    let mut res = res;
    #[derive(Deserialize)]
    struct IdOnly {
        id: Thing,
    }
    let rows: Vec<IdOnly> = res
        .take(0)
        .map_err(|e| DteError::CertEncrypt(format!("decode cert id: {e}")))?;
    let id = rows
        .into_iter()
        .next()
        .ok_or_else(|| DteError::CertEncrypt("CREATE cert_digital no retornó id".to_string()))?
        .id;
    tracing::debug!(rut = rut, ?desde, ?hasta, "cert digital persistido");
    Ok(id)
}

/// Record raw de `cert_digital` en SurrealDB.
#[derive(Debug, Deserialize)]
struct CertRecord {
    pfx_blob: surrealdb::sql::Bytes,
    password_encrypted: surrealdb::sql::Bytes,
}

/// Carga el cert vigente más reciente para el tenant. Retorna `None` si no
/// hay cert activo. NO descifra: el caller debe llamar `decrypt_pfx` con la
/// passphrase que el operador suministra.
pub async fn load_cert<C: Connection>(
    db: &Surreal<C>,
    tenant: TenantId,
) -> Result<Option<EncryptedPfx>, DteError> {
    let tenant_thing = Thing::from(("tenant", tenant.as_str()));
    let mut res = db
        .query(
            "SELECT pfx_blob, password_encrypted, created_at FROM cert_digital \
             WHERE tenant = $tenant AND activo = true \
               AND vigencia_desde <= time::now() AND vigencia_hasta >= time::now() \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(("tenant", tenant_thing))
        .await
        .map_err(|e| DteError::CertInvalid(format!("query cert vigente: {e}")))?;
    let rows: Vec<CertRecord> = res
        .take(0)
        .map_err(|e| DteError::CertInvalid(format!("decode cert vigente: {e}")))?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(None);
    };
    let kdf: KdfParams = serde_json::from_slice(&row.password_encrypted)
        .map_err(|e| DteError::CertInvalid(format!("kdf params corrupt: {e}")))?;
    Ok(Some(EncryptedPfx {
        blob: row.pfx_blob.into(),
        kdf,
    }))
}

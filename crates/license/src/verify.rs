//! Parse + verify license JSON. Offline, no network, no clock.

use agent::canonical::canonical_bytes;
use agent::identity::verify_with_did;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use ed25519_dalek::Signature;
use serde_json::Value;
use thiserror::Error;

use crate::keys::{lookup_did, LICENSER_KEYS};
use crate::schema::{License, Tier, SCHEMA_VERSION};

#[derive(Debug, Error)]
pub enum LicenseError {
    #[error("JSON inválido: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("schema_version {found} no soportada (binario stale, máx {max})", max = SCHEMA_VERSION)]
    UnsupportedSchemaVersion { found: u32 },
    #[error("key_id desconocido: {0} (binario stale o license falsa)")]
    UnknownKeyId(String),
    #[error("firma inválida")]
    InvalidSignature,
    #[error("formato inválido: {0}")]
    InvalidFormat(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Parse and cryptographically verify a license against the production
/// licenser keys embedded in this binary. Does NOT check expiry — caller
/// applies the grace-period policy from `gate::is_expired` / `is_in_grace`.
pub fn parse_and_verify(json: &[u8]) -> Result<License, LicenseError> {
    parse_and_verify_with_keys(json, LICENSER_KEYS)
}

/// Same as [`parse_and_verify`] but with a custom key table. Used by tests
/// that mint ephemeral keypairs.
pub fn parse_and_verify_with_keys(
    json: &[u8],
    keys: &[(&str, &str)],
) -> Result<License, LicenseError> {
    let value: Value = serde_json::from_slice(json)?;
    let license: License = serde_json::from_value(value.clone())?;

    if license.schema_version > SCHEMA_VERSION {
        return Err(LicenseError::UnsupportedSchemaVersion {
            found: license.schema_version,
        });
    }
    if license.expires_at.is_none() && license.tier != Tier::Free {
        return Err(LicenseError::InvalidFormat(
            "expires_at=null sólo válido para tier=free".into(),
        ));
    }

    let did = keys
        .iter()
        .find(|(k, _)| *k == license.key_id)
        .map(|(_, d)| *d)
        .or_else(|| lookup_did(&license.key_id))
        .ok_or_else(|| LicenseError::UnknownKeyId(license.key_id.clone()))?;
    if did != license.issuer_did {
        return Err(LicenseError::InvalidFormat(format!(
            "issuer_did no coincide con DID de key_id={}",
            license.key_id
        )));
    }

    let mut unsigned = value;
    unsigned
        .as_object_mut()
        .ok_or_else(|| LicenseError::InvalidFormat("root no es objeto".into()))?
        .remove("signature");
    let bytes = canonical_bytes(&unsigned);

    let sig_bytes = B64
        .decode(&license.signature)
        .map_err(|e| LicenseError::InvalidFormat(format!("signature base64 inválida: {e}")))?;
    let sig_arr: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| LicenseError::InvalidFormat("signature debe ser 64 bytes".into()))?;
    let sig = Signature::from_bytes(&sig_arr);

    verify_with_did(did, &bytes, &sig).map_err(|_| LicenseError::InvalidSignature)?;

    Ok(license)
}

/// Helper for callers that build + sign a license (test mints, future
/// license-server cross-compat tests): compute canonical bytes of `value`
/// with `signature` removed.
pub fn canonical_unsigned_bytes(value: &Value) -> Result<Vec<u8>, LicenseError> {
    let mut v = value.clone();
    v.as_object_mut()
        .ok_or_else(|| LicenseError::InvalidFormat("root no es objeto".into()))?
        .remove("signature");
    Ok(canonical_bytes(&v))
}

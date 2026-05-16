//! Node identity — an Ed25519 keypair generated at install and persisted to
//! disk with restrictive perms. The public key, base58-encoded, forms the
//! node's stable decentralized identifier:
//!
//! ```text
//! did:pharma:<bs58(32-byte ed25519 pubkey)>
//! ```
//!
//! The on-disk key file stores ONLY the 32-byte signing seed, hex-encoded,
//! on a single line. Losing it = losing the node's identity (a new keypair =
//! a new DID); back it up alongside the SurrealKv data dir.

use std::path::Path;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

use crate::{AgentError, Result};

pub const DID_PREFIX: &str = "did:pharma:";

pub struct Identity {
    signing: SigningKey,
}

impl Identity {
    /// Generate a fresh random identity (install-time).
    pub fn generate() -> Self {
        let mut csprng = rand::rngs::OsRng;
        Self {
            signing: SigningKey::generate(&mut csprng),
        }
    }

    /// Reconstruct from a 32-byte hex seed (as written by [`Self::save`]).
    pub fn from_hex_seed(hex_seed: &str) -> Result<Self> {
        let bytes = hex::decode(hex_seed.trim())
            .map_err(|e| AgentError::Key(format!("hex inválido: {e}")))?;
        let arr: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| AgentError::Key("seed debe ser 32 bytes".into()))?;
        Ok(Self {
            signing: SigningKey::from_bytes(&arr),
        })
    }

    /// Persist the seed (hex, single line). Best-effort `0600` on Unix; on
    /// Windows the file inherits the data dir ACL (service runs as LocalSystem).
    pub fn save(&self, path: &Path) -> Result<()> {
        let hex_seed = hex::encode(self.signing.to_bytes());
        std::fs::write(path, format!("{hex_seed}\n"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(path, perms)?;
        }
        Ok(())
    }

    /// Load an identity previously written by [`Self::save`].
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_hex_seed(&content)
    }

    /// Load if the file exists, otherwise generate + save (install idempotent).
    pub fn load_or_init(path: &Path) -> Result<Self> {
        if path.exists() {
            Self::load(path)
        } else {
            let id = Self::generate();
            id.save(path)?;
            Ok(id)
        }
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    /// `did:pharma:<bs58(pubkey)>`.
    pub fn did(&self) -> String {
        did_from_verifying_key(&self.verifying_key())
    }

    pub fn sign(&self, msg: &[u8]) -> Signature {
        self.signing.sign(msg)
    }
}

pub fn did_from_verifying_key(vk: &VerifyingKey) -> String {
    format!("{DID_PREFIX}{}", bs58::encode(vk.to_bytes()).into_string())
}

/// Parse a `did:pharma:<bs58>` back into a verifying key.
pub fn verifying_key_from_did(did: &str) -> Result<VerifyingKey> {
    let b58 = did
        .strip_prefix(DID_PREFIX)
        .ok_or_else(|| AgentError::BadDid(format!("falta prefijo {DID_PREFIX}")))?;
    let bytes = bs58::decode(b58)
        .into_vec()
        .map_err(|e| AgentError::BadDid(format!("base58 inválido: {e}")))?;
    let arr: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| AgentError::BadDid("pubkey debe ser 32 bytes".into()))?;
    VerifyingKey::from_bytes(&arr).map_err(|e| AgentError::BadDid(e.to_string()))
}

/// Verify `sig` over `msg` against the public key embedded in `did`.
pub fn verify_with_did(did: &str, msg: &[u8], sig: &Signature) -> Result<()> {
    let vk = verifying_key_from_did(did)?;
    vk.verify(msg, sig).map_err(|_| AgentError::BadSignature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn did_roundtrips_through_parse() {
        let id = Identity::generate();
        let did = id.did();
        assert!(did.starts_with(DID_PREFIX));
        let vk = verifying_key_from_did(&did).unwrap();
        assert_eq!(vk.to_bytes(), id.verifying_key().to_bytes());
    }

    #[test]
    fn sign_then_verify_with_did() {
        let id = Identity::generate();
        let msg = b"quote.request payload";
        let sig = id.sign(msg);
        verify_with_did(&id.did(), msg, &sig).expect("valid sig");
        // Tampered message fails.
        assert!(verify_with_did(&id.did(), b"tampered", &sig).is_err());
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("agent.key");
        let id = Identity::generate();
        id.save(&p).unwrap();
        let loaded = Identity::load(&p).unwrap();
        assert_eq!(loaded.did(), id.did());
    }

    #[test]
    fn load_or_init_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("agent.key");
        let a = Identity::load_or_init(&p).unwrap();
        let b = Identity::load_or_init(&p).unwrap();
        assert_eq!(a.did(), b.did());
    }
}

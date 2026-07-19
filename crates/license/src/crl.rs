//! CRL (Certificate Revocation List) cliente — ADR-0006.
//!
//! El licenser publica revocaciones como **CRL versionado incremental** vía
//! CDN: cada `crl-v{N}.json` es un diff firmado Ed25519 (misma cadena de
//! claves `keys::LICENSER_KEYS` + canonical-JSON que la license). El nodo
//! mantiene un cache local (`data/crl_state.json`) con la última versión
//! vista y el set acumulado de `license_id` revocados.
//!
//! Este módulo es **puro** (sin red): parse + verify + apply + persist. El
//! fetch HTTP vive en la capa que ya tiene cliente HTTP (refresh job en
//! `crates/api`), manteniendo el crate `license` 100% offline-testeable.
//!
//! Invariantes ADR-0005/0006:
//! - Revocación NUNCA bloquea el core gratis: una license revocada degrada a
//!   `License::free_default`, no a un kill-switch.
//! - Offline graceful: sin red, el nodo opera con el último CRL conocido
//!   (o ninguno — `CrlState::default()` no revoca nada).
//! - Monotonicidad: las versiones sólo avanzan; aplicar una versión que no
//!   es `last_seen + 1` es error (el caller decide pedir snapshot).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chrono::{DateTime, Utc};
use ed25519_dalek::Signature;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::keys::{lookup_did, LICENSER_KEYS};
use crate::verify::LicenseError;
use agent::canonical::canonical_bytes;
use agent::identity::verify_with_did;

/// Entrada de revocación (elemento de `diff.added` y de los snapshots).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevokedEntry {
    pub license_id: String,
    pub revoked_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Diff de una versión CRL. `removed` típicamente vacío (revocaciones son
/// permanentes; existe para garbage-collection de licenses ya expiradas).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CrlDiff {
    #[serde(default)]
    pub added: Vec<RevokedEntry>,
    #[serde(default)]
    pub removed: Vec<String>,
}

/// Una versión CRL publicada (`crl-v{N}.json`), firmada por el licenser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrlVersion {
    pub schema_version: u32,
    pub crl_version: u64,
    /// `None` sólo en la v1 (raíz de la cadena).
    pub previous_version: Option<u64>,
    pub published_at: DateTime<Utc>,
    pub diff: CrlDiff,
    pub issuer_did: String,
    pub key_id: String,
    /// Base64 Ed25519 sobre el canonical-JSON sin `signature`.
    pub signature: String,
}

/// Snapshot completo (`snapshot-v{N}.json`): lista total de revocados hasta
/// `crl_version = N`. Lo usa un nodo nuevo o uno con gap > 100 versiones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrlSnapshot {
    pub schema_version: u32,
    pub crl_version: u64,
    pub published_at: DateTime<Utc>,
    pub revoked: Vec<RevokedEntry>,
    pub issuer_did: String,
    pub key_id: String,
    pub signature: String,
}

/// Cache local persistido en `data/crl_state.json`. Sin estado (archivo
/// ausente) ⇒ nada revocado — el nodo nunca arranca bloqueado.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CrlState {
    pub last_seen_version: u64,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
    pub revoked: BTreeSet<String>,
}

impl CrlState {
    /// `true` si la license fue revocada según el último CRL conocido.
    pub fn is_revoked(&self, license_id: &str) -> bool {
        self.revoked.contains(license_id)
    }

    /// Aplica un diff verificado. Exige continuidad de cadena:
    /// `crl_version == last_seen_version + 1` (o cualquier versión si el
    /// estado está vacío y la versión trae `previous_version == None`).
    pub fn apply_version(&mut self, v: &CrlVersion) -> Result<(), LicenseError> {
        let expected = self.last_seen_version + 1;
        let chain_root = self.last_seen_version == 0 && v.previous_version.is_none();
        if !chain_root && v.crl_version != expected {
            return Err(LicenseError::InvalidFormat(format!(
                "CRL fuera de secuencia: esperaba v{expected}, llegó v{} (pedir snapshot)",
                v.crl_version
            )));
        }
        for e in &v.diff.added {
            self.revoked.insert(e.license_id.clone());
        }
        for id in &v.diff.removed {
            self.revoked.remove(id);
        }
        self.last_seen_version = v.crl_version;
        self.updated_at = Some(Utc::now());
        Ok(())
    }

    /// Reemplaza el estado completo desde un snapshot verificado. Rechaza
    /// snapshots más viejos que lo ya visto (rollback).
    pub fn apply_snapshot(&mut self, s: &CrlSnapshot) -> Result<(), LicenseError> {
        if s.crl_version < self.last_seen_version {
            return Err(LicenseError::InvalidFormat(format!(
                "snapshot v{} es más viejo que el estado local v{} (rollback rechazado)",
                s.crl_version, self.last_seen_version
            )));
        }
        self.revoked = s.revoked.iter().map(|e| e.license_id.clone()).collect();
        self.last_seen_version = s.crl_version;
        self.updated_at = Some(Utc::now());
        Ok(())
    }
}

/// Path por defecto del cache CRL, hermano de `license.json`.
pub fn default_crl_state_path(data_dir: &Path) -> PathBuf {
    data_dir.join("crl_state.json")
}

/// Carga el cache local. Archivo ausente ⇒ `CrlState::default()` (nada
/// revocado). Archivo corrupto ⇒ error (el caller loguea y puede resetear).
pub fn load_state(path: &Path) -> Result<CrlState, LicenseError> {
    if !path.exists() {
        return Ok(CrlState::default());
    }
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Persiste el cache local pretty-printed (inspeccionable en soporte).
pub fn save_state(state: &CrlState, path: &Path) -> Result<(), LicenseError> {
    let json = serde_json::to_vec_pretty(state)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Parse + verify de una versión CRL con las claves embebidas de producción.
pub fn parse_and_verify_crl(json: &[u8]) -> Result<CrlVersion, LicenseError> {
    parse_and_verify_crl_with_keys(json, LICENSER_KEYS)
}

/// Idem con tabla de claves custom (tests con keypair efímero).
pub fn parse_and_verify_crl_with_keys(
    json: &[u8],
    keys: &[(&str, &str)],
) -> Result<CrlVersion, LicenseError> {
    let value: Value = serde_json::from_slice(json)?;
    let crl: CrlVersion = serde_json::from_value(value.clone())?;
    verify_signed_value(&value, &crl.key_id, &crl.issuer_did, &crl.signature, keys)?;
    Ok(crl)
}

/// Parse + verify de un snapshot CRL (claves de producción).
pub fn parse_and_verify_snapshot(json: &[u8]) -> Result<CrlSnapshot, LicenseError> {
    parse_and_verify_snapshot_with_keys(json, LICENSER_KEYS)
}

/// Idem con tabla de claves custom.
pub fn parse_and_verify_snapshot_with_keys(
    json: &[u8],
    keys: &[(&str, &str)],
) -> Result<CrlSnapshot, LicenseError> {
    let value: Value = serde_json::from_slice(json)?;
    let snap: CrlSnapshot = serde_json::from_value(value.clone())?;
    verify_signed_value(
        &value,
        &snap.key_id,
        &snap.issuer_did,
        &snap.signature,
        keys,
    )?;
    Ok(snap)
}

/// Núcleo compartido: resolver `key_id` → DID, exigir que coincida con el
/// `issuer_did` declarado y verificar la firma Ed25519 sobre el
/// canonical-JSON sin `signature` — mismo esquema que `verify::parse_and_verify`.
fn verify_signed_value(
    value: &Value,
    key_id: &str,
    issuer_did: &str,
    signature_b64: &str,
    keys: &[(&str, &str)],
) -> Result<(), LicenseError> {
    let did = keys
        .iter()
        .find(|(k, _)| *k == key_id)
        .map(|(_, d)| *d)
        .or_else(|| lookup_did(key_id))
        .ok_or_else(|| LicenseError::UnknownKeyId(key_id.to_string()))?;
    if did != issuer_did {
        return Err(LicenseError::InvalidFormat(format!(
            "issuer_did no coincide con DID de key_id={key_id}"
        )));
    }
    let mut unsigned = value.clone();
    unsigned
        .as_object_mut()
        .ok_or_else(|| LicenseError::InvalidFormat("root no es objeto".into()))?
        .remove("signature");
    let bytes = canonical_bytes(&unsigned)
        .map_err(|e| LicenseError::InvalidFormat(format!("canonical: {e}")))?;
    let sig_bytes = B64
        .decode(signature_b64.as_bytes())
        .map_err(|e| LicenseError::InvalidFormat(format!("signature base64 inválida: {e}")))?;
    let sig_arr: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| LicenseError::InvalidFormat("signature debe ser 64 bytes".into()))?;
    let sig = Signature::from_bytes(&sig_arr);
    verify_with_did(did, &bytes, &sig).map_err(|_| LicenseError::InvalidSignature)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent::identity::Identity;
    use serde_json::json;

    /// Keypair efímero + tabla de claves para firmar CRLs de test.
    fn test_keys() -> (Identity, String) {
        let id = Identity::generate();
        (id, "lk-test".to_string())
    }

    fn sign_value(id: &Identity, mut v: Value) -> Value {
        let unsigned = {
            let mut u = v.clone();
            u.as_object_mut().unwrap().remove("signature");
            canonical_bytes(&u).unwrap()
        };
        let sig = id.sign(&unsigned);
        v["signature"] = Value::String(B64.encode(sig.to_bytes()));
        v
    }

    fn crl_v(
        id: &Identity,
        key_id: &str,
        version: u64,
        prev: Option<u64>,
        added: &[&str],
    ) -> Vec<u8> {
        let v = sign_value(
            id,
            json!({
                "schema_version": 1,
                "crl_version": version,
                "previous_version": prev,
                "published_at": "2026-06-10T00:00:00Z",
                "diff": {
                    "added": added.iter().map(|l| json!({
                        "license_id": l,
                        "revoked_at": "2026-06-09T00:00:00Z",
                        "reason": "refund",
                    })).collect::<Vec<_>>(),
                    "removed": [],
                },
                "issuer_did": id.did(),
                "key_id": key_id,
                "signature": "",
            }),
        );
        serde_json::to_vec(&v).unwrap()
    }

    #[test]
    fn verify_apply_chain_and_revoke() {
        let (id, key_id) = test_keys();
        let did = id.did();
        let keys: &[(&str, &str)] = &[(&key_id, &did)];

        let v1 = crl_v(&id, &key_id, 1, None, &["lic_a"]);
        let v2 = crl_v(&id, &key_id, 2, Some(1), &["lic_b"]);

        let mut state = CrlState::default();
        let c1 = parse_and_verify_crl_with_keys(&v1, keys).expect("v1 verifica");
        state.apply_version(&c1).expect("v1 aplica");
        let c2 = parse_and_verify_crl_with_keys(&v2, keys).expect("v2 verifica");
        state.apply_version(&c2).expect("v2 aplica");

        assert_eq!(state.last_seen_version, 2);
        assert!(state.is_revoked("lic_a"));
        assert!(state.is_revoked("lic_b"));
        assert!(!state.is_revoked("lic_c"));
    }

    #[test]
    fn out_of_sequence_rejected() {
        let (id, key_id) = test_keys();
        let did = id.did();
        let keys: &[(&str, &str)] = &[(&key_id, &did)];
        let v3 = crl_v(&id, &key_id, 3, Some(2), &["lic_x"]);
        let c3 = parse_and_verify_crl_with_keys(&v3, keys).unwrap();
        let mut state = CrlState::default();
        let err = state.apply_version(&c3).unwrap_err();
        assert!(format!("{err}").contains("fuera de secuencia"));
        assert!(!state.is_revoked("lic_x"), "diff rechazado no aplica");
    }

    #[test]
    fn tampered_signature_rejected() {
        let (id, key_id) = test_keys();
        let did = id.did();
        let keys: &[(&str, &str)] = &[(&key_id, &did)];
        let v1 = crl_v(&id, &key_id, 1, None, &["lic_a"]);
        let tampered = String::from_utf8(v1).unwrap().replace("lic_a", "lic_z");
        let err = parse_and_verify_crl_with_keys(tampered.as_bytes(), keys).unwrap_err();
        assert!(matches!(err, LicenseError::InvalidSignature));
    }

    #[test]
    fn unknown_key_rejected() {
        let (id, key_id) = test_keys();
        let v1 = crl_v(&id, &key_id, 1, None, &["lic_a"]);
        let err = parse_and_verify_crl_with_keys(&v1, &[]).unwrap_err();
        assert!(matches!(err, LicenseError::UnknownKeyId(_)));
    }

    #[test]
    fn snapshot_replaces_and_rejects_rollback() {
        let (id, key_id) = test_keys();
        let did = id.did();
        let keys: &[(&str, &str)] = &[(&key_id, &did)];
        let snap = sign_value(
            &id,
            json!({
                "schema_version": 1,
                "crl_version": 50,
                "published_at": "2026-06-10T00:00:00Z",
                "revoked": [
                    {"license_id": "lic_s1", "revoked_at": "2026-01-01T00:00:00Z"},
                    {"license_id": "lic_s2", "revoked_at": "2026-02-01T00:00:00Z", "reason": "fraud"},
                ],
                "issuer_did": id.did(),
                "key_id": key_id,
                "signature": "",
            }),
        );
        let bytes = serde_json::to_vec(&snap).unwrap();
        let s = parse_and_verify_snapshot_with_keys(&bytes, keys).expect("snapshot verifica");

        let mut state = CrlState {
            last_seen_version: 10,
            updated_at: None,
            revoked: ["lic_old".to_string()].into_iter().collect(),
        };
        state.apply_snapshot(&s).expect("snapshot aplica");
        assert_eq!(state.last_seen_version, 50);
        assert!(state.is_revoked("lic_s1") && state.is_revoked("lic_s2"));
        assert!(!state.is_revoked("lic_old"), "snapshot reemplaza, no une");

        // Rollback: snapshot más viejo que el estado → rechazado.
        let mut newer = CrlState {
            last_seen_version: 99,
            ..CrlState::default()
        };
        assert!(newer.apply_snapshot(&s).is_err());
    }

    #[test]
    fn state_roundtrip_disk() {
        let dir = std::env::temp_dir().join(format!("crl-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = default_crl_state_path(&dir);

        // Ausente ⇒ default sin revocados.
        let empty = load_state(&path).unwrap();
        assert_eq!(empty.last_seen_version, 0);

        let mut st = CrlState::default();
        st.revoked.insert("lic_a".into());
        st.last_seen_version = 7;
        save_state(&st, &path).unwrap();
        let back = load_state(&path).unwrap();
        assert_eq!(back.last_seen_version, 7);
        assert!(back.is_revoked("lic_a"));
        std::fs::remove_dir_all(&dir).ok();
    }
}

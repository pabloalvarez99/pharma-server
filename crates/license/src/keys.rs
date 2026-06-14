//! Licenser public keys embedded in the binary. Multi-key to support
//! rotation without invalidating licenses signed by an older key.
//! Procedure: ADR-0007. Schema: `(key_id, did:pharma:<bs58 pubkey>)`.

/// `key_id` assigned to licenses minted before the `key_id` field existed
/// (ADR-0007 backward-compat). A license whose `key_id` is absent/empty is
/// verified against this entry, so introducing rotation never invalidates a
/// historical license. MUST remain the entry of the original (pre-rotation)
/// signing key for as long as any such license can still be in the wild.
pub const LEGACY_KEY_ID: &str = "lk-prod-2026-01";

/// One embedded licenser key with rotation metadata (ADR-0007 trust store).
pub struct KeyEntry {
    /// Stable identifier carried in `License.key_id` / `Crl.key_id`.
    pub key_id: &'static str,
    /// `did:pharma:<bs58 of the Ed25519 pubkey>`.
    pub did: &'static str,
    /// `true` while this key still validates in-the-wild licenses. Set `false`
    /// once monitoring confirms every license signed with it has expired; a
    /// retired key stays embedded for audit but is rejected by `verify`.
    pub accepted: bool,
}

/// Embedded licenser trust store. Multi-entry to validate licenses signed by any
/// historically-valid key (rotation policy: ADR-0007). The active emitter key is
/// the first `accepted` entry by convention; older entries remain to validate
/// licenses issued before the most recent rotation.
///
/// Storage: corresponding private seed lives in
/// `pharma-license-server/.secrets/prod-key.json` (gitignored, staging only)
/// and is loaded into Vercel encrypted env as `LICENSER_PRIVATE_KEY_SEED`.
/// Production migrates to GCP KMS asymmetric Ed25519 before Fase 11b — see
/// `pharma-license-server/docs/adr/0008-kms-strategy.md`.
pub const TRUST_STORE: &[KeyEntry] = &[
    // Active emitter (Fase 11a staging — bound to env-stored seed on Vercel).
    // Doubles as the legacy key (LEGACY_KEY_ID) for pre-key_id licenses.
    KeyEntry {
        key_id: "lk-prod-2026-01",
        did: "did:pharma:HbL8Gfa3x4HEGseE8jqa85NyA1pRg58D6ZbMfV4C5Ep9",
        accepted: true,
    },
    // Dev placeholder kept for legacy local-only tests that hardcode it.
    KeyEntry {
        key_id: "lk-dev-2026",
        did: "did:pharma:11111111111111111111111111111111",
        accepted: true,
    },
];

/// Backward-compatible flat view `(key_id, did)` for callers that only need the
/// pubkey lookup (CRL verify, API key injection). Mirrors `TRUST_STORE`.
pub const LICENSER_KEYS: &[(&str, &str)] = &[
    (
        "lk-prod-2026-01",
        "did:pharma:HbL8Gfa3x4HEGseE8jqa85NyA1pRg58D6ZbMfV4C5Ep9",
    ),
    ("lk-dev-2026", "did:pharma:11111111111111111111111111111111"),
];

/// Look up a DID by `key_id`, applying the ADR-0007 legacy fallback: an empty
/// `key_id` resolves to [`LEGACY_KEY_ID`]. Returns `None` if the resolved
/// `key_id` is not embedded (license signed by a key not in this binary —
/// usually means the binary is stale). Does NOT enforce `accepted`; callers
/// that gate on retirement use [`is_accepted`].
pub fn lookup_did(key_id: &str) -> Option<&'static str> {
    let resolved = resolve_key_id(key_id);
    LICENSER_KEYS
        .iter()
        .find(|(k, _)| *k == resolved)
        .map(|(_, did)| *did)
}

/// Normalize a license/CRL `key_id`: empty → [`LEGACY_KEY_ID`] (ADR-0007).
pub fn resolve_key_id(key_id: &str) -> &str {
    if key_id.is_empty() {
        LEGACY_KEY_ID
    } else {
        key_id
    }
}

/// Whether a `key_id` is currently accepted for verification (embedded AND not
/// retired). Empty `key_id` resolves to [`LEGACY_KEY_ID`] first.
pub fn is_accepted(key_id: &str) -> bool {
    let resolved = resolve_key_id(key_id);
    TRUST_STORE
        .iter()
        .find(|e| e.key_id == resolved)
        .map(|e| e.accepted)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_store_and_flat_view_stay_in_sync() {
        assert_eq!(TRUST_STORE.len(), LICENSER_KEYS.len());
        for (entry, (kid, did)) in TRUST_STORE.iter().zip(LICENSER_KEYS.iter()) {
            assert_eq!(
                entry.key_id, *kid,
                "key_id drift between TRUST_STORE and LICENSER_KEYS"
            );
            assert_eq!(
                entry.did, *did,
                "did drift between TRUST_STORE and LICENSER_KEYS"
            );
        }
    }

    #[test]
    fn empty_key_id_resolves_to_legacy() {
        assert_eq!(resolve_key_id(""), LEGACY_KEY_ID);
        assert_eq!(resolve_key_id("lk-dev-2026"), "lk-dev-2026");
        assert_eq!(lookup_did(""), lookup_did(LEGACY_KEY_ID));
    }

    #[test]
    fn legacy_key_id_is_embedded() {
        assert!(
            LICENSER_KEYS.iter().any(|(k, _)| *k == LEGACY_KEY_ID),
            "LEGACY_KEY_ID must reference an embedded key"
        );
    }

    #[test]
    fn unknown_key_id_not_accepted() {
        assert!(!is_accepted("lk-does-not-exist"));
        assert!(is_accepted("lk-prod-2026-01"));
        assert!(is_accepted("")); // legacy fallback accepted
    }
}

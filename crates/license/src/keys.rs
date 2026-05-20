//! Licenser public keys embedded in the binary. Multi-key to support
//! rotation without invalidating licenses signed by an older key.
//! Procedure: ADR-0007. Schema: `(key_id, did:pharma:<bs58 pubkey>)`.

/// Embedded licenser pubkeys. The real production key is injected once
/// `pharma-license-server` (Fase 11a, separate repo) generates its KMS
/// keypair. The placeholder below is intentionally invalid for production:
/// it cannot verify real signatures and tests construct their own ephemeral
/// DIDs via [`crate::verify::tests`] helpers.
pub const LICENSER_KEYS: &[(&str, &str)] = &[
    // (key_id, did:pharma:<bs58 pubkey>)
    // Placeholder — replaced by real production key when license-server lands.
    ("lk-dev-2026", "did:pharma:11111111111111111111111111111111"),
];

/// Look up a DID by `key_id`. Returns `None` if unknown (license signed by a
/// key not embedded in this binary — usually means binary is stale).
pub fn lookup_did(key_id: &str) -> Option<&'static str> {
    LICENSER_KEYS
        .iter()
        .find(|(k, _)| *k == key_id)
        .map(|(_, did)| *did)
}

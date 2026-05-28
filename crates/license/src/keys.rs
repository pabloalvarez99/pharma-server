//! Licenser public keys embedded in the binary. Multi-key to support
//! rotation without invalidating licenses signed by an older key.
//! Procedure: ADR-0007. Schema: `(key_id, did:pharma:<bs58 pubkey>)`.

/// Embedded licenser pubkeys. Multi-entry to validate licenses signed by any
/// historically-valid key (rotation policy: ADR-0007). The active emitter key
/// is the first entry by convention; older entries remain to validate licenses
/// issued before the most recent rotation.
///
/// Storage: corresponding private seed lives in
/// `pharma-license-server/.secrets/prod-key.json` (gitignored, staging only)
/// and is loaded into Vercel encrypted env as `LICENSER_PRIVATE_KEY_SEED`.
/// Production migrates to GCP KMS asymmetric Ed25519 before Fase 11b — see
/// `pharma-license-server/docs/adr/0008-kms-strategy.md`.
pub const LICENSER_KEYS: &[(&str, &str)] = &[
    // (key_id, did:pharma:<bs58 pubkey>)
    // Active emitter (Fase 11a staging — bound to env-stored seed on Vercel).
    (
        "lk-prod-2026-01",
        "did:pharma:HbL8Gfa3x4HEGseE8jqa85NyA1pRg58D6ZbMfV4C5Ep9",
    ),
    // Dev placeholder kept for legacy local-only tests that hardcode it.
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

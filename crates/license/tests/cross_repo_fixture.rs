//! Cross-repo contract test. Loads a license signed by the TS impl in
//! `pharma-license-server/scripts/generate-fixture.ts` and verifies it with
//! the Rust verifier. If this passes, the TS canonical-JSON encoder is
//! bit-exact with `crates/agent/src/canonical.rs`. If it fails, ONE OF the two
//! encoders has drifted — fix before shipping anything that signs licenses.
//!
//! Regenerate fixture: `cd pharma-license-server && npx tsx
//! scripts/generate-fixture.ts && cp fixtures/cross-repo-v1.* ../pharma-server/
//! crates/license/tests/fixtures/`.
//!
//! The fixture seed is a public, test-only 32-byte buffer of `0x42`. Never use
//! that seed for any real key.

use license::schema::Tier;
use license::verify::parse_and_verify_with_keys;

const FIXTURE_LIC: &[u8] = include_bytes!("fixtures/cross-repo-v1.lic");
const FIXTURE_DID: &str = "did:pharma:3F5qRPtKg8GhGNnbd3qCj6nVJxWsGxq7pvH84okYLAqf";
const FIXTURE_KEY_ID: &str = "lk-fixture-2026";

#[test]
fn ts_signed_license_verifies_in_rust() {
    let keys: &[(&str, &str)] = &[(FIXTURE_KEY_ID, FIXTURE_DID)];
    let parsed = parse_and_verify_with_keys(FIXTURE_LIC, keys)
        .expect("TS-signed license must verify in Rust");
    assert_eq!(parsed.tier, Tier::Pro);
    assert_eq!(parsed.license_id, "lic_fixture_cross_repo_v1");
    assert_eq!(parsed.key_id, FIXTURE_KEY_ID);
    assert_eq!(parsed.seat_count, 3);
    assert!(parsed
        .features
        .iter()
        .any(|f| f == "reports.margins_daily"));
}

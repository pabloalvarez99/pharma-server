mod common;

use chrono::{Duration, Utc};
use common::{build_license, keys_as_refs, mint};
use license::schema::Tier;
use license::verify::{parse_and_verify_with_keys, LicenseError};

#[test]
fn unknown_key_id_rejected() {
    let lic = build_license(
        Tier::Pro,
        vec!["reports.margins_daily"],
        Some(Utc::now() + Duration::days(30)),
    );
    let minted = mint(lic);
    // Empty key table → key_id "test-key-2026" is unknown (and the embedded
    // production placeholder doesn't match either).
    let keys: Vec<(&'static str, String)> = Vec::new();
    let err = parse_and_verify_with_keys(&minted.bytes, &keys_as_refs(&keys)).unwrap_err();
    assert!(matches!(err, LicenseError::UnknownKeyId(_)), "got {err:?}");
}

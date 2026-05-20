mod common;

use chrono::{Duration, Utc};
use common::{build_license, keys_as_refs, mint, test_keys};
use license::schema::Tier;
use license::verify::{parse_and_verify_with_keys, LicenseError};

#[test]
fn tampering_features_breaks_signature() {
    let lic = build_license(
        Tier::Pro,
        vec!["reports.margins_daily"],
        Some(Utc::now() + Duration::days(30)),
    );
    let minted = mint(lic);

    // Decode JSON, mutate features array, re-encode.
    let mut v: serde_json::Value = serde_json::from_slice(&minted.bytes).unwrap();
    v["features"] = serde_json::json!(["reports.margins_daily", "branding.white_label"]);
    let tampered = serde_json::to_vec(&v).unwrap();

    let keys = test_keys();
    let err = parse_and_verify_with_keys(&tampered, &keys_as_refs(&keys)).unwrap_err();
    assert!(matches!(err, LicenseError::InvalidSignature), "got {err:?}");
}

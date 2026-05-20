mod common;

use chrono::{Duration, Utc};
use common::{build_license, keys_as_refs, mint, test_keys};
use license::schema::Tier;
use license::verify::{parse_and_verify_with_keys, LicenseError};

#[test]
fn future_schema_version_rejected_clearly() {
    let mut lic = build_license(
        Tier::Pro,
        vec!["reports.margins_daily"],
        Some(Utc::now() + Duration::days(30)),
    );
    lic.schema_version = 999;
    let minted = mint(lic);
    let keys = test_keys();
    let err = parse_and_verify_with_keys(&minted.bytes, &keys_as_refs(&keys)).unwrap_err();
    assert!(
        matches!(err, LicenseError::UnsupportedSchemaVersion { found: 999 }),
        "got {err:?}"
    );
}

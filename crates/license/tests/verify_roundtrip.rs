mod common;

use chrono::{Duration, Utc};
use common::{build_license, keys_as_refs, mint, test_keys};
use license::schema::Tier;
use license::verify::parse_and_verify_with_keys;

#[test]
fn signed_license_verifies() {
    let lic = build_license(
        Tier::Pro,
        vec!["reports.margins_daily"],
        Some(Utc::now() + Duration::days(365)),
    );
    let minted = mint(lic);
    let keys = test_keys();
    let parsed =
        parse_and_verify_with_keys(&minted.bytes, &keys_as_refs(&keys)).expect("verify ok");
    assert_eq!(parsed.tier, Tier::Pro);
    assert_eq!(parsed.features, vec!["reports.margins_daily".to_string()]);
}

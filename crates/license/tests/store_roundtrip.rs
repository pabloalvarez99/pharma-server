mod common;

use chrono::{Duration, Utc};
use common::{build_license, keys_as_refs, mint, test_keys};
use license::schema::Tier;
use license::store::{load_from_disk, save_to_disk};
use license::verify::parse_and_verify_with_keys;
use tempfile::tempdir;

#[test]
fn save_then_load_via_keys() {
    let lic = build_license(
        Tier::Business,
        vec!["federation.po_create"],
        Some(Utc::now() + Duration::days(365)),
    );
    let minted = mint(lic);
    let keys = test_keys();
    let parsed = parse_and_verify_with_keys(&minted.bytes, &keys_as_refs(&keys)).expect("verify");

    let dir = tempdir().unwrap();
    let path = dir.path().join("license.json");
    save_to_disk(&parsed, &path).expect("save");

    // load_from_disk uses production keys, which do not include the test key,
    // so it errors — that's correct behavior. Verify the file round-trips
    // through the test key table instead.
    let raw = std::fs::read(&path).expect("read back");
    let parsed2 = parse_and_verify_with_keys(&raw, &keys_as_refs(&keys)).expect("verify2");
    assert_eq!(parsed2.tier, Tier::Business);

    // And production-key path errors as designed.
    let err = load_from_disk(&path).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("key_id desconocido") || msg.contains("firma"),
        "got {msg}"
    );
}

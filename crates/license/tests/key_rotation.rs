//! ADR-0007 key rotation: multi-key verification, legacy (no key_id) fallback,
//! and retired-key rejection. Mints licenses with two deterministic keypairs
//! (lk-A, lk-B) plus a legacy (empty key_id) variant.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chrono::{Duration, Utc};
use ed25519_dalek::{Signer, SigningKey};
use license::keys::{is_accepted, resolve_key_id, LEGACY_KEY_ID};
use license::schema::{License, Metadata, Tier, SCHEMA_VERSION};
use license::verify::{canonical_unsigned_bytes, parse_and_verify_with_keys, LicenseError};
use uuid::Uuid;

use agent::identity::did_from_verifying_key;

fn keypair(seed: u8) -> (SigningKey, String) {
    let sk = SigningKey::from_bytes(&[seed; 32]);
    let did = did_from_verifying_key(&sk.verifying_key());
    (sk, did)
}

/// Build + canonical-sign a license with the given key_id and signing key.
fn mint(key_id: &str, sk: &SigningKey, did: &str) -> Vec<u8> {
    let lic = License {
        schema_version: SCHEMA_VERSION,
        license_id: "lic_rot_01".into(),
        tenant_id: Uuid::nil(),
        tier: Tier::Pro,
        features: vec!["reports.margins_daily".into()],
        bought_addons: Vec::new(),
        seat_count: 1,
        issued_at: Utc::now() - Duration::days(1),
        expires_at: Some(Utc::now() + Duration::days(30)),
        issuer_did: did.to_string(),
        key_id: key_id.to_string(),
        signature: String::new(),
        metadata: Some(Metadata::default()),
    };
    let mut value = serde_json::to_value(&lic).expect("serialize");
    let bytes = canonical_unsigned_bytes(&value).expect("canonical");
    let sig = sk.sign(&bytes);
    value.as_object_mut().unwrap().insert(
        "signature".into(),
        serde_json::Value::String(B64.encode(sig.to_bytes())),
    );
    serde_json::to_vec(&value).expect("encode")
}

/// Build + sign a legacy license whose `key_id` field is entirely absent.
fn mint_legacy(sk: &SigningKey, did: &str) -> Vec<u8> {
    let bytes = mint("", sk, did);
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value.as_object_mut().unwrap().remove("key_id");
    // Re-sign over the canonical form WITHOUT key_id so the signature matches
    // what an old licenser (pre-key_id) would have produced.
    let mut unsigned = value.clone();
    unsigned.as_object_mut().unwrap().remove("signature");
    let cbytes = canonical_unsigned_bytes(&unsigned).expect("canonical");
    let sig = sk.sign(&cbytes);
    value.as_object_mut().unwrap().insert(
        "signature".into(),
        serde_json::Value::String(B64.encode(sig.to_bytes())),
    );
    serde_json::to_vec(&value).expect("encode")
}

#[test]
fn license_signed_by_key_a_verifies() {
    let (sk_a, did_a) = keypair(0xAA);
    let (_sk_b, did_b) = keypair(0xBB);
    let keys: &[(&str, &str)] = &[("lk-A", &did_a), ("lk-B", &did_b)];
    let bytes = mint("lk-A", &sk_a, &did_a);
    let lic = parse_and_verify_with_keys(&bytes, keys).expect("A should verify");
    assert_eq!(lic.key_id, "lk-A");
}

#[test]
fn license_signed_by_key_b_verifies() {
    let (_sk_a, did_a) = keypair(0xAA);
    let (sk_b, did_b) = keypair(0xBB);
    let keys: &[(&str, &str)] = &[("lk-A", &did_a), ("lk-B", &did_b)];
    let bytes = mint("lk-B", &sk_b, &did_b);
    let lic = parse_and_verify_with_keys(&bytes, keys).expect("B should verify");
    assert_eq!(lic.key_id, "lk-B");
}

#[test]
fn unknown_key_id_rejected() {
    let (sk_a, did_a) = keypair(0xAA);
    let (_sk_b, did_b) = keypair(0xBB);
    let keys: &[(&str, &str)] = &[("lk-A", &did_a), ("lk-B", &did_b)];
    let bytes = mint("lk-UNKNOWN", &sk_a, &did_a);
    let err = parse_and_verify_with_keys(&bytes, keys).unwrap_err();
    assert!(matches!(err, LicenseError::UnknownKeyId(_)), "got {err:?}");
}

#[test]
fn legacy_license_without_key_id_falls_back_to_legacy_key() {
    // Old license: no key_id field. Registered under LEGACY_KEY_ID.
    let (sk_legacy, did_legacy) = keypair(0xCC);
    let keys: &[(&str, &str)] = &[(LEGACY_KEY_ID, &did_legacy)];
    let bytes = mint_legacy(&sk_legacy, &did_legacy);
    // Sanity: the field really is absent in the wire form.
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(v.get("key_id").is_none(), "key_id must be absent");
    let lic = parse_and_verify_with_keys(&bytes, keys).expect("legacy should verify");
    assert_eq!(lic.key_id, "", "absent key_id deserializes to empty");
    assert_eq!(resolve_key_id(&lic.key_id), LEGACY_KEY_ID);
}

#[test]
fn wrong_did_for_key_id_rejected() {
    // key_id maps to a DID, but the license was signed by a different key.
    let (sk_a, did_a) = keypair(0xAA);
    let (_sk_b, did_b) = keypair(0xBB);
    // Table maps lk-A → did_b (wrong), so issuer_did/did mismatch triggers.
    let keys: &[(&str, &str)] = &[("lk-A", &did_b)];
    let bytes = mint("lk-A", &sk_a, &did_a);
    let err = parse_and_verify_with_keys(&bytes, keys).unwrap_err();
    assert!(matches!(err, LicenseError::InvalidFormat(_)), "got {err:?}");
}

#[test]
fn retirement_helper_reflects_trust_store() {
    // Embedded keys are accepted; legacy fallback (empty) is accepted; an
    // arbitrary unknown id is not.
    assert!(is_accepted(LEGACY_KEY_ID));
    assert!(is_accepted("")); // empty → LEGACY_KEY_ID
    assert!(!is_accepted("lk-retired-2099"));
}

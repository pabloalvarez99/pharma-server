#![allow(dead_code)]
//! Shared test helpers: mint a deterministic licenser keypair, build a
//! License struct, canonical-JSON-sign it, return serialized bytes plus the
//! key table to pass into `parse_and_verify_with_keys`.

use agent::identity::did_from_verifying_key;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signer, SigningKey};
use license::schema::{License, Metadata, Tier, SCHEMA_VERSION};
use license::verify::canonical_unsigned_bytes;
use uuid::Uuid;

pub const TEST_KEY_ID: &str = "test-key-2026";

pub struct MintedLicense {
    pub bytes: Vec<u8>,
    pub did: String,
    pub license: License,
}

pub fn test_signing_key() -> SigningKey {
    // Deterministic seed so failures are reproducible.
    SigningKey::from_bytes(&[7u8; 32])
}

pub fn test_did() -> String {
    did_from_verifying_key(&test_signing_key().verifying_key())
}

pub fn build_license(
    tier: Tier,
    features: Vec<&str>,
    expires_at: Option<DateTime<Utc>>,
) -> License {
    License {
        schema_version: SCHEMA_VERSION,
        license_id: "lic_test_01".into(),
        tenant_id: Uuid::nil(),
        tier,
        features: features.into_iter().map(String::from).collect(),
        bought_addons: Vec::new(),
        seat_count: 1,
        issued_at: Utc::now() - Duration::days(1),
        expires_at,
        issuer_did: test_did(),
        key_id: TEST_KEY_ID.into(),
        signature: String::new(),
        metadata: Some(Metadata::default()),
    }
}

pub fn mint(license: License) -> MintedLicense {
    let sk = test_signing_key();
    let mut value = serde_json::to_value(&license).expect("serialize");
    let bytes = canonical_unsigned_bytes(&value).expect("canonical");
    let sig = sk.sign(&bytes);
    let sig_b64 = B64.encode(sig.to_bytes());
    value.as_object_mut().unwrap().insert(
        "signature".to_string(),
        serde_json::Value::String(sig_b64.clone()),
    );
    let mut signed = license;
    signed.signature = sig_b64;
    let json = serde_json::to_vec(&value).expect("encode");
    MintedLicense {
        bytes: json,
        did: test_did(),
        license: signed,
    }
}

pub fn test_keys() -> Vec<(&'static str, String)> {
    vec![(TEST_KEY_ID, test_did())]
}

/// Convert owned key table into the slice-of-tuples shape
/// `parse_and_verify_with_keys` expects.
pub fn keys_as_refs<'a>(keys: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    keys.iter().map(|(k, v)| (*k, v.as_str())).collect()
}

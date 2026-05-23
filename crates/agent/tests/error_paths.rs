//! Error-path coverage for the `agent` crate. Asserts that malformed inputs
//! flow through the public API as typed [`AgentError`] variants instead of
//! panicking — guards against regressions of the `unwrap/expect` hot-path
//! audit (branch `feat/quality-agent-unwrap-fix`).

use agent::{AgentCard, AgentError, AgentKind, Envelope, Identity};

#[test]
fn bad_key_bytes_returns_err() {
    // Wrong length: 8 bytes of hex instead of 32 → `Key` variant.
    // NB: `Identity` deliberately does not derive `Debug` (it wraps secret key
    // material), so we collapse the `Ok` arm via `.err()` before unwrapping —
    // `Option::expect` only needs `AgentError: Debug`, not `Identity: Debug`.
    let short = "deadbeef";
    let err = Identity::from_hex_seed(short)
        .err()
        .expect("short seed must fail");
    assert!(matches!(err, AgentError::Key(_)), "got {err:?}");

    // Garbage hex (odd length, non-hex chars) is also reported as Key.
    let garbage = "zzzz-not-hex";
    let err = Identity::from_hex_seed(garbage)
        .err()
        .expect("garbage hex must fail");
    assert!(matches!(err, AgentError::Key(_)), "got {err:?}");
}

#[test]
fn bad_signature_returns_signature_invalid() {
    // Build a real signed envelope, then flip the body so the embedded sig no
    // longer covers the canonical bytes → `SignatureInvalid`.
    let alice = Identity::generate();
    let mut env = Envelope::create(
        &alice,
        "did:pharma:peer",
        "m",
        "ping",
        serde_json::json!({ "ok": true }),
    )
    .expect("envelope build");
    env.body = serde_json::json!({ "ok": false });
    let err = env.verify().expect_err("tampered body must fail");
    assert!(matches!(err, AgentError::SignatureInvalid), "got {err:?}");

    // Same flow for an AgentCard: tamper an attribute after signing.
    let mut card = AgentCard::new(&alice, "Lab", AgentKind::Lab, "CL-RM", "http://lab.example")
        .expect("card build");
    card.endpoint = "http://evil.example".into();
    let err = card.verify().expect_err("tampered card must fail");
    assert!(matches!(err, AgentError::SignatureInvalid), "got {err:?}");
}

#[test]
fn bad_base64_returns_err() {
    // Build a valid envelope then replace `sig` (which is hex, not base64,
    // but the variant is named after the canonical "byte-string decode"
    // failure mode) with garbage that can't be hex-decoded → `Base64`.
    let alice = Identity::generate();
    let mut env = Envelope::create(
        &alice,
        "did:pharma:peer",
        "m",
        "ping",
        serde_json::json!({}),
    )
    .expect("envelope build");
    env.sig = "ZZZZ-not-a-valid-hex-string".into();
    let err = env.verify().expect_err("garbage sig must fail");
    assert!(matches!(err, AgentError::Base64(_)), "got {err:?}");

    // Card uses the same path.
    let mut card =
        AgentCard::new(&alice, "Pharma", AgentKind::Pharmacy, "CL-CO", "").expect("card build");
    card.sig = "not%%hex".into();
    let err = card.verify().expect_err("garbage card sig must fail");
    assert!(matches!(err, AgentError::Base64(_)), "got {err:?}");
}

#[test]
fn bad_envelope_json_returns_err() {
    // Malformed JSON.
    let err = Envelope::from_json("{not json").expect_err("malformed json must fail");
    assert!(matches!(err, AgentError::Envelope(_)), "got {err:?}");

    // Well-formed JSON but missing required fields (wrong shape).
    let err = Envelope::from_json(r#"{"hello":"world"}"#).expect_err("wrong-shape json must fail");
    assert!(matches!(err, AgentError::Envelope(_)), "got {err:?}");
}

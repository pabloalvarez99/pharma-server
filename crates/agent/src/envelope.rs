//! Signed message envelope — the wire unit between agents.
//!
//! ```json
//! {
//!   "from": "did:pharma:<pk_a>",
//!   "to":   "did:pharma:<pk_b>",
//!   "msg_id": "uuid-or-ulid",
//!   "ts": "RFC3339",
//!   "topic": "quote.request",
//!   "body": { ... topic-specific ... },
//!   "sig": "hex(ed25519 over canonical(envelope sans sig))"
//! }
//! ```
//!
//! Topics (MVP): `catalog.lookup` / `catalog.match`, `quote.request` /
//! `quote.response`, `po.create` / `po.ack` / `po.reject`,
//! `shipment.notify` / `shipment.ack`, `payment.confirm` / `payment.ack`.
//! The body schema per topic lives with the domain handlers; this crate only
//! guarantees authenticity + integrity of the wrapper.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::canonical_bytes;
use crate::identity::{verify_with_did, Identity};
use crate::{AgentError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub from: String,
    pub to: String,
    pub msg_id: String,
    pub ts: String,
    pub topic: String,
    pub body: Value,
    #[serde(default)]
    pub sig: String,
}

impl Envelope {
    /// Build + sign an envelope from `id` to `to_did` on `topic`.
    pub fn create(
        id: &Identity,
        to_did: impl Into<String>,
        msg_id: impl Into<String>,
        topic: impl Into<String>,
        body: Value,
    ) -> Self {
        let mut env = Self {
            from: id.did(),
            to: to_did.into(),
            msg_id: msg_id.into(),
            ts: chrono::Utc::now().to_rfc3339(),
            topic: topic.into(),
            body,
            sig: String::new(),
        };
        let bytes = canonical_bytes(&env.unsigned_value());
        env.sig = hex::encode(id.sign(&bytes).to_bytes());
        env
    }

    fn unsigned_value(&self) -> Value {
        serde_json::json!({
            "from": self.from,
            "to": self.to,
            "msg_id": self.msg_id,
            "ts": self.ts,
            "topic": self.topic,
            "body": self.body,
        })
    }

    /// Verify `sig` against the key embedded in `from`.
    pub fn verify(&self) -> Result<()> {
        let sig_bytes = hex::decode(&self.sig)
            .map_err(|e| AgentError::Key(format!("sig hex inválido: {e}")))?;
        let arr: [u8; 64] = sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| AgentError::BadSignature)?;
        let sig = ed25519_dalek::Signature::from_bytes(&arr);
        verify_with_did(&self.from, &canonical_bytes(&self.unsigned_value()), &sig)
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn signed_envelope_verifies() {
        let alice = Identity::generate();
        let bob = Identity::generate();
        let env = Envelope::create(
            &alice,
            bob.did(),
            "msg-1",
            "quote.request",
            json!({ "items": [{ "barcode": "7800001", "qty": 10 }] }),
        );
        env.verify().expect("valid");
    }

    #[test]
    fn tampered_body_fails() {
        let alice = Identity::generate();
        let mut env = Envelope::create(
            &alice,
            "did:pharma:x",
            "m",
            "po.create",
            json!({ "total": 1000 }),
        );
        env.body = json!({ "total": 999999 });
        assert!(env.verify().is_err());
    }

    #[test]
    fn forged_from_fails() {
        // Sign as alice but claim to be bob.
        let alice = Identity::generate();
        let bob = Identity::generate();
        let mut env = Envelope::create(&alice, "did:pharma:x", "m", "ping", json!({}));
        env.from = bob.did();
        assert!(env.verify().is_err());
    }

    #[test]
    fn json_roundtrip() {
        let id = Identity::generate();
        let env = Envelope::create(&id, "did:pharma:y", "m2", "ping", json!({"n":1}));
        let back = Envelope::from_json(&env.to_json().unwrap()).unwrap();
        back.verify().expect("survives roundtrip");
    }
}

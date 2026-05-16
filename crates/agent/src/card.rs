//! Self-signed public node metadata for discovery.
//!
//! An `AgentCard` is what a node hands to peers (out-of-band: email, QR, or a
//! future relay) so they can address it and verify its messages. It is signed
//! by the node's own key over the canonical encoding of every field except
//! `sig`, so tampering (e.g. swapping the endpoint) invalidates it.

use serde::{Deserialize, Serialize};

use crate::canonical::canonical_bytes;
use crate::identity::{verify_with_did, Identity};
use crate::{AgentError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    Pharmacy,
    Supplier,
    Distributor,
    Lab,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub did: String,
    pub name: String,
    pub kind: AgentKind,
    /// ISO-3166 region hint (e.g. "CL-CO" for Coquimbo).
    pub region: String,
    /// Reachable base URL for direct HTTP push (may be LAN-only or empty if
    /// the node is relay-only).
    pub endpoint: String,
    pub created_at: String,
    /// Hex Ed25519 signature over the canonical card sans `sig`.
    pub sig: String,
}

impl AgentCard {
    pub fn new(
        id: &Identity,
        name: impl Into<String>,
        kind: AgentKind,
        region: impl Into<String>,
        endpoint: impl Into<String>,
    ) -> Self {
        let mut card = Self {
            did: id.did(),
            name: name.into(),
            kind,
            region: region.into(),
            endpoint: endpoint.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            sig: String::new(),
        };
        let bytes = canonical_bytes(&card.unsigned_value());
        card.sig = hex::encode(id.sign(&bytes).to_bytes());
        card
    }

    fn unsigned_value(&self) -> serde_json::Value {
        serde_json::json!({
            "did": self.did,
            "name": self.name,
            "kind": self.kind,
            "region": self.region,
            "endpoint": self.endpoint,
            "created_at": self.created_at,
        })
    }

    /// Verify the card's self-signature against the key embedded in its DID.
    pub fn verify(&self) -> Result<()> {
        let sig_bytes = hex::decode(&self.sig)
            .map_err(|e| AgentError::Key(format!("sig hex inválido: {e}")))?;
        let arr: [u8; 64] = sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| AgentError::BadSignature)?;
        let sig = ed25519_dalek::Signature::from_bytes(&arr);
        verify_with_did(&self.did, &canonical_bytes(&self.unsigned_value()), &sig)
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn card_self_signature_verifies() {
        let id = Identity::generate();
        let card = AgentCard::new(
            &id,
            "Farmacia Coquimbo",
            AgentKind::Pharmacy,
            "CL-CO",
            "http://192.168.1.10:8080",
        );
        card.verify().expect("valid self-sig");
    }

    #[test]
    fn tampered_endpoint_fails_verification() {
        let id = Identity::generate();
        let mut card = AgentCard::new(
            &id,
            "Farmacia",
            AgentKind::Pharmacy,
            "CL-CO",
            "http://192.168.1.10:8080",
        );
        card.endpoint = "http://evil.example".into();
        assert!(card.verify().is_err());
    }

    #[test]
    fn json_roundtrip_preserves_signature() {
        let id = Identity::generate();
        let card = AgentCard::new(&id, "Lab X", AgentKind::Lab, "CL-RM", "");
        let j = card.to_json().unwrap();
        let back = AgentCard::from_json(&j).unwrap();
        back.verify().expect("sig survives roundtrip");
    }
}

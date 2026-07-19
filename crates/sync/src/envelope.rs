//! [`SyncEnvelope`] — a stock-delta wrapped in the reused Ed25519 signed
//! `agent::Envelope`. The sync crate invents NO new wire format: a stock sync
//! message is just an `agent::Envelope` on the `stock.delta` topic, so it
//! inherits the existing canonicalization + signature verification (ADR-0013
//! HMAC webhook is a *different*, complementary transport — this durable layer
//! signs with the node identity so a federated peer can verify provenance).

use agent::{Envelope, Identity};

use crate::model::{StockDelta, TOPIC_STOCK_DELTA};
use crate::{Result, SyncError};

/// A signed stock-delta envelope queued for push to a federated peer.
#[derive(Debug, Clone)]
pub struct SyncEnvelope {
    inner: Envelope,
}

impl SyncEnvelope {
    /// Build + sign a stock-delta envelope from this node (`id`) to `peer_did`.
    /// `msg_id` is the idempotency key (use a UUID v7 for monotonic ordering).
    pub fn create(
        id: &Identity,
        peer_did: impl Into<String>,
        msg_id: impl Into<String>,
        delta: StockDelta,
    ) -> Result<Self> {
        let body = serde_json::to_value(&delta)?;
        let inner = Envelope::create(id, peer_did, msg_id, TOPIC_STOCK_DELTA, body)?;
        Ok(Self { inner })
    }

    /// Wrap an already-built envelope (e.g. re-read from the outbox), rejecting
    /// anything that is not on the `stock.delta` topic.
    pub fn from_envelope(inner: Envelope) -> Result<Self> {
        if inner.topic != TOPIC_STOCK_DELTA {
            return Err(SyncError::transport_permanent(format!(
                "topic inesperado: {}",
                inner.topic
            )));
        }
        Ok(Self { inner })
    }

    /// Parse a queued envelope JSON and verify its signature in one step. Used
    /// on the drain path: a tampered queued payload is rejected before it can
    /// be delivered.
    pub fn parse_verified(json: &str) -> Result<Self> {
        let inner = Envelope::from_json(json)?;
        inner.verify().map_err(|_| SyncError::TamperedEnvelope)?;
        Self::from_envelope(inner)
    }

    /// Verify the Ed25519 signature against the key embedded in `from`.
    pub fn verify(&self) -> Result<()> {
        self.inner.verify().map_err(|_| SyncError::TamperedEnvelope)
    }

    /// Idempotency key (the envelope `msg_id`).
    pub fn msg_id(&self) -> &str {
        &self.inner.msg_id
    }

    /// Sender DID (`did:pharma:<pk>`).
    pub fn from_did(&self) -> &str {
        &self.inner.from
    }

    /// Recipient DID.
    pub fn to_did(&self) -> &str {
        &self.inner.to
    }

    /// Decode the typed stock-delta body.
    pub fn delta(&self) -> Result<StockDelta> {
        Ok(serde_json::from_value(self.inner.body.clone())?)
    }

    /// Canonical signed JSON for persistence / transport.
    pub fn to_json(&self) -> Result<String> {
        Ok(self.inner.to_json()?)
    }

    /// Borrow the underlying signed envelope.
    pub fn envelope(&self) -> &Envelope {
        &self.inner
    }
}

//! Transport traits — the seam between the durable outbox and the wire. The
//! real HTTP/NATS implementation lives in the api/agent layer (and the peer
//! receiver in the OTHER repo); tests use an in-memory mock peer. Keeping the
//! traits here lets the outbox drain worker stay transport-agnostic and fully
//! unit-testable offline.

use async_trait::async_trait;

use crate::envelope::SyncEnvelope;
use crate::Result;

/// Push side: deliver a signed envelope to a federated peer.
///
/// Implementations MUST be idempotent-friendly — the outbox may re-deliver a
/// message whose ack was lost, and the peer de-dupes on the envelope `msg_id`.
/// Return [`crate::SyncError::transport_retryable`] for transient failures
/// (peer offline, 5xx, timeout) so the row stays pending, and
/// [`crate::SyncError::transport_permanent`] for contract errors (bad
/// signature, 4xx) so it fails fast instead of hammering the peer.
#[async_trait]
pub trait PushTransport: Send + Sync {
    async fn push(&self, env: &SyncEnvelope) -> Result<()>;
}

/// Pull side: fetch everything a peer has accepted from this node, in delivery
/// order. Used by the reconcile path and the push→pull roundtrip property test.
#[async_trait]
pub trait PullTransport: Send + Sync {
    async fn pull(&self, from_did: &str) -> Result<Vec<SyncEnvelope>>;
}

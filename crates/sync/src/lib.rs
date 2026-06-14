//! `sync` — durable stock-sync outbox (Fase 12, ADR-0013 Patrón B).
//!
//! The offline-first complement to the in-memory stock webhook
//! (`crates/api/src/stock_webhook.rs`): a bounded, on-disk, tenant-scoped queue
//! of Ed25519-signed stock-delta envelopes that **accumulates while a federated
//! peer is offline and drains, in order, when it returns** — the behaviour the
//! ADR-0013 offline-first goal calls for, which the best-effort webhook (drop +
//! nightly reconcile) does not provide.
//!
//! It invents no new wire format: a sync message is an [`agent::Envelope`] on
//! the `stock.delta` topic (reusing the existing canonicalization + signature),
//! so a peer verifies provenance with the same trust machinery as the agent
//! federation layer.
//!
//! ## Layout
//! - [`model`] — [`StockDelta`] body, [`OutboxRow`], [`OutboxStatus`].
//! - [`envelope`] — [`SyncEnvelope`] (sign / parse-verified).
//! - [`transport`] — [`PushTransport`] / [`PullTransport`] seams (mock in tests).
//! - [`outbox`] — [`enqueue`] + [`drain`] with per-tenant serialization, bounded
//!   retry, and head-of-line order preservation.

pub mod envelope;
pub mod error;
pub mod model;
pub mod outbox;
pub mod transport;

pub use envelope::SyncEnvelope;
pub use error::{Result, SyncError};
pub use model::{OutboxRow, OutboxStatus, StockDelta, STOCK_DELTA_SCHEMA, TOPIC_STOCK_DELTA};
pub use outbox::{drain, enqueue, list, pending_depth, DrainReport, EnqueueOutcome, MAX_ATTEMPTS};
pub use transport::{PullTransport, PushTransport};

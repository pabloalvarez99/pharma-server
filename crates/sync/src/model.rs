//! Data model for the stock-sync outbox: the signed stock-delta envelope, the
//! persisted outbox row, and its delivery status.

use serde::{Deserialize, Serialize};

/// Topic stamped on every stock-delta sync envelope. The peer routes on this.
pub const TOPIC_STOCK_DELTA: &str = "stock.delta";

/// Schema version of the [`StockDelta`] body. Bumps require a sub-ADR
/// (mirrors `crates/api/src/stock_webhook.rs` discipline).
pub const STOCK_DELTA_SCHEMA: &str = "1.0";

/// Body of a stock-delta sync envelope. The ERP is canonical for stock
/// (ADR-0013 truth matrix); the peer keys on `external_id`.
///
/// `new_stock` is the absolute post-movement count, never the delta alone —
/// deltas are not idempotent under retry, so the peer applies the absolute and
/// uses `ts` to discard out-of-order events. `delta` is carried for audit /
/// ordering only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockDelta {
    /// Pinned schema version (`"1.0"`).
    pub schema_version: String,
    /// Commercial SKU (`product.external_id`) — the peer's join key.
    pub external_id: String,
    /// Signed change that triggered this event (audit / ordering only).
    pub delta: i64,
    /// Stock **after** the movement. Idempotent under retry.
    pub new_stock: i64,
    /// `new_stock > 0` convenience boolean for the peer.
    pub in_stock: bool,
    /// RFC3339 timestamp of the movement.
    pub ts: String,
}

impl StockDelta {
    /// Build a stock-delta body for one product's post-movement stock.
    pub fn new(
        external_id: impl Into<String>,
        delta: i64,
        new_stock: i64,
        ts: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: STOCK_DELTA_SCHEMA.to_string(),
            external_id: external_id.into(),
            delta,
            new_stock,
            in_stock: new_stock > 0,
            ts: ts.into(),
        }
    }
}

/// Delivery state of an outbox row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutboxStatus {
    /// Queued, not yet delivered (or awaiting the next retry).
    Pending,
    /// Delivered and acknowledged by the peer.
    Sent,
    /// Permanently undeliverable (retries exhausted or contract error).
    Failed,
}

impl OutboxStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutboxStatus::Pending => "pending",
            OutboxStatus::Sent => "sent",
            OutboxStatus::Failed => "failed",
        }
    }
}

/// A persisted outbox row as read back from SurrealDB. `tenant` is the record
/// link string (`tenant:<id>`); `envelope` is the canonical signed-envelope
/// JSON re-parsed on drain so a tampered queued payload is rejected before
/// delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxRow {
    pub id: String,
    pub peer_did: String,
    /// Idempotency key = the envelope `msg_id`. Unique per tenant.
    pub msg_id: String,
    /// Signed-envelope JSON (verbatim, re-verified on drain).
    pub envelope: String,
    pub status: OutboxStatus,
    pub attempts: i64,
    pub created_at: String,
    pub next_attempt_at: String,
    #[serde(default)]
    pub last_error: Option<String>,
}

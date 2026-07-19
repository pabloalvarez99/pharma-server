//! Durable outbox: enqueue signed stock-delta envelopes and drain them to a
//! peer with bounded retry, preserving per-tenant order.
//!
//! ## Concurrency (mirrors the BUG-003/004 fix, PR #158)
//!
//! Drains are serialized per tenant with an [`AsyncMutex`] keyed on the tenant
//! record — exactly the proven approach in
//! `crates/domain/src/sales/service.rs::SALE_LOCKS` and `caf.rs::ASSIGN_LOCK`.
//! Two concurrent drains of the same tenant can otherwise both read a row as
//! `pending` and double-deliver; the lock removes that race. Different tenants
//! never share a lock, so multi-tenant throughput is unaffected. Each DB write
//! is additionally wrapped in a retry-on-conflict loop for SurrealKv's
//! write-write MVCC aborts (e.g. a concurrent enqueue).
//!
//! ## Order preservation
//!
//! Pending rows drain oldest-first. A *retryable* delivery failure stops the
//! drain for that tenant at the head of the line — message N+1 is never
//! delivered before N — so the peer sees stock deltas in commit order. A
//! *permanent* failure (tampered envelope / contract error) is marked `failed`
//! and skipped, since it can never be delivered and must not wedge the queue
//! forever.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use surrealdb::sql::Thing;
use tokio::sync::Mutex as AsyncMutex;

use crate::envelope::SyncEnvelope;
use crate::model::{OutboxRow, OutboxStatus};
use crate::{Result, SyncError};

type Db = surrealdb::Surreal<surrealdb::engine::local::Db>;

/// Attempt + 3 retries before a row is marked permanently `failed` — matches
/// the ADR-0013 webhook retry budget.
pub const MAX_ATTEMPTS: i64 = 4;

/// Max DB-write retries on a retryable SurrealKv MVCC conflict.
const MAX_COMMIT_RETRIES: usize = 64;

/// Backoff before the n-th retry (1-indexed by `attempts` already made):
/// `[1, 5, 30]` s, then clamped. Mirrors `stock_webhook.rs`.
fn backoff_after(attempts: i64) -> Duration {
    let secs = match attempts {
        0 => 0,
        1 => 1,
        2 => 5,
        _ => 30,
    };
    Duration::seconds(secs)
}

// --- per-tenant drain serialization (BUG-003/004 pattern) -------------------

static DRAIN_LOCKS: OnceLock<StdMutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();

fn tenant_drain_lock(tenant: &Thing) -> Arc<AsyncMutex<()>> {
    let locks = DRAIN_LOCKS.get_or_init(|| StdMutex::new(HashMap::new()));
    let mut guard = locks.lock().expect("DRAIN_LOCKS mutex poisoned");
    guard
        .entry(tenant.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

fn is_retryable_conflict(e: &SyncError) -> bool {
    if let SyncError::Db(inner) = e {
        let s = inner.to_string();
        s.contains("read or write conflict") || s.contains("failed transaction")
    } else {
        false
    }
}

// --- DB row shape (internal; `id` as Thing for UPDATE binding) --------------

#[derive(Debug, Deserialize)]
struct Stored {
    id: Thing,
    peer_did: String,
    msg_id: String,
    envelope: String,
    status: OutboxStatus,
    attempts: i64,
    created_at: String,
    next_attempt_at: String,
    last_error: Option<String>,
}

impl Stored {
    fn into_public(self) -> OutboxRow {
        OutboxRow {
            id: self.id.to_string(),
            peer_did: self.peer_did,
            msg_id: self.msg_id,
            envelope: self.envelope,
            status: self.status,
            attempts: self.attempts,
            created_at: self.created_at,
            next_attempt_at: self.next_attempt_at,
            last_error: self.last_error,
        }
    }
}

// --- public API -------------------------------------------------------------

/// Outcome of an [`enqueue`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnqueueOutcome {
    /// A new row was created (record id).
    Inserted(String),
    /// An idempotent duplicate — a row with this `(tenant, msg_id)` already
    /// exists (record id). No second row is written.
    Duplicate(String),
}

impl EnqueueOutcome {
    pub fn id(&self) -> &str {
        match self {
            EnqueueOutcome::Inserted(id) | EnqueueOutcome::Duplicate(id) => id,
        }
    }
    pub fn is_duplicate(&self) -> bool {
        matches!(self, EnqueueOutcome::Duplicate(_))
    }
}

/// Result of a [`drain`] pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DrainReport {
    /// Rows delivered + acked this pass.
    pub sent: usize,
    /// Rows marked permanently `failed` this pass.
    pub failed: usize,
    /// Rows still `pending` after this pass (blocked head-of-line or not due).
    pub pending_remaining: usize,
}

/// Enqueue a signed envelope for `tenant`, addressed to `env.to_did()`.
/// Idempotent on `(tenant, msg_id)`: a re-enqueue of the same envelope returns
/// [`EnqueueOutcome::Duplicate`] without writing a second row.
pub async fn enqueue(
    db: &Db,
    tenant: &Thing,
    env: &SyncEnvelope,
    now: DateTime<Utc>,
) -> Result<EnqueueOutcome> {
    let msg_id = env.msg_id().to_string();
    let peer_did = env.to_did().to_string();
    let envelope_json = env.to_json()?;
    let ts = now.to_rfc3339();

    if let Some(existing) = find_by_msg(db, tenant, &msg_id).await? {
        return Ok(EnqueueOutcome::Duplicate(existing.id.to_string()));
    }

    let created: Vec<Stored> = db
        .query(
            "CREATE sync_outbox SET tenant = $tenant, peer_did = $peer_did, \
             msg_id = $msg_id, envelope = $envelope, status = 'pending', \
             attempts = 0, created_at = $ts, next_attempt_at = $ts, \
             last_error = NONE",
        )
        .bind(("tenant", tenant.clone()))
        .bind(("peer_did", peer_did))
        .bind(("msg_id", msg_id))
        .bind(("envelope", envelope_json))
        .bind(("ts", ts))
        .await?
        .take(0)?;

    let row = created
        .into_iter()
        .next()
        .ok_or_else(|| SyncError::transport_permanent("CREATE sync_outbox no devolvió fila"))?;
    Ok(EnqueueOutcome::Inserted(row.id.to_string()))
}

/// Drain `tenant`'s pending outbox to `transport`, oldest-first, stopping at the
/// first retryable failure to preserve order. `now` gates which rows are due
/// (a previously-failed row waits out its backoff).
pub async fn drain<T: crate::PushTransport + ?Sized>(
    db: &Db,
    tenant: &Thing,
    transport: &T,
    now: DateTime<Utc>,
) -> Result<DrainReport> {
    let lock = tenant_drain_lock(tenant);
    let _guard = lock.lock().await;

    let mut report = DrainReport::default();
    let due = due_pending(db, tenant, now).await?;

    for row in due {
        // Re-verify the queued envelope before trusting it on the wire. A
        // tampered row is permanently poisoned → fail + skip, never deliver.
        let env = match SyncEnvelope::parse_verified(&row.envelope) {
            Ok(env) => env,
            Err(_) => {
                mark_failed(db, &row.id, row.attempts, "envelope adulterado o inválido").await?;
                report.failed += 1;
                continue;
            }
        };

        match transport.push(&env).await {
            Ok(()) => {
                mark_sent(db, &row.id, row.attempts).await?;
                report.sent += 1;
            }
            Err(SyncError::Transport {
                retryable: false,
                message,
            }) => {
                mark_failed(db, &row.id, row.attempts, &message).await?;
                report.failed += 1;
            }
            Err(e) => {
                // Retryable (peer offline / 5xx / timeout) — bump attempts and
                // either fail (budget spent) or schedule the next retry, then
                // STOP: this row blocks the head of the line so the peer never
                // sees a later delta before this one.
                let attempts = row.attempts + 1;
                let msg = e.to_string();
                if attempts >= MAX_ATTEMPTS {
                    mark_failed(db, &row.id, attempts, &msg).await?;
                    report.failed += 1;
                } else {
                    let next = now + backoff_after(attempts);
                    reschedule(db, &row.id, attempts, next, &msg).await?;
                    report.pending_remaining = pending_depth(db, tenant).await?;
                    return Ok(report);
                }
            }
        }
    }

    report.pending_remaining = pending_depth(db, tenant).await?;
    Ok(report)
}

/// Count of rows still `pending` for a tenant (any due time).
pub async fn pending_depth(db: &Db, tenant: &Thing) -> Result<usize> {
    let n: Option<i64> = db
        .query("SELECT count() FROM sync_outbox WHERE tenant = $t AND status = 'pending' GROUP ALL")
        .bind(("t", tenant.clone()))
        .await?
        .take((0, "count"))?;
    Ok(n.unwrap_or(0).max(0) as usize)
}

/// All outbox rows for a tenant, newest-first (status / audit view).
pub async fn list(db: &Db, tenant: &Thing, limit: usize) -> Result<Vec<OutboxRow>> {
    let rows: Vec<Stored> = db
        .query("SELECT * FROM sync_outbox WHERE tenant = $t ORDER BY created_at DESC LIMIT $lim")
        .bind(("t", tenant.clone()))
        .bind(("lim", limit as i64))
        .await?
        .take(0)?;
    Ok(rows.into_iter().map(Stored::into_public).collect())
}

// --- internals --------------------------------------------------------------

async fn find_by_msg(db: &Db, tenant: &Thing, msg_id: &str) -> Result<Option<Stored>> {
    let row: Option<Stored> = db
        .query("SELECT * FROM sync_outbox WHERE tenant = $t AND msg_id = $m LIMIT 1")
        .bind(("t", tenant.clone()))
        .bind(("m", msg_id.to_string()))
        .await?
        .take(0)?;
    Ok(row)
}

async fn due_pending(db: &Db, tenant: &Thing, now: DateTime<Utc>) -> Result<Vec<Stored>> {
    let rows: Vec<Stored> = db
        .query(
            "SELECT * FROM sync_outbox WHERE tenant = $t AND status = 'pending' \
             AND next_attempt_at <= $now ORDER BY created_at ASC",
        )
        .bind(("t", tenant.clone()))
        .bind(("now", now.to_rfc3339()))
        .await?
        .take(0)?;
    Ok(rows)
}

async fn mark_sent(db: &Db, id: &Thing, attempts: i64) -> Result<()> {
    with_conflict_retry(|| async {
        db.query("UPDATE $id SET status = 'sent', attempts = $a, last_error = NONE")
            .bind(("id", id.clone()))
            .bind(("a", attempts))
            .await?
            .check()?;
        Ok(())
    })
    .await
}

async fn mark_failed(db: &Db, id: &Thing, attempts: i64, err: &str) -> Result<()> {
    let err = err.to_string();
    with_conflict_retry(|| async {
        db.query("UPDATE $id SET status = 'failed', attempts = $a, last_error = $e")
            .bind(("id", id.clone()))
            .bind(("a", attempts))
            .bind(("e", err.clone()))
            .await?
            .check()?;
        Ok(())
    })
    .await
}

async fn reschedule(
    db: &Db,
    id: &Thing,
    attempts: i64,
    next: DateTime<Utc>,
    err: &str,
) -> Result<()> {
    let err = err.to_string();
    let next = next.to_rfc3339();
    with_conflict_retry(|| async {
        db.query(
            "UPDATE $id SET status = 'pending', attempts = $a, \
             next_attempt_at = $n, last_error = $e",
        )
        .bind(("id", id.clone()))
        .bind(("a", attempts))
        .bind(("n", next.clone()))
        .bind(("e", err.clone()))
        .await?
        .check()?;
        Ok(())
    })
    .await
}

/// Retry a DB write on SurrealKv's retryable write-write MVCC conflict.
async fn with_conflict_retry<F, Fut>(mut op: F) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let mut attempt = 0usize;
    loop {
        match op().await {
            Ok(()) => return Ok(()),
            Err(e) if is_retryable_conflict(&e) && attempt < MAX_COMMIT_RETRIES => {
                let micros = std::cmp::min((attempt as u64 + 1) * 50, 5_000);
                tokio::time::sleep(std::time::Duration::from_micros(micros)).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

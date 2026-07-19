//! Store-and-forward relay for federated envelopes (BACKLOG #7).
//!
//! A node often needs to send a signed [`Envelope`] to a federated peer that is
//! temporarily offline. Instead of dropping the message or blocking, the
//! originating node *enqueues* it on the local `agent_relay` table (migration
//! `0025`) and a later *drain* pass delivers it when the peer is reachable.
//!
//! Design:
//! * **Persistence** is the tenant-scoped `agent_relay` table. A row is a
//!   queued envelope (canonical JSON), its target peer DID, a `pending|sent|
//!   failed` status, an attempt counter and a `next_attempt_at` due time.
//! * **Transport** is abstracted by [`PeerTransport`] so this crate stays
//!   offline-pure and unit-testable: the real HTTP/NATS delivery is wired in
//!   `crates/api` (a future job hub), while tests use an in-memory mock.
//! * **Retry** is bounded exponential backoff. After [`MAX_ATTEMPTS`] failed
//!   deliveries the row becomes terminal `failed` and is no longer retried.
//! * **Integrity** — drain re-verifies the *stored* envelope's Ed25519
//!   signature before delivery, so a row tampered with at rest is rejected and
//!   never sent (it goes terminal `failed`, not delivered).
//! * **Idempotency** — `(tenant, msg_id)` is UNIQUE: re-enqueuing the same
//!   envelope does not duplicate it, and a `sent` row is never re-selected, so
//!   re-draining cannot double-deliver.
//! * **Backpressure** — a peer that stays offline must not let the queue grow
//!   without bound. [`enqueue`] refuses a *new* envelope with
//!   [`AgentError::QueueFull`] once a peer already has [`MAX_PENDING_PER_PEER`]
//!   pending rows. Idempotent re-enqueues of an already-queued envelope are
//!   never rejected (they short-circuit before the depth check).
//! * **Dead-letter queue** — a terminal `failed` row *is* the dead-letter
//!   entry. [`list_dead_letters`] inspects them and [`redrive`] / [`redrive_all`]
//!   reset a failed row back to `pending` (attempts cleared, due now) so an
//!   operator can replay deliveries after fixing the peer.
//! * **Observability** — `enqueue`/`drain`/`redrive` are `tracing`-instrumented
//!   spans; a terminal failure emits a `warn` dead-letter event and a drain
//!   pass emits its [`DrainReport`] tally.
//!
//! Federation is opt-in (`admin_setting federation_enabled`); the caller gates
//! it exactly as the inbound `/agent/inbox` handler does — never here.

use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use surrealdb::engine::local::Db as LocalDb;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

use crate::envelope::Envelope;
use crate::{AgentError, Result};

type Db = Surreal<LocalDb>;

/// Maximum delivery attempts before a queued envelope is terminal `failed`.
pub const MAX_ATTEMPTS: u32 = 8;
/// First retry waits this many seconds; each later attempt doubles it.
pub const BASE_BACKOFF_SECS: i64 = 2;
/// Backoff is capped here so a long-offline peer retries at a steady cadence.
pub const MAX_BACKOFF_SECS: i64 = 3600;
/// Backpressure cap: max `pending` rows a single peer may hold before
/// [`enqueue`] refuses new envelopes with [`AgentError::QueueFull`]. Bounds the
/// disk a permanently-offline peer can consume; terminal `sent`/`failed` rows
/// do not count toward it.
pub const MAX_PENDING_PER_PEER: usize = 10_000;

fn db_err<E: std::fmt::Display>(e: E) -> AgentError {
    AgentError::Db(e.to_string())
}

/// Bounded exponential backoff for the `n`-th failed attempt (1-based count of
/// attempts already made). `backoff(1)` = `BASE_BACKOFF_SECS`, doubling each
/// time, capped at `MAX_BACKOFF_SECS`.
fn backoff(attempts: u32) -> Duration {
    // attempts is 1.. on entry; shift by attempts-1 so the first retry == base.
    let shift = attempts.saturating_sub(1).min(20);
    let secs = BASE_BACKOFF_SECS
        .saturating_mul(1_i64 << shift)
        .min(MAX_BACKOFF_SECS);
    Duration::seconds(secs)
}

/// Lifecycle of a queued envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayStatus {
    /// Awaiting delivery (or a backoff window after a failed attempt).
    Pending,
    /// Delivered and acknowledged by the peer. Terminal.
    Sent,
    /// Gave up after [`MAX_ATTEMPTS`], or the stored envelope failed
    /// verification. Terminal.
    Failed,
}

impl RelayStatus {
    fn as_str(self) -> &'static str {
        match self {
            RelayStatus::Pending => "pending",
            RelayStatus::Sent => "sent",
            RelayStatus::Failed => "failed",
        }
    }
}

/// A queued relay row as visible to operators / status queries.
#[derive(Debug, Clone)]
pub struct RelayEntry {
    pub id: String,
    pub msg_id: String,
    pub target_did: String,
    pub status: String,
    pub attempts: u32,
    pub next_attempt_at: DateTime<Utc>,
    pub last_error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Row {
    id: Thing,
    msg_id: String,
    target_did: String,
    envelope_json: String,
    status: String,
    attempts: i64,
    next_attempt_at: DateTime<Utc>,
    last_error: Option<String>,
}

impl From<Row> for RelayEntry {
    fn from(r: Row) -> Self {
        Self {
            id: r.id.to_string(),
            msg_id: r.msg_id,
            target_did: r.target_did,
            status: r.status,
            attempts: r.attempts.max(0) as u32,
            next_attempt_at: r.next_attempt_at,
            last_error: r.last_error,
        }
    }
}

const SELECT_COLS: &str = "id, msg_id, target_did, envelope_json, status, \
     attempts, next_attempt_at, last_error";

/// Outbound delivery of a signed envelope to a peer DID. The real
/// implementation lives in the transport layer (HTTP push to the peer's
/// `/agent/inbox`); tests provide an in-memory mock. An `Err(String)` means the
/// peer was unreachable / rejected the message and the row should be retried.
pub trait PeerTransport {
    fn deliver(
        &self,
        peer_did: &str,
        envelope: &Envelope,
    ) -> impl std::future::Future<Output = std::result::Result<(), String>> + Send;
}

/// Tally of what a single [`drain`] pass did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DrainReport {
    /// Rows delivered + acked this pass.
    pub sent: usize,
    /// Rows whose delivery failed but remain `pending` for a later retry.
    pub retried: usize,
    /// Rows that became terminal `failed` (attempt cap hit or verify failed).
    pub failed: usize,
}

/// Enqueue `envelope` for delivery to `peer_did` on behalf of `tenant`.
///
/// The envelope is verified up front (a forged/tampered envelope is refused
/// with [`AgentError::SignatureInvalid`] and never stored). Idempotent on
/// `(tenant, msg_id)`: enqueuing the same envelope twice returns the existing
/// row instead of duplicating it.
///
/// Backpressure: a *new* envelope is refused with [`AgentError::QueueFull`]
/// once the peer holds [`MAX_PENDING_PER_PEER`] pending rows. Re-enqueuing an
/// already-queued envelope is never rejected (it returns early above the cap
/// check), so retry-on-timeout callers stay idempotent even at the cap.
#[tracing::instrument(skip(db, envelope), fields(msg_id = %envelope.msg_id, peer = peer_did), err)]
pub async fn enqueue(
    db: &Db,
    tenant: &Thing,
    envelope: &Envelope,
    peer_did: &str,
) -> Result<RelayEntry> {
    enqueue_capped(db, tenant, envelope, peer_did, MAX_PENDING_PER_PEER).await
}

/// [`enqueue`] with an explicit backpressure cap (the public entry point uses
/// [`MAX_PENDING_PER_PEER`]). Factored out so the cap is unit-testable without
/// materializing tens of thousands of rows.
async fn enqueue_capped(
    db: &Db,
    tenant: &Thing,
    envelope: &Envelope,
    peer_did: &str,
    cap: usize,
) -> Result<RelayEntry> {
    envelope.verify()?;

    // Idempotency: a prior copy of this envelope (same tenant+msg_id) wins.
    if let Some(existing) = get(db, tenant, &envelope.msg_id).await? {
        return Ok(existing);
    }

    // Backpressure: refuse new work once the peer's pending backlog is at cap.
    let pending = count(db, tenant, peer_did, RelayStatus::Pending).await?;
    if pending >= cap {
        tracing::warn!(pending, cap, "relay queue full; refusing enqueue");
        return Err(AgentError::QueueFull(cap));
    }

    let envelope_json = envelope.to_json()?;
    let mut r = db
        .query(format!(
            "CREATE agent_relay SET tenant = $t, msg_id = $m, target_did = $d, \
             envelope_json = $j, status = 'pending', attempts = 0, \
             next_attempt_at = time::now() RETURN {SELECT_COLS}"
        ))
        .bind(("t", tenant.clone()))
        .bind(("m", envelope.msg_id.clone()))
        .bind(("d", peer_did.to_string()))
        .bind(("j", envelope_json))
        .await
        .map_err(db_err)?
        .check()
        .map_err(db_err)?;
    let row: Option<Row> = r.take(0).map_err(db_err)?;
    // A race on the UNIQUE index can make CREATE return empty; fall back to the
    // row the winner inserted so enqueue stays idempotent under concurrency.
    match row {
        Some(row) => Ok(row.into()),
        None => get(db, tenant, &envelope.msg_id)
            .await?
            .ok_or_else(|| AgentError::Db("enqueue: row vanished after create".into())),
    }
}

/// Fetch the queued row for `(tenant, msg_id)`, if any.
pub async fn get(db: &Db, tenant: &Thing, msg_id: &str) -> Result<Option<RelayEntry>> {
    let mut r = db
        .query(format!(
            "SELECT {SELECT_COLS} FROM agent_relay \
             WHERE tenant = $t AND msg_id = $m LIMIT 1"
        ))
        .bind(("t", tenant.clone()))
        .bind(("m", msg_id.to_string()))
        .await
        .map_err(db_err)?
        .check()
        .map_err(db_err)?;
    let row: Option<Row> = r.take(0).map_err(db_err)?;
    Ok(row.map(Into::into))
}

/// Count rows in `status` for a peer (status query helper).
pub async fn count(db: &Db, tenant: &Thing, peer_did: &str, status: RelayStatus) -> Result<usize> {
    let mut r = db
        .query(
            "SELECT count() FROM agent_relay \
             WHERE tenant = $t AND target_did = $d AND status = $s GROUP ALL",
        )
        .bind(("t", tenant.clone()))
        .bind(("d", peer_did.to_string()))
        .bind(("s", status.as_str().to_string()))
        .await
        .map_err(db_err)?
        .check()
        .map_err(db_err)?;
    let n: Option<i64> = r.take((0, "count")).map_err(db_err)?;
    Ok(n.unwrap_or(0).max(0) as usize)
}

/// Deliver every due `pending` envelope for `peer_did`, draining the queue.
///
/// "Due" = `status = pending AND next_attempt_at <= now`. For each row the
/// stored envelope is re-verified (tamper at rest → terminal `failed`, never
/// delivered), then handed to `transport`:
/// * success → `sent` (terminal; never re-selected, so no double-delivery);
/// * failure → attempt counter bumped; reschedule with [`backoff`] while under
///   [`MAX_ATTEMPTS`], otherwise terminal `failed`.
///
/// `now` is injected so tests can advance time deterministically; callers pass
/// `Utc::now()`.
#[tracing::instrument(skip(db, transport, now), fields(peer = peer_did), err)]
pub async fn drain<T: PeerTransport>(
    db: &Db,
    tenant: &Thing,
    peer_did: &str,
    transport: &T,
    now: DateTime<Utc>,
) -> Result<DrainReport> {
    let mut q = db
        .query(format!(
            "SELECT {SELECT_COLS} FROM agent_relay \
             WHERE tenant = $t AND target_did = $d AND status = 'pending' \
             AND next_attempt_at <= $now ORDER BY next_attempt_at ASC"
        ))
        .bind(("t", tenant.clone()))
        .bind(("d", peer_did.to_string()))
        .bind(("now", surrealdb::sql::Datetime::from(now)))
        .await
        .map_err(db_err)?
        .check()
        .map_err(db_err)?;
    let rows: Vec<Row> = q.take(0).map_err(db_err)?;

    let mut report = DrainReport::default();
    for row in rows {
        // Re-verify the stored envelope: a row tampered at rest must never be
        // delivered. Parse failure or bad signature → terminal failed.
        let envelope = match Envelope::from_json(&row.envelope_json) {
            Ok(env) if env.verify().is_ok() => env,
            _ => {
                let reason = "envelope verify failed (tampered at rest)";
                mark_failed(db, &row.id, reason).await?;
                tracing::warn!(msg_id = %row.msg_id, reason, "relay dead-letter");
                report.failed += 1;
                continue;
            }
        };

        match transport.deliver(peer_did, &envelope).await {
            Ok(()) => {
                mark_sent(db, &row.id).await?;
                report.sent += 1;
            }
            Err(e) => {
                let attempts = (row.attempts.max(0) as u32).saturating_add(1);
                if attempts >= MAX_ATTEMPTS {
                    mark_failed(db, &row.id, &format!("max attempts ({MAX_ATTEMPTS}): {e}"))
                        .await?;
                    tracing::warn!(
                        msg_id = %row.msg_id, attempts,
                        reason = %e, "relay dead-letter (attempt cap)"
                    );
                    report.failed += 1;
                } else {
                    let next = now + backoff(attempts);
                    reschedule(db, &row.id, attempts, next, &e).await?;
                    report.retried += 1;
                }
            }
        }
    }
    tracing::debug!(
        sent = report.sent,
        retried = report.retried,
        failed = report.failed,
        "relay drain pass complete"
    );
    Ok(report)
}

async fn mark_sent(db: &Db, id: &Thing) -> Result<()> {
    db.query("UPDATE $id SET status = 'sent', last_error = NONE, updated_at = time::now()")
        .bind(("id", id.clone()))
        .await
        .map_err(db_err)?
        .check()
        .map_err(db_err)?;
    Ok(())
}

async fn mark_failed(db: &Db, id: &Thing, err: &str) -> Result<()> {
    db.query("UPDATE $id SET status = 'failed', last_error = $e, updated_at = time::now()")
        .bind(("id", id.clone()))
        .bind(("e", err.to_string()))
        .await
        .map_err(db_err)?
        .check()
        .map_err(db_err)?;
    Ok(())
}

async fn reschedule(
    db: &Db,
    id: &Thing,
    attempts: u32,
    next_attempt_at: DateTime<Utc>,
    err: &str,
) -> Result<()> {
    db.query(
        "UPDATE $id SET attempts = $a, next_attempt_at = $n, status = 'pending', \
         last_error = $e, updated_at = time::now()",
    )
    .bind(("id", id.clone()))
    .bind(("a", attempts as i64))
    .bind(("n", surrealdb::sql::Datetime::from(next_attempt_at)))
    .bind(("e", err.to_string()))
    .await
    .map_err(db_err)?
    .check()
    .map_err(db_err)?;
    Ok(())
}

/// List the dead-letter rows (terminal `status = failed`) for a peer, newest
/// first, capped at `limit`. These are envelopes that exhausted [`MAX_ATTEMPTS`]
/// or failed at-rest verification; an operator inspects them before deciding to
/// [`redrive`].
pub async fn list_dead_letters(
    db: &Db,
    tenant: &Thing,
    peer_did: &str,
    limit: usize,
) -> Result<Vec<RelayEntry>> {
    let mut r = db
        .query(format!(
            "SELECT {SELECT_COLS} FROM agent_relay \
             WHERE tenant = $t AND target_did = $d AND status = 'failed' \
             ORDER BY next_attempt_at DESC LIMIT $l"
        ))
        .bind(("t", tenant.clone()))
        .bind(("d", peer_did.to_string()))
        .bind(("l", limit as i64))
        .await
        .map_err(db_err)?
        .check()
        .map_err(db_err)?;
    let rows: Vec<Row> = r.take(0).map_err(db_err)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// Redrive a single dead-letter back onto the queue: reset a `failed` row to
/// `pending`, clear its attempt counter and error, and make it due now so the
/// next [`drain`] retries it. Returns `true` if a failed row matched (so a
/// `pending`/`sent`/missing row is a no-op returning `false`, never a
/// double-deliver).
#[tracing::instrument(skip(db), fields(msg_id = %msg_id), err)]
pub async fn redrive(db: &Db, tenant: &Thing, msg_id: &str) -> Result<bool> {
    let mut r = db
        .query(
            "UPDATE agent_relay SET status = 'pending', attempts = 0, \
             last_error = NONE, next_attempt_at = time::now(), \
             updated_at = time::now() \
             WHERE tenant = $t AND msg_id = $m AND status = 'failed' \
             RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("m", msg_id.to_string()))
        .await
        .map_err(db_err)?
        .check()
        .map_err(db_err)?;
    let updated: Vec<Row> = r.take(0).map_err(db_err)?;
    let n = updated.len();
    if n > 0 {
        tracing::info!(count = n, "relay redrive");
    }
    Ok(n > 0)
}

/// Redrive every dead-letter for a peer back onto the queue. Returns how many
/// `failed` rows were reset to `pending`.
#[tracing::instrument(skip(db), fields(peer = peer_did), err)]
pub async fn redrive_all(db: &Db, tenant: &Thing, peer_did: &str) -> Result<usize> {
    let mut r = db
        .query(
            "UPDATE agent_relay SET status = 'pending', attempts = 0, \
             last_error = NONE, next_attempt_at = time::now(), \
             updated_at = time::now() \
             WHERE tenant = $t AND target_did = $d AND status = 'failed' \
             RETURN AFTER",
        )
        .bind(("t", tenant.clone()))
        .bind(("d", peer_did.to_string()))
        .await
        .map_err(db_err)?
        .check()
        .map_err(db_err)?;
    let updated: Vec<Row> = r.take(0).map_err(db_err)?;
    let n = updated.len();
    tracing::info!(count = n, "relay redrive_all");
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use surrealdb::engine::local::Mem;

    /// Mock transport: counts deliver calls; optionally always fails (peer
    /// offline). Flip `online` to recover.
    #[derive(Clone)]
    struct MockTransport {
        calls: Arc<AtomicUsize>,
        online: Arc<std::sync::atomic::AtomicBool>,
    }

    impl MockTransport {
        fn new(online: bool) -> Self {
            Self {
                calls: Arc::new(AtomicUsize::new(0)),
                online: Arc::new(std::sync::atomic::AtomicBool::new(online)),
            }
        }
        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
        fn set_online(&self, v: bool) {
            self.online.store(v, Ordering::SeqCst);
        }
    }

    impl PeerTransport for MockTransport {
        async fn deliver(&self, _peer: &str, _env: &Envelope) -> std::result::Result<(), String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.online.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err("peer offline".into())
            }
        }
    }

    async fn setup() -> (Db, Thing) {
        let db = Surreal::new::<Mem>(()).await.expect("mem db");
        db.use_ns("test").use_db("test").await.unwrap();
        let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
        db::run_migrations(&db, migrations)
            .await
            .expect("migrations");
        let mut r = db
            .query("CREATE tenant SET name = 'Relay Test', slug = 'relay' RETURN id")
            .await
            .unwrap();
        let tenant: Option<Thing> = r.take((0, "id")).unwrap();
        (db, tenant.expect("tenant id"))
    }

    fn env(to: &str, msg_id: &str) -> Envelope {
        let id = Identity::generate();
        Envelope::create(&id, to.to_string(), msg_id, "po.create", json!({ "n": 1 }))
            .expect("create envelope")
    }

    #[tokio::test]
    async fn enqueue_then_drain_happy() {
        let (db, t) = setup().await;
        let peer = "did:pharma:peer";
        let e = env(peer, "m-1");
        enqueue(&db, &t, &e, peer).await.unwrap();

        let tx = MockTransport::new(true);
        let report = drain(&db, &t, peer, &tx, Utc::now()).await.unwrap();
        assert_eq!(report.sent, 1);
        assert_eq!(tx.calls(), 1);
        let row = get(&db, &t, "m-1").await.unwrap().unwrap();
        assert_eq!(row.status, "sent");
    }

    #[tokio::test]
    async fn peer_offline_stays_pending_then_drains_on_retry() {
        let (db, t) = setup().await;
        let peer = "did:pharma:peer";
        enqueue(&db, &t, &env(peer, "m-2"), peer).await.unwrap();

        // Peer down: delivery fails, row stays pending with a future due time.
        let tx = MockTransport::new(false);
        let now = Utc::now();
        let report = drain(&db, &t, peer, &tx, now).await.unwrap();
        assert_eq!(report.retried, 1);
        assert_eq!(report.sent, 0);
        let row = get(&db, &t, "m-2").await.unwrap().unwrap();
        assert_eq!(row.status, "pending");
        assert_eq!(row.attempts, 1);
        assert!(
            row.next_attempt_at > now,
            "backoff should push due time out"
        );

        // Not yet due → drain is a no-op (no extra deliver call).
        let report = drain(&db, &t, peer, &tx, now).await.unwrap();
        assert_eq!(report, DrainReport::default());
        assert_eq!(tx.calls(), 1);

        // Peer recovers + time advances past the backoff → delivered.
        tx.set_online(true);
        let later = row.next_attempt_at + Duration::seconds(1);
        let report = drain(&db, &t, peer, &tx, later).await.unwrap();
        assert_eq!(report.sent, 1);
        assert_eq!(get(&db, &t, "m-2").await.unwrap().unwrap().status, "sent");
    }

    #[tokio::test]
    async fn tampered_queued_envelope_rejected_on_drain() {
        let (db, t) = setup().await;
        let peer = "did:pharma:peer";
        let e = env(peer, "m-3");
        enqueue(&db, &t, &e, peer).await.unwrap();

        // Simulate at-rest tampering: mutate the stored envelope body so its
        // signature no longer matches.
        let mut tampered = e.clone();
        tampered.body = json!({ "n": 999999 });
        let tampered_json = serde_json::to_string(&tampered).unwrap();
        db.query("UPDATE agent_relay SET envelope_json = $j WHERE tenant = $t AND msg_id = 'm-3'")
            .bind(("j", tampered_json))
            .bind(("t", t.clone()))
            .await
            .unwrap()
            .check()
            .unwrap();

        let tx = MockTransport::new(true);
        let report = drain(&db, &t, peer, &tx, Utc::now()).await.unwrap();
        assert_eq!(report.failed, 1, "tampered row must be terminal failed");
        assert_eq!(tx.calls(), 0, "tampered envelope must never be delivered");
        assert_eq!(get(&db, &t, "m-3").await.unwrap().unwrap().status, "failed");
    }

    #[tokio::test]
    async fn idempotent_enqueue_and_redrain_no_double_deliver() {
        let (db, t) = setup().await;
        let peer = "did:pharma:peer";
        let e = env(peer, "m-4");
        let a = enqueue(&db, &t, &e, peer).await.unwrap();
        // Re-enqueue the same envelope: must return the same row, not a dup.
        let b = enqueue(&db, &t, &e, peer).await.unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(count(&db, &t, peer, RelayStatus::Pending).await.unwrap(), 1);

        let tx = MockTransport::new(true);
        assert_eq!(drain(&db, &t, peer, &tx, Utc::now()).await.unwrap().sent, 1);
        // Re-drain after a successful send: nothing re-selected, no re-deliver.
        let report = drain(&db, &t, peer, &tx, Utc::now()).await.unwrap();
        assert_eq!(report, DrainReport::default());
        assert_eq!(tx.calls(), 1, "sent row must never be re-delivered");
        assert_eq!(count(&db, &t, peer, RelayStatus::Sent).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn forged_envelope_refused_at_enqueue() {
        let (db, t) = setup().await;
        let peer = "did:pharma:peer";
        let mut e = env(peer, "m-5");
        e.body = json!({ "tampered": true }); // breaks the signature
        let err = enqueue(&db, &t, &e, peer).await;
        assert!(matches!(err, Err(AgentError::SignatureInvalid)));
        assert_eq!(count(&db, &t, peer, RelayStatus::Pending).await.unwrap(), 0);
    }

    /// Drive a queued row to terminal `failed` by draining against an offline
    /// peer, advancing `now` past each backoff window until the attempt cap is
    /// hit. Returns once the row's status is `failed`.
    async fn drive_to_failed(db: &Db, t: &Thing, peer: &str, msg_id: &str) {
        let tx = MockTransport::new(false);
        for _ in 0..(MAX_ATTEMPTS + 2) {
            let row = get(db, t, msg_id).await.unwrap().unwrap();
            if row.status == "failed" {
                return;
            }
            let now = row.next_attempt_at + Duration::seconds(1);
            drain(db, t, peer, &tx, now).await.unwrap();
        }
        panic!("row did not reach terminal failed");
    }

    #[tokio::test]
    async fn enqueue_backpressure_refuses_at_cap() {
        let (db, t) = setup().await;
        let peer = "did:pharma:peer";
        // Cap = 2: first two new envelopes queue, the third is refused.
        enqueue_capped(&db, &t, &env(peer, "b-1"), peer, 2)
            .await
            .unwrap();
        enqueue_capped(&db, &t, &env(peer, "b-2"), peer, 2)
            .await
            .unwrap();
        let full = enqueue_capped(&db, &t, &env(peer, "b-3"), peer, 2).await;
        assert!(matches!(full, Err(AgentError::QueueFull(2))));
        assert_eq!(count(&db, &t, peer, RelayStatus::Pending).await.unwrap(), 2);

        // Idempotent re-enqueue of an already-queued envelope is NOT refused,
        // even though the queue is at cap (it short-circuits before the check).
        let again = enqueue_capped(&db, &t, &env(peer, "b-1"), peer, 2).await;
        assert!(again.is_ok(), "re-enqueue at cap must stay idempotent");
    }

    #[tokio::test]
    async fn dead_letter_listed_and_redriven() {
        let (db, t) = setup().await;
        let peer = "did:pharma:peer";
        enqueue(&db, &t, &env(peer, "dl-1"), peer).await.unwrap();
        drive_to_failed(&db, &t, peer, "dl-1").await;

        // It now shows up in the dead-letter queue.
        let dead = list_dead_letters(&db, &t, peer, 10).await.unwrap();
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].msg_id, "dl-1");
        assert_eq!(dead[0].status, "failed");

        // Redrive resets it to pending (attempts cleared, due now).
        assert!(redrive(&db, &t, "dl-1").await.unwrap());
        let row = get(&db, &t, "dl-1").await.unwrap().unwrap();
        assert_eq!(row.status, "pending");
        assert_eq!(row.attempts, 0);
        assert!(list_dead_letters(&db, &t, peer, 10)
            .await
            .unwrap()
            .is_empty());

        // Peer is back: a drain now delivers the redriven envelope.
        let tx = MockTransport::new(true);
        let report = drain(&db, &t, peer, &tx, Utc::now()).await.unwrap();
        assert_eq!(report.sent, 1);
        assert_eq!(get(&db, &t, "dl-1").await.unwrap().unwrap().status, "sent");
    }

    #[tokio::test]
    async fn redrive_is_noop_on_non_failed_rows() {
        let (db, t) = setup().await;
        let peer = "did:pharma:peer";
        enqueue(&db, &t, &env(peer, "np-1"), peer).await.unwrap();

        // A pending row is not a dead-letter: redrive matches nothing.
        assert!(!redrive(&db, &t, "np-1").await.unwrap());
        assert!(!redrive(&db, &t, "does-not-exist").await.unwrap());
        // Still pending, untouched.
        assert_eq!(count(&db, &t, peer, RelayStatus::Pending).await.unwrap(), 1);

        // A sent row is terminal too: redrive must never resurrect it.
        let tx = MockTransport::new(true);
        drain(&db, &t, peer, &tx, Utc::now()).await.unwrap();
        assert_eq!(get(&db, &t, "np-1").await.unwrap().unwrap().status, "sent");
        assert!(!redrive(&db, &t, "np-1").await.unwrap());
        assert_eq!(get(&db, &t, "np-1").await.unwrap().unwrap().status, "sent");
    }

    #[tokio::test]
    async fn redrive_all_resets_every_dead_letter() {
        let (db, t) = setup().await;
        let peer = "did:pharma:peer";
        for id in ["ra-1", "ra-2", "ra-3"] {
            enqueue(&db, &t, &env(peer, id), peer).await.unwrap();
            drive_to_failed(&db, &t, peer, id).await;
        }
        assert_eq!(count(&db, &t, peer, RelayStatus::Failed).await.unwrap(), 3);

        let n = redrive_all(&db, &t, peer).await.unwrap();
        assert_eq!(n, 3);
        assert_eq!(count(&db, &t, peer, RelayStatus::Pending).await.unwrap(), 3);
        assert_eq!(count(&db, &t, peer, RelayStatus::Failed).await.unwrap(), 0);

        // Second redrive_all is a clean no-op (nothing failed remains).
        assert_eq!(redrive_all(&db, &t, peer).await.unwrap(), 0);
    }

    #[test]
    fn backoff_is_bounded_and_exponential() {
        assert_eq!(backoff(1), Duration::seconds(BASE_BACKOFF_SECS));
        assert_eq!(backoff(2), Duration::seconds(BASE_BACKOFF_SECS * 2));
        assert_eq!(backoff(3), Duration::seconds(BASE_BACKOFF_SECS * 4));
        assert_eq!(backoff(100), Duration::seconds(MAX_BACKOFF_SECS));
    }
}

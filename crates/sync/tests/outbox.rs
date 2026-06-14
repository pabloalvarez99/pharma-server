//! Integration tests for the durable stock-sync outbox on in-memory SurrealDB
//! (`kv-mem`), mirroring the harness in `crates/domain/tests/*.rs`.
//!
//! Coverage (≥10 cases): enqueue happy path, idempotent re-enqueue, cross-tenant
//! isolation, drain delivers + acks, peer-offline keeps rows pending and drains
//! on recovery, head-of-line order preservation, retry budget exhaustion,
//! tampered-envelope rejection, permanent contract-error fail, backoff gating,
//! and the push→pull roundtrip order property.

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

use agent::Identity;
use sync::{
    drain, enqueue, list, pending_depth, PullTransport, PushTransport, StockDelta, SyncEnvelope,
    SyncError,
};

type Db = Surreal<surrealdb::engine::local::Db>;

async fn setup() -> (Db, Thing, Thing) {
    let db = Surreal::new::<Mem>(()).await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations)
        .await
        .expect("migrations");
    let a = mk_tenant(&db, "farmacia-a").await;
    let b = mk_tenant(&db, "minimarket-b").await;
    (db, a, b)
}

async fn mk_tenant(db: &Db, slug: &str) -> Thing {
    let mut r = db
        .query("CREATE tenant SET name = $n, slug = $s RETURN id")
        .bind(("n", slug.to_string()))
        .bind(("s", slug.to_string()))
        .await
        .unwrap();
    let t: Option<Thing> = r.take((0, "id")).unwrap();
    t.expect("tenant id")
}

/// Build a signed stock-delta envelope from a fresh node identity to `peer`.
fn mk_env(id: &Identity, peer: &str, msg_id: &str, sku: &str, new_stock: i64) -> SyncEnvelope {
    let delta = StockDelta::new(sku, -1, new_stock, Utc::now().to_rfc3339());
    SyncEnvelope::create(id, peer, msg_id, delta).expect("sign envelope")
}

/// In-memory peer implementing both transports. `offline` makes `push` fail
/// with a retryable transport error; otherwise it records the delivered
/// envelopes in arrival order (idempotent on msg_id, like the real peer).
struct MockPeer {
    offline: Mutex<bool>,
    delivered: Mutex<Vec<SyncEnvelope>>,
    /// If set, push fails permanently (contract error) — never retried.
    permanent_fail: bool,
}

impl MockPeer {
    fn online() -> Self {
        Self {
            offline: Mutex::new(false),
            delivered: Mutex::new(Vec::new()),
            permanent_fail: false,
        }
    }
    fn offline() -> Self {
        Self {
            offline: Mutex::new(true),
            delivered: Mutex::new(Vec::new()),
            permanent_fail: false,
        }
    }
    fn permanent() -> Self {
        Self {
            offline: Mutex::new(false),
            delivered: Mutex::new(Vec::new()),
            permanent_fail: true,
        }
    }
    fn go_online(&self) {
        *self.offline.lock().unwrap() = false;
    }
    fn msg_ids(&self) -> Vec<String> {
        self.delivered
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.msg_id().to_string())
            .collect()
    }
}

#[async_trait]
impl PushTransport for MockPeer {
    async fn push(&self, env: &SyncEnvelope) -> sync::Result<()> {
        if self.permanent_fail {
            return Err(SyncError::transport_permanent("400 firma rechazada"));
        }
        if *self.offline.lock().unwrap() {
            return Err(SyncError::transport_retryable("peer offline"));
        }
        // Verify like the real receiver, then de-dupe on msg_id (idempotent).
        env.verify()?;
        let mut d = self.delivered.lock().unwrap();
        if !d.iter().any(|e| e.msg_id() == env.msg_id()) {
            d.push(env.clone());
        }
        Ok(())
    }
}

#[async_trait]
impl PullTransport for MockPeer {
    async fn pull(&self, from_did: &str) -> sync::Result<Vec<SyncEnvelope>> {
        Ok(self
            .delivered
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.from_did() == from_did)
            .cloned()
            .collect())
    }
}

#[tokio::test]
async fn enqueue_inserts_pending_row() {
    let (db, a, _) = setup().await;
    let id = Identity::generate();
    let env = mk_env(&id, "did:pharma:peer", "m1", "SKU1", 9);
    let out = enqueue(&db, &a, &env, Utc::now()).await.unwrap();
    assert!(!out.is_duplicate());
    assert_eq!(pending_depth(&db, &a).await.unwrap(), 1);
}

#[tokio::test]
async fn enqueue_is_idempotent_on_msg_id() {
    let (db, a, _) = setup().await;
    let id = Identity::generate();
    let env = mk_env(&id, "did:pharma:peer", "dup", "SKU1", 9);
    let first = enqueue(&db, &a, &env, Utc::now()).await.unwrap();
    let second = enqueue(&db, &a, &env, Utc::now()).await.unwrap();
    assert!(!first.is_duplicate());
    assert!(second.is_duplicate(), "second enqueue must dedupe");
    assert_eq!(first.id(), second.id(), "same row id");
    assert_eq!(pending_depth(&db, &a).await.unwrap(), 1, "no second row");
}

#[tokio::test]
async fn cross_tenant_drain_is_isolated() {
    let (db, a, b) = setup().await;
    let id = Identity::generate();
    let env = mk_env(&id, "did:pharma:peer", "m1", "SKU1", 9);
    enqueue(&db, &a, &env, Utc::now()).await.unwrap();

    // Draining tenant B must not touch tenant A's row.
    let peer = MockPeer::online();
    let report = drain(&db, &b, &peer, Utc::now()).await.unwrap();
    assert_eq!(report.sent, 0, "B has nothing to send");
    assert!(peer.msg_ids().is_empty());
    assert_eq!(pending_depth(&db, &a).await.unwrap(), 1, "A still pending");
}

#[tokio::test]
async fn drain_delivers_and_acks() {
    let (db, a, _) = setup().await;
    let id = Identity::generate();
    let env = mk_env(&id, "did:pharma:peer", "m1", "SKU1", 9);
    enqueue(&db, &a, &env, Utc::now()).await.unwrap();

    let peer = MockPeer::online();
    let report = drain(&db, &a, &peer, Utc::now()).await.unwrap();
    assert_eq!(report.sent, 1);
    assert_eq!(report.pending_remaining, 0);
    assert_eq!(peer.msg_ids(), vec!["m1"]);
    assert_eq!(pending_depth(&db, &a).await.unwrap(), 0);
}

#[tokio::test]
async fn peer_offline_keeps_pending_then_drains_on_recovery() {
    let (db, a, _) = setup().await;
    let id = Identity::generate();
    let env = mk_env(&id, "did:pharma:peer", "m1", "SKU1", 9);
    enqueue(&db, &a, &env, Utc::now()).await.unwrap();

    let peer = MockPeer::offline();
    // First drain: peer down → row stays pending, nothing delivered.
    let r1 = drain(&db, &a, &peer, Utc::now()).await.unwrap();
    assert_eq!(r1.sent, 0);
    assert_eq!(r1.pending_remaining, 1);
    assert!(peer.msg_ids().is_empty());

    // Peer returns; wait out the 1s backoff, drain again → delivered.
    peer.go_online();
    let later = Utc::now() + Duration::seconds(2);
    let r2 = drain(&db, &a, &peer, later).await.unwrap();
    assert_eq!(r2.sent, 1);
    assert_eq!(peer.msg_ids(), vec!["m1"]);
    assert_eq!(pending_depth(&db, &a).await.unwrap(), 0);
}

#[tokio::test]
async fn backoff_gates_retry_until_due() {
    let (db, a, _) = setup().await;
    let id = Identity::generate();
    let env = mk_env(&id, "did:pharma:peer", "m1", "SKU1", 9);
    let t0 = Utc::now();
    enqueue(&db, &a, &env, t0).await.unwrap();

    let peer = MockPeer::offline();
    drain(&db, &a, &peer, t0).await.unwrap(); // attempts=1, next=+1s
    peer.go_online();

    // Immediately retrying (before the 1s backoff) finds nothing due.
    let r_early = drain(&db, &a, &peer, t0).await.unwrap();
    assert_eq!(r_early.sent, 0, "row not due yet");
    assert!(peer.msg_ids().is_empty());

    // After the backoff window it delivers.
    let r_due = drain(&db, &a, &peer, t0 + Duration::seconds(2))
        .await
        .unwrap();
    assert_eq!(r_due.sent, 1);
}

#[tokio::test]
async fn retry_budget_exhaustion_marks_failed() {
    let (db, a, _) = setup().await;
    let id = Identity::generate();
    let env = mk_env(&id, "did:pharma:peer", "m1", "SKU1", 9);
    let mut now = Utc::now();
    enqueue(&db, &a, &env, now).await.unwrap();

    let peer = MockPeer::offline();
    // Keep retrying past the budget; advance time each pass to clear backoff.
    for _ in 0..sync::MAX_ATTEMPTS + 1 {
        drain(&db, &a, &peer, now).await.unwrap();
        now += Duration::seconds(60);
    }
    assert_eq!(
        pending_depth(&db, &a).await.unwrap(),
        0,
        "no longer pending"
    );
    let rows = list(&db, &a, 10).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, sync::OutboxStatus::Failed);
    assert_eq!(rows[0].attempts, sync::MAX_ATTEMPTS);
}

#[tokio::test]
async fn head_of_line_order_preserved_on_transient_failure() {
    let (db, a, _) = setup().await;
    let id = Identity::generate();
    let t0 = Utc::now();
    // Three queued in order; enqueue with increasing created_at.
    for (i, sku) in ["SKU1", "SKU2", "SKU3"].iter().enumerate() {
        let env = mk_env(&id, "did:pharma:peer", &format!("m{i}"), sku, 9 - i as i64);
        enqueue(&db, &a, &env, t0 + Duration::milliseconds(i as i64))
            .await
            .unwrap();
    }

    let peer = MockPeer::offline();
    // Offline: head blocks, nothing delivered.
    let r1 = drain(&db, &a, &peer, t0).await.unwrap();
    assert_eq!(r1.sent, 0);
    assert_eq!(r1.pending_remaining, 3);

    // Online: all three delivered in enqueue order.
    peer.go_online();
    let r2 = drain(&db, &a, &peer, t0 + Duration::seconds(60))
        .await
        .unwrap();
    assert_eq!(r2.sent, 3);
    assert_eq!(peer.msg_ids(), vec!["m0", "m1", "m2"]);
}

#[tokio::test]
async fn tampered_queued_envelope_rejected_on_drain() {
    let (db, a, _) = setup().await;
    let id = Identity::generate();
    let env = mk_env(&id, "did:pharma:peer", "m1", "SKU1", 9);
    // Corrupt the queued JSON body AFTER signing, then store it directly.
    let mut json: serde_json::Value = serde_json::from_str(&env.to_json().unwrap()).unwrap();
    json["body"]["new_stock"] = serde_json::json!(999999);
    let tampered = serde_json::to_string(&json).unwrap();
    db.query(
        "CREATE sync_outbox SET tenant=$t, peer_did='did:pharma:peer', msg_id='m1', \
         envelope=$e, status='pending', attempts=0, created_at=$ts, next_attempt_at=$ts, \
         last_error=NONE",
    )
    .bind(("t", a.clone()))
    .bind(("e", tampered))
    .bind(("ts", Utc::now().to_rfc3339()))
    .await
    .unwrap();

    let peer = MockPeer::online();
    let report = drain(&db, &a, &peer, Utc::now()).await.unwrap();
    assert_eq!(report.sent, 0, "tampered row never delivered");
    assert_eq!(report.failed, 1);
    assert!(peer.msg_ids().is_empty());
    let rows = list(&db, &a, 10).await.unwrap();
    assert_eq!(rows[0].status, sync::OutboxStatus::Failed);
}

#[tokio::test]
async fn permanent_transport_error_fails_without_retry() {
    let (db, a, _) = setup().await;
    let id = Identity::generate();
    let env = mk_env(&id, "did:pharma:peer", "m1", "SKU1", 9);
    enqueue(&db, &a, &env, Utc::now()).await.unwrap();

    let peer = MockPeer::permanent();
    let report = drain(&db, &a, &peer, Utc::now()).await.unwrap();
    assert_eq!(report.sent, 0);
    assert_eq!(report.failed, 1);
    let rows = list(&db, &a, 10).await.unwrap();
    assert_eq!(rows[0].status, sync::OutboxStatus::Failed);
    assert_eq!(rows[0].attempts, 0, "no retry consumed on contract error");
}

#[tokio::test]
async fn idempotent_redrain_does_not_double_deliver() {
    let (db, a, _) = setup().await;
    let id = Identity::generate();
    let env = mk_env(&id, "did:pharma:peer", "m1", "SKU1", 9);
    enqueue(&db, &a, &env, Utc::now()).await.unwrap();

    let peer = MockPeer::online();
    drain(&db, &a, &peer, Utc::now()).await.unwrap();
    // Second drain: row already sent → nothing to do, peer not hit again.
    let r2 = drain(&db, &a, &peer, Utc::now()).await.unwrap();
    assert_eq!(r2.sent, 0);
    assert_eq!(peer.msg_ids(), vec!["m1"], "delivered exactly once");
}

#[tokio::test]
async fn push_pull_roundtrip_preserves_order() {
    let (db, a, _) = setup().await;
    let id = Identity::generate();
    let from = id.did();
    let t0 = Utc::now();
    let skus = ["A", "B", "C", "D", "E"];
    for (i, sku) in skus.iter().enumerate() {
        let env = mk_env(
            &id,
            "did:pharma:peer",
            &format!("seq{i}"),
            sku,
            5 - i as i64,
        );
        enqueue(&db, &a, &env, t0 + Duration::milliseconds(i as i64))
            .await
            .unwrap();
    }
    let peer = MockPeer::online();
    drain(&db, &a, &peer, t0 + Duration::seconds(1))
        .await
        .unwrap();

    // Pull back what the peer accepted from this node — order matches enqueue.
    let pulled = peer.pull(&from).await.unwrap();
    let pulled_skus: Vec<String> = pulled
        .iter()
        .map(|e| e.delta().unwrap().external_id)
        .collect();
    assert_eq!(pulled_skus, skus.to_vec());
}

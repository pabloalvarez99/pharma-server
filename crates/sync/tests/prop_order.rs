//! Property test (slice 5): for any randomized, interleaved sequence of stock
//! deltas across two tenants, draining each tenant pushes its envelopes to the
//! peer in exactly the order they were enqueued — and never leaks another
//! tenant's deltas into that order. Generative coverage of the per-tenant FIFO
//! invariant beyond the hand-written `push_pull_roundtrip_preserves_order` case.

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use proptest::prelude::*;
use surrealdb::engine::local::Mem;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

use agent::Identity;
use sync::{drain, enqueue, PushTransport, StockDelta, SyncEnvelope};

type Db = Surreal<surrealdb::engine::local::Db>;

struct CollectPeer {
    delivered: Mutex<Vec<SyncEnvelope>>,
}

#[async_trait]
impl PushTransport for CollectPeer {
    async fn push(&self, env: &SyncEnvelope) -> sync::Result<()> {
        env.verify()?;
        let mut d = self.delivered.lock().unwrap();
        if !d.iter().any(|e| e.msg_id() == env.msg_id()) {
            d.push(env.clone());
        }
        Ok(())
    }
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

/// One enqueue: which tenant (false=A, true=B) and the new-stock value.
type Op = (bool, i64);

async fn run_case(ops: Vec<Op>) {
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("t").use_db("t").await.unwrap();
    let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/../../migrations");
    db::run_migrations(&db, migrations).await.unwrap();

    let a = mk_tenant(&db, "ta").await;
    let b = mk_tenant(&db, "tb").await;
    let id = Identity::generate();
    let t0 = Utc::now();

    let mut expect_a: Vec<String> = Vec::new();
    let mut expect_b: Vec<String> = Vec::new();

    for (i, (to_b, stock)) in ops.iter().enumerate() {
        let msg = format!("m{i}");
        let sku = format!("SKU{i}");
        let env = mk_env(&id, &msg, &sku, *stock);
        let tenant = if *to_b { &b } else { &a };
        // Distinct, increasing created_at so enqueue order is well-defined.
        enqueue(&db, tenant, &env, t0 + Duration::milliseconds(i as i64))
            .await
            .unwrap();
        if *to_b {
            expect_b.push(msg);
        } else {
            expect_a.push(msg);
        }
    }

    let peer_a = CollectPeer {
        delivered: Mutex::new(Vec::new()),
    };
    let peer_b = CollectPeer {
        delivered: Mutex::new(Vec::new()),
    };
    let after = t0 + Duration::seconds(1);
    drain(&db, &a, &peer_a, after).await.unwrap();
    drain(&db, &b, &peer_b, after).await.unwrap();

    let got_a: Vec<String> = peer_a
        .delivered
        .lock()
        .unwrap()
        .iter()
        .map(|e| e.msg_id().to_string())
        .collect();
    let got_b: Vec<String> = peer_b
        .delivered
        .lock()
        .unwrap()
        .iter()
        .map(|e| e.msg_id().to_string())
        .collect();
    assert_eq!(got_a, expect_a, "tenant A FIFO order");
    assert_eq!(got_b, expect_b, "tenant B FIFO order");
}

fn mk_env(id: &Identity, msg_id: &str, sku: &str, new_stock: i64) -> SyncEnvelope {
    let delta = StockDelta::new(sku, -1, new_stock, Utc::now().to_rfc3339());
    SyncEnvelope::create(id, "did:pharma:peer", msg_id, delta).unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]
    #[test]
    fn per_tenant_order_preserved(ops in proptest::collection::vec((any::<bool>(), 0i64..1000), 1..16)) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(run_case(ops));
    }
}

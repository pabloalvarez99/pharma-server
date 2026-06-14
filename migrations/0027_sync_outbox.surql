-- pharma-server 0027: durable stock-sync outbox (Fase 12, ADR-0013 Patrón B,
-- offline-first complement).
--
-- ADR-0013's in-memory webhook (crates/api/src/stock_webhook.rs, PR #162) is
-- best-effort: on a prolonged web/peer outage it exhausts its 3 retries and
-- DROPS the event, relying on the nightly pull-catalog reconcile to repair the
-- drift. That preserves the POS hot path but loses near-real-time accuracy for
-- the whole outage window. This table adds the durable layer the ADR's
-- offline-first goal calls for ("si la web está caída, pharma-server acumula
-- deltas y los empuja cuando vuelve"): a BOUNDED, on-disk, tenant-scoped queue
-- of signed stock-delta envelopes that drains when the peer returns — drift is
-- corrected in seconds on recovery instead of waiting for the nightly job.
--
-- On-disk + bounded (status transitions to `sent`/`failed`; failed rows are
-- prunable) so it does NOT reintroduce the unbounded-memory-growth the ADR
-- rejected for the in-memory pusher.
--
-- Multi-tenant obligatorio (CLAUDE.md regla #4): every row carries
-- `tenant: record<tenant>` and every query is tenant-scoped. Idempotency is a
-- UNIQUE index on (tenant, msg_id) — the envelope's msg_id is the dedupe key,
-- so a re-enqueue of the same movement is a no-op and a re-drain never
-- double-delivers.

DEFINE TABLE sync_outbox SCHEMAFULL;

DEFINE FIELD tenant          ON sync_outbox TYPE record<tenant>;
-- Recipient federated peer DID (`did:pharma:<pk>`).
DEFINE FIELD peer_did        ON sync_outbox TYPE string;
-- Idempotency key = the signed envelope's msg_id.
DEFINE FIELD msg_id          ON sync_outbox TYPE string;
-- Canonical signed-envelope JSON (re-verified on drain; never trusted blindly).
DEFINE FIELD envelope        ON sync_outbox TYPE string;
DEFINE FIELD status          ON sync_outbox TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending', 'sent', 'failed'];
DEFINE FIELD attempts        ON sync_outbox TYPE int DEFAULT 0 ASSERT $value >= 0;
DEFINE FIELD created_at       ON sync_outbox TYPE string;
DEFINE FIELD next_attempt_at ON sync_outbox TYPE string;
DEFINE FIELD last_error      ON sync_outbox TYPE option<string>;

-- Ordered drain: pending rows per tenant in enqueue order. Including `status`
-- keeps the pending scan selective once most rows are sent.
DEFINE INDEX sync_outbox_tenant_status_created
    ON sync_outbox FIELDS tenant, status, created_at;

-- Idempotency: one row per (tenant, msg_id). A duplicate enqueue collides here.
DEFINE INDEX sync_outbox_tenant_msg
    ON sync_outbox FIELDS tenant, msg_id UNIQUE;

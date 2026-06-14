-- pharma-server 0025: store-and-forward relay queue (BACKLOG #7).
-- SCHEMAFULL. An `agent_relay` row is a signed `Envelope` (crates/agent
-- envelope.rs) queued for delivery to a federated peer DID that is currently
-- unreachable. A drain pass re-verifies the stored envelope and attempts
-- delivery; failures back off and retry up to a bounded attempt cap, after
-- which the row is terminal `failed`.
--
-- Tenant-scoped (the local tenant that originated the message) AND carries the
-- target peer DID. The envelope itself is stored as its canonical JSON string
-- so the queue carries no schema knowledge of the wire format — drain parses +
-- verifies it (tamper in storage is rejected, never delivered).
--
-- Federation is opt-in: enqueue/drain only run when the tenant's `admin_setting`
-- key `federation_enabled` = "true" (enforced by the caller, same as
-- /agent/inbox — never here).
--
-- Idempotency: `(tenant, msg_id)` is UNIQUE, so re-enqueuing the same envelope
-- never creates a duplicate and re-draining never double-delivers.

DEFINE TABLE agent_relay SCHEMAFULL;
DEFINE FIELD tenant          ON agent_relay TYPE record<tenant>;
DEFINE FIELD msg_id          ON agent_relay TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD target_did      ON agent_relay TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD envelope_json   ON agent_relay TYPE string;
DEFINE FIELD status          ON agent_relay TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending', 'sent', 'failed'];
DEFINE FIELD attempts        ON agent_relay TYPE int DEFAULT 0;
DEFINE FIELD next_attempt_at ON agent_relay TYPE datetime DEFAULT time::now();
DEFINE FIELD last_error      ON agent_relay TYPE option<string>;
DEFINE FIELD created_at      ON agent_relay TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at      ON agent_relay TYPE datetime DEFAULT time::now();

-- Idempotency key: one queued copy of an envelope per tenant.
DEFINE INDEX agent_relay_tenant_msg ON agent_relay FIELDS tenant, msg_id UNIQUE;
-- Drain selector: pending rows for a peer that are due, oldest first.
DEFINE INDEX agent_relay_drain ON agent_relay FIELDS tenant, target_did, status, next_attempt_at;

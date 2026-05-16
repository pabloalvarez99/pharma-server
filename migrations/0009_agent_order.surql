-- pharma-server 0009: inter-node purchase orders (Fase 11 step 4).
-- SCHEMAFULL. An `agent_order` is a purchase order a remote signed peer
-- placed ON one of this install's tenants via `po.create`. It is
-- tenant-scoped (the supplier tenant that fulfills it) AND carries the
-- buyer's federation DID. `lines` is stored as a JSON string to avoid
-- nested-schema gymnastics (cf. serde-flatten/Option<Thing> gotcha).
--
-- Trading is opt-in: a tenant only answers quote.request / po.create if it
-- has `admin_setting` key `federation_enabled` = "true" (enforced in the
-- /agent/inbox handler, never here).

DEFINE TABLE agent_order SCHEMAFULL;
DEFINE FIELD tenant     ON agent_order TYPE record<tenant>;
DEFINE FIELD peer_did   ON agent_order TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD lines_json ON agent_order TYPE string;
DEFINE FIELD total      ON agent_order TYPE decimal DEFAULT 0;
DEFINE FIELD currency   ON agent_order TYPE string DEFAULT 'CLP';
DEFINE FIELD status     ON agent_order TYPE string DEFAULT 'received'
    ASSERT $value IN ['received', 'accepted', 'rejected', 'fulfilled', 'cancelled'];
DEFINE FIELD buyer_note ON agent_order TYPE option<string>;
DEFINE FIELD created_at ON agent_order TYPE datetime DEFAULT time::now();
DEFINE INDEX agent_order_tenant_created ON agent_order FIELDS tenant, created_at;
DEFINE INDEX agent_order_peer           ON agent_order FIELDS peer_did, created_at;
DEFINE INDEX agent_order_tenant_status  ON agent_order FIELDS tenant, status, created_at;

-- pharma-server 0008: agent federation log (Fase 11 step 2).
-- SCHEMAFULL. NODE-LEVEL (NOT tenant-scoped): inter-node messaging is between
-- sovereign pharma-server installs, not between tenants. Each row records one
-- inbound envelope this node accepted, forming a local-only trust graph
-- (reputation is never centralized — see docs/strategy/ecosystem-roadmap.md).

DEFINE TABLE agent_interaction SCHEMAFULL;
DEFINE FIELD peer_did   ON agent_interaction TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD topic      ON agent_interaction TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD msg_id     ON agent_interaction TYPE string;
DEFINE FIELD outcome    ON agent_interaction TYPE string DEFAULT 'ok'
    ASSERT $value IN ['ok', 'rejected', 'error'];
DEFINE FIELD created_at ON agent_interaction TYPE datetime DEFAULT time::now();
DEFINE INDEX agent_interaction_peer  ON agent_interaction FIELDS peer_did, created_at;
DEFINE INDEX agent_interaction_topic ON agent_interaction FIELDS topic, created_at;

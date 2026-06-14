-- pharma-server 0026: scoped API keys for the public DSS storefront seam
-- (ADR-0014 L1 "endurecer el seam").
--
-- The public endpoints `GET /api/v1/public/catalog` and
-- `POST /api/v1/public/orders/web` are opt-in (ADR-0005 "no surprise public
-- exposure"). When an operator turns on `require_api_key`, a caller (a DSS
-- storefront) must present a tenant-scoped key carrying the right scope:
--   · `catalog:read`  → read the scrubbed public catalog
--   · `orders:write`  → push a web order (in ADDITION to the HMAC body sig)
--
-- Fail-closed: the plaintext key is shown ONCE at mint time (CLI) and never
-- stored. We persist only `key_hash` = SHA-256(plaintext) hex; a leaked DB can
-- not be replayed against the seam. Multi-tenant: every key belongs to exactly
-- one `tenant` and is only ever matched under that tenant (composite index).

DEFINE TABLE api_key SCHEMAFULL;
DEFINE FIELD tenant       ON api_key TYPE record<tenant>;
DEFINE FIELD key_hash     ON api_key TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD scopes       ON api_key TYPE array<string> DEFAULT [];
DEFINE FIELD label        ON api_key TYPE option<string>;
DEFINE FIELD active       ON api_key TYPE bool DEFAULT true;
DEFINE FIELD created_at   ON api_key TYPE datetime DEFAULT time::now();
DEFINE FIELD last_used_at ON api_key TYPE option<datetime>;

-- Primary lookup path: (tenant, key_hash). UNIQUE so a tenant can't hold the
-- same key twice.
DEFINE INDEX api_key_tenant_hash ON api_key FIELDS tenant, key_hash UNIQUE;
-- Global integrity: a key hash is unique across the whole install.
DEFINE INDEX api_key_hash ON api_key FIELDS key_hash UNIQUE;

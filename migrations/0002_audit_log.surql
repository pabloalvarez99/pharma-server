-- pharma-server 0002: audit_log
-- Append-only log of mutating HTTP requests (POST/PATCH/DELETE).
-- Inserted asynchronously by AuditLayer middleware (`crates/api/src/middleware/audit.rs`).
-- Read by `GET /api/v1/audit`. NEVER updated or deleted at application layer.

DEFINE TABLE audit_log SCHEMAFULL;

DEFINE FIELD tenant       ON audit_log TYPE record<tenant>;
DEFINE FIELD user         ON audit_log TYPE option<record<user>>;
DEFINE FIELD user_email   ON audit_log TYPE option<string>;
DEFINE FIELD method       ON audit_log TYPE string;
DEFINE FIELD path         ON audit_log TYPE string;
DEFINE FIELD status       ON audit_log TYPE option<int>;
DEFINE FIELD payload_hash ON audit_log TYPE option<string>;     -- sha256 hex of request body (mutations only)
DEFINE FIELD ip           ON audit_log TYPE option<string>;
DEFINE FIELD user_agent   ON audit_log TYPE option<string>;
DEFINE FIELD created_at   ON audit_log TYPE datetime DEFAULT time::now();

DEFINE INDEX audit_log_tenant_ts  ON audit_log FIELDS tenant, created_at;
DEFINE INDEX audit_log_user_ts    ON audit_log FIELDS user, created_at;
DEFINE INDEX audit_log_path_ts    ON audit_log FIELDS path, created_at;

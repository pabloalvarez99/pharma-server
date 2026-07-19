-- pharma-server initial schema
-- Multi-tenant model: every domain table carries a `tenant` ref.

DEFINE TABLE tenant SCHEMAFULL;
DEFINE FIELD name        ON tenant TYPE string;
DEFINE FIELD slug        ON tenant TYPE string ASSERT $value != NONE AND string::len($value) > 0;
DEFINE FIELD created_at  ON tenant TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at  ON tenant TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX tenant_slug ON tenant FIELDS slug UNIQUE;

DEFINE TABLE user SCHEMAFULL;
DEFINE FIELD tenant      ON user TYPE record<tenant>;
DEFINE FIELD email       ON user TYPE string ASSERT string::is::email($value);
DEFINE FIELD password    ON user TYPE string;          -- argon2id hash
DEFINE FIELD roles       ON user TYPE array<string> DEFAULT [];
DEFINE FIELD active      ON user TYPE bool DEFAULT true;
DEFINE FIELD created_at  ON user TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at  ON user TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX user_tenant_email ON user FIELDS tenant, email UNIQUE;

DEFINE TABLE session SCHEMAFULL;
DEFINE FIELD user        ON session TYPE record<user>;
DEFINE FIELD tenant      ON session TYPE record<tenant>;
DEFINE FIELD jti         ON session TYPE string;
DEFINE FIELD issued_at   ON session TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at  ON session TYPE datetime;
DEFINE FIELD revoked     ON session TYPE bool DEFAULT false;
DEFINE INDEX session_jti ON session FIELDS jti UNIQUE;

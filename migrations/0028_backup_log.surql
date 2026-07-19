-- pharma-server 0028: backup_log — install-wide audit trail of snapshots.
--
-- NOT tenant-scoped (deliberate exception to the multi-tenant rule, reglas #4).
-- A backup is a snapshot of the ENTIRE SurrealKv store (every tenant + the
-- `agent.key` identity) → it is a per-INSTALL operational artifact, not
-- per-tenant data. Restoring one restores all tenants. Hence no `tenant`
-- field and no tenant-prefixed composite index. The only access path is the
-- admin/owner backup surface (POST /api/v1/admin/backup, the scheduled job,
-- and `pharma backup`), all of which are already install-global.
--
-- One row per backup ATTEMPT (scheduled | manual endpoint | cli):
--   * status='ok'     → path/bytes/sha256/duration_ms populated.
--   * status='failed' → `error` populated; artifact fields may be NULL.
-- Rows are pruned independently of the .tar.gz files on disk
-- (db::backup_log::prune_log keeps the N newest rows) so the audit trail can
-- outlive the archives it references.

DEFINE TABLE backup_log SCHEMAFULL;
DEFINE FIELD status      ON backup_log TYPE string ASSERT $value IN ['ok', 'failed'];
DEFINE FIELD source      ON backup_log TYPE string DEFAULT 'scheduled'
    ASSERT $value IN ['scheduled', 'manual', 'cli'];
DEFINE FIELD path        ON backup_log TYPE option<string>;
DEFINE FIELD bytes       ON backup_log TYPE option<int>;
DEFINE FIELD sha256      ON backup_log TYPE option<string>;
DEFINE FIELD duration_ms ON backup_log TYPE option<int>;
DEFINE FIELD error       ON backup_log TYPE option<string>;
DEFINE FIELD started_at  ON backup_log TYPE datetime DEFAULT time::now();
DEFINE INDEX backup_log_started ON backup_log FIELDS started_at;

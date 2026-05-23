-- pharma-server 0017: notification — Fase 8 jobs scheduler outputs
--
-- Tenant-scoped append-only feed of system-generated notifications. The
-- near-expiry cron job (08:00 UTC daily) writes one row per tenant where
-- `admin_setting near_expiry_alert_enabled = "true"` with `kind="near_expiry"`
-- and a `payload` summarizing the batches expiring within 30 days.
--
-- Schema is intentionally generic (`kind` + `payload object`) so future cron
-- jobs (low_stock, license_expiring, backup_failed, …) can reuse it without
-- another migration. A `seen` flag + composite index keep "unread" queries
-- O(unread) for the admin UI.
--
-- Branch note: this slot (0017) was free on `feature/erp-parity` when this
-- branch (`feat/jobs-cron-activate`) was cut. The sibling DTE branch
-- (`feat/dte-9-1-abc-xml-ted-caf`) also claims 0017 for its own table; at
-- merge time renumber whichever lands second (migrations are tracked by
-- filename stem, so the rename is the migration).

DEFINE TABLE notification SCHEMAFULL;
DEFINE FIELD tenant     ON notification TYPE record<tenant>;
DEFINE FIELD kind       ON notification TYPE string ASSERT string::len($value) > 0;
-- FLEXIBLE so the cron writer can append arbitrary keys (count, batches, …)
-- without a per-key DEFINE FIELD. Without FLEXIBLE, SCHEMAFULL silently strips
-- undefined sub-keys → payload ends up as `{}`.
DEFINE FIELD payload    ON notification FLEXIBLE TYPE object;
DEFINE FIELD created_at ON notification TYPE datetime DEFAULT time::now();
DEFINE FIELD seen       ON notification TYPE bool DEFAULT false;
DEFINE INDEX notification_tenant_seen    ON notification FIELDS tenant, seen;
DEFINE INDEX notification_tenant_created ON notification FIELDS tenant, created_at;
DEFINE INDEX notification_tenant_kind    ON notification FIELDS tenant, kind, created_at;

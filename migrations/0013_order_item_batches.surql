-- Migration 0013 — multi-lot split traceability on order_item (BACKLOG #3)
--
-- Additive only. Adds `batches_json` to `order_item` (option<string>) holding a
-- JSON-encoded list of {batch, qty} pairs for the FEFO allocations consumed by
-- the line. The legacy `batch` field (primary/earliest-expiry lot) stays for
-- backward compat. NULL on rows persisted before this migration (and on lines
-- with no FEFO plan), so old clients keep working.
--
-- JSON-string column mirrors the established `agent_order.lines_json` pattern
-- in this codebase, which sidesteps SurrealQL array-of-objects binding traps.

DEFINE FIELD batches_json ON order_item TYPE option<string>;

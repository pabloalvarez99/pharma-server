-- pharma-server 0017: add `channel` field to `order` for web/POS provenance
-- (ADR-0012 pattern 2 — web→ERP push HMAC, `POST /api/v1/public/orders/web`).
--
-- Why: `crates/api/src/v1/public_orders.rs` writes orders with
-- `channel='web'` so downstream reports / reconciliation can distinguish
-- counter-walk-in sales (`channel='pos'` or NONE) from online pushes from
-- tu-farmacia.cl. The web push does NOT decrement stock — the order is a
-- reservation until pickup, at which point the POS flow handles the FEFO
-- decrement.
--
-- Idempotency for the web push reuses the existing `order.external_ref`
-- field as the storage slot for the caller-supplied `external_order_id`.
-- An index on `(tenant, channel, external_ref)` makes the replay lookup
-- O(log n) regardless of total order count.
--
-- Existing rows: channel stays NONE (option<string>), so reports must treat
-- NONE as the legacy POS counter channel.

DEFINE FIELD channel ON order TYPE option<string>
    ASSERT $value = NONE OR $value IN ['pos','web'];

DEFINE INDEX order_tenant_channel_ext_ref
    ON order FIELDS tenant, channel, external_ref;

-- pharma-server 0038: web pickup orders — status/channel extension, pickup
-- fields on `order`, `stock_reserved` on `product` (Free Web PR3, ADR-0020).
--
-- Redefines extend existing fields (append-only doctrine: a NEW migration file
-- may re-DEFINE a field to widen its ASSERT list):
-- * `status` gains 'preparing' + 'ready_for_pickup' for the pickup flow
--   reserved → preparing → ready_for_pickup → completed (0007 list kept).
-- * `channel` keeps `option<string>` (0019: NONE = legacy POS counter rows)
--   and gains 'agent' so all three provenance channels share one field.
--
-- `stock_reserved` is the web-reservation counter: a web pickup order holds
-- units without decrementing `stock` (POS truth). Sellable-for-web =
-- `stock - stock_reserved`. Completed pickup releases the reserve AND
-- decrements `stock`; cancel/expire releases only. Existing rows read the
-- DEFAULT under SCHEMAFULL, so no backfill.

DEFINE FIELD OVERWRITE status ON order TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending','paid','completed','reserved','preparing','ready_for_pickup','cancelled','refunded'];
DEFINE FIELD OVERWRITE channel ON order TYPE option<string>
    ASSERT $value = NONE OR $value IN ['pos','web','agent'];
DEFINE FIELD pickup_code      ON order TYPE option<string>;
DEFINE FIELD fulfillment_type ON order TYPE option<string>
    ASSERT $value = NONE OR $value IN ['pickup','delivery'];
DEFINE FIELD expires_at       ON order TYPE option<datetime>;
DEFINE FIELD ready_at         ON order TYPE option<datetime>;
DEFINE INDEX order_tenant_channel ON order FIELDS tenant, channel, created_at;

DEFINE FIELD stock_reserved ON product TYPE int DEFAULT 0;

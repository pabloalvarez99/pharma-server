-- pharma-server 0017: purchase-order receiving lifecycle (BACKLOG #8 Fase
-- 5-full — line-level recepción de mercadería).
--
-- Slice 1 (0015) modeled `purchase_order` with status enum
-- ['draft','received','cancelled'] and slice 2 activated the one-shot
-- `draft → received` whole-PO receipt. This migration extends the lifecycle
-- to a real receiving workflow:
--
--   * `sent` / `approved`  — a PO that has been issued to the supplier and is
--     awaiting goods. Receiving is only legal from these states (a `draft`
--     hasn't been sent yet, a `cancelled` is void).
--   * `partially_received` — at least one line received, but not every line is
--     fully fulfilled. Subsequent receipts on the same PO are additive.
--   * `received` — every catalogued+free-text line fully received.
--
-- `purchase_order_item.qty_received` tracks the cumulative quantity already
-- received per line so partial receipts are idempotent against over-receipt
-- (a line can never receive more than its ordered `quantity`).
--
-- Additive + append-only: never edit once applied. The `status` field is
-- redefined with OVERWRITE purely to widen the ASSERT enum — existing rows
-- (status ∈ old enum) stay valid. `qty_received` defaults to 0 so historical
-- `purchase_order_item` rows read back as "nothing received yet".
--
-- Stock receipt (weighted-average cost recompute + `stock_movement(
-- reason='purchase_receive')` + `product_batch` create/update when a lot is
-- supplied) is materialized atomically by `purchasing::repo` in the same
-- BEGIN/COMMIT — preserving `product.stock = SUM(stock_movement.delta)`.

DEFINE FIELD OVERWRITE status ON purchase_order TYPE string DEFAULT 'draft'
    ASSERT $value IN ['draft', 'sent', 'approved', 'partially_received', 'received', 'cancelled'];

DEFINE FIELD qty_received ON purchase_order_item TYPE int DEFAULT 0 ASSERT $value >= 0;

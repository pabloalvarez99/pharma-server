-- Migration 0014 — multi-lot split traceability on agent_order fulfillment
-- (BACKLOG #2 remainder, sibling of migration 0013 on the sales path).
--
-- Additive only. Adds `fulfillment_batches_json` to `agent_order`: a JSON-
-- encoded list of per-line FEFO breakdowns recorded by `agent_orders::fulfill`,
-- so the supplier (and audit) can trace exactly which lots shipped which qty
-- on a federated order. Buyer-side `lines_json` stays untouched (it is the
-- order intent, not the fulfillment record).
--
-- Shape:
--   [ { "product": "product:<id>", "allocations": [ { "batch": "product_batch:<id>", "qty": N }, … ] }, … ]
--
-- NULL on rows persisted before this migration and on orders not yet fulfilled
-- (and on fulfillments of non-batch-tracked products).

DEFINE FIELD fulfillment_batches_json ON agent_order TYPE option<string>;

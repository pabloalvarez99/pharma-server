-- pharma-server 0024: product.publish_to_web gate for ERP→web stock push (ADR-0013).
--
-- Patrón B (stock webhook) fires only for SKUs the operator marked publishable.
-- Default `false` = opt-in per SKU: no surprise exposure of internal products to
-- the public storefront (ADR-0005 invariant). `product` is already tenant-scoped
-- (migration 0003), so this is a column add, not a new table — no new index.
--
-- Existing rows predate the field; under SCHEMAFULL they read as the declared
-- DEFAULT (`false`) on next select, so the webhook's `publish_to_web = true`
-- filter treats them as "do not push" until the operator flips the flag. That
-- silence-by-default matches the ADR-0013 trigger rule ("NO disparan:
-- movimientos en SKUs con publish_to_web = false").

DEFINE FIELD publish_to_web ON product TYPE bool DEFAULT false;

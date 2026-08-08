-- pharma-server 0036: web storefront — product online projection fields (Free Web PR1, ADR-0020).
--
-- The free-tier public storefront (`/api/v1/public/{slug}/…`) lists only rows the
-- operator flipped to `online_visible = true` (opt-in per SKU, same doctrine as
-- `publish_to_web` in 0024 — that flag gates the ERP→web stock push webhook, this
-- one gates catalog visibility on the built-in storefront; independent channels).
-- `online_*` overrides let the shop show a web-friendly title/description/price
-- without touching the POS fields. Existing rows read the declared DEFAULTs under
-- SCHEMAFULL, so no backfill.

DEFINE FIELD online_visible     ON product TYPE bool DEFAULT false;
DEFINE FIELD online_title       ON product TYPE option<string>;
DEFINE FIELD online_description ON product TYPE option<string>;
DEFINE FIELD online_sort        ON product TYPE int DEFAULT 0;
DEFINE FIELD online_price       ON product TYPE option<decimal>;
DEFINE INDEX product_tenant_online ON product FIELDS tenant, online_visible, active, online_sort;

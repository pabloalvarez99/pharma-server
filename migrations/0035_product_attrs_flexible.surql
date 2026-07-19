-- pharma-server 0035: product.attrs must be FLEXIBLE.
--
-- Migration 0033 defined `attrs TYPE option<object>` without FLEXIBLE.
-- On SCHEMAFULL tables Surreal strips unknown nested keys, so create/update
-- with `{talla, color, sku}` persisted as `{}`. Same pattern as
-- `dte.metadata` (0022) and `dte.referencias` (0023): free-form rubro attrs
-- need FLEXIBLE or they are useless for multi-rubro packs.

DEFINE FIELD OVERWRITE attrs ON product FLEXIBLE TYPE option<object>;

-- pharma-server 0029: product_stats maintained aggregate (BUG-perf-001 fix)
-- ---------------------------------------------------------------------------
-- Per-tenant catalog aggregate (one row per tenant), maintained INCREMENTALLY
-- by a `DEFINE EVENT` on the `product` table. Replaces the O(n) full-scan
-- `GROUP ALL` in catalog::repo::stats — measured 2.7s p99 @50k SKUs (bob's
-- pos_hotpath bench), violating the <50ms POS budget. The stats read is now an
-- O(1) lookup of the maintained row.
--
-- Why an EVENT and not a pre-computed table view (`DEFINE TABLE … AS SELECT`):
-- SurrealDB's view maintenance mis-handles UPDATEs (flipping `active` dropped a
-- row from `total`; verified by the catalog tests). The event uses `$before`/
-- `$after` so every op — CREATE / UPDATE / DELETE — applies an exact delta.
--
-- The low-stock threshold is baked as 5; it MUST equal
-- catalog::model::LOW_STOCK_DEFAULT (asserted by `low_stock_threshold_matches_view`).
-- catalog::repo::stats falls back to the live scan for any non-default `low` and
-- for a tenant with no stats row yet (the scan is then trivially O(0)).
--
-- Semantics match the old scan exactly:
--   total            = count of products            (active + inactive)
--   active           = count where active = true
--   low_stock        = count where active = true AND stock <= 5
--   out_of_stock     = count where active = true AND stock <= 0
--   inventory_value  = sum(stock * (cost_price ?? 0))  (active + inactive)
-- Append-only: never edit once applied.
-- ---------------------------------------------------------------------------

DEFINE TABLE product_stats SCHEMAFULL;
DEFINE FIELD tenant          ON product_stats TYPE record<tenant>;
DEFINE FIELD total           ON product_stats TYPE int     DEFAULT 0;
DEFINE FIELD active          ON product_stats TYPE int     DEFAULT 0;
DEFINE FIELD low_stock       ON product_stats TYPE int     DEFAULT 0;
DEFINE FIELD out_of_stock    ON product_stats TYPE int     DEFAULT 0;
DEFINE FIELD inventory_value ON product_stats TYPE decimal DEFAULT 0dec;
DEFINE INDEX product_stats_tenant ON product_stats FIELDS tenant UNIQUE;

-- Incremental maintenance. `$before` is NONE on CREATE, `$after` is NONE on
-- DELETE; the per-op guards collapse the absent side's contribution to zero, so
-- the net delta applied to the tenant row is (after − before).
DEFINE EVENT product_stats_maint ON TABLE product
  WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN {
    LET $tenant = IF $event = "DELETE" THEN $before.tenant ELSE $after.tenant END;

    LET $bt = IF $event = "CREATE" THEN 0 ELSE 1 END;
    LET $ba = IF $event = "CREATE" THEN 0 ELSE (IF $before.active THEN 1 ELSE 0 END) END;
    LET $bl = IF $event = "CREATE" THEN 0
              ELSE (IF $before.active AND $before.stock <= 5 THEN 1 ELSE 0 END) END;
    LET $bo = IF $event = "CREATE" THEN 0
              ELSE (IF $before.active AND $before.stock <= 0 THEN 1 ELSE 0 END) END;
    LET $bi = IF $event = "CREATE" THEN 0dec
              ELSE ($before.stock * ($before.cost_price ?? 0dec)) END;

    LET $at = IF $event = "DELETE" THEN 0 ELSE 1 END;
    LET $aa = IF $event = "DELETE" THEN 0 ELSE (IF $after.active THEN 1 ELSE 0 END) END;
    LET $al = IF $event = "DELETE" THEN 0
              ELSE (IF $after.active AND $after.stock <= 5 THEN 1 ELSE 0 END) END;
    LET $ao = IF $event = "DELETE" THEN 0
              ELSE (IF $after.active AND $after.stock <= 0 THEN 1 ELSE 0 END) END;
    LET $ai = IF $event = "DELETE" THEN 0dec
              ELSE ($after.stock * ($after.cost_price ?? 0dec)) END;

    UPSERT type::thing('product_stats', meta::id($tenant)) SET
      tenant           = $tenant,
      total           += ($at - $bt),
      active          += ($aa - $ba),
      low_stock       += ($al - $bl),
      out_of_stock    += ($ao - $bo),
      inventory_value += ($ai - $bi);
  };

-- Backfill the upgrade path: an install that already has a catalog when this
-- migration runs gets correct numbers immediately (events only fire on future
-- writes). No-op on a fresh install (empty `product`).
FOR $r IN (
  SELECT
    tenant,
    count() AS total,
    count(active = true) AS active,
    count(active = true AND stock <= 5) AS low_stock,
    count(active = true AND stock <= 0) AS out_of_stock,
    math::sum(stock * (cost_price ?? 0dec)) AS inventory_value
  FROM product GROUP BY tenant
) {
  UPSERT type::thing('product_stats', meta::id($r.tenant)) SET
    tenant          = $r.tenant,
    total           = $r.total,
    active          = $r.active,
    low_stock       = $r.low_stock,
    out_of_stock    = $r.out_of_stock,
    inventory_value = ($r.inventory_value ?? 0dec);
};

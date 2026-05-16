-- pharma-server 0004: inventory (Fase 3 erp-parity)
-- Tables: stock_movement, product_batch, falta (all tenant-scoped).
-- Multi-tenant: every table carries `tenant: record<tenant>` + composite
-- indexes leading with tenant. Append-only — never edit once applied.
--
-- Invariant: product.stock = SUM(stock_movement.delta) per (tenant, product).
-- Mutations that change stock MUST go through stock_movement; the api/domain
-- layer materializes product.stock atomically in the same transaction.

-- ---------------------------------------------------------------------------
-- stock_movement  (audit trail; immutable rows)
-- ---------------------------------------------------------------------------
DEFINE TABLE stock_movement SCHEMAFULL;
DEFINE FIELD tenant      ON stock_movement TYPE record<tenant>;
DEFINE FIELD product     ON stock_movement TYPE record<product>;
DEFINE FIELD delta       ON stock_movement TYPE int ASSERT $value != 0;
DEFINE FIELD reason      ON stock_movement TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD admin       ON stock_movement TYPE option<record<user>>;
DEFINE FIELD ref         ON stock_movement TYPE option<string>;
DEFINE FIELD created_at  ON stock_movement TYPE datetime DEFAULT time::now();
DEFINE INDEX sm_tenant_product_created ON stock_movement FIELDS tenant, product, created_at;
DEFINE INDEX sm_tenant_created         ON stock_movement FIELDS tenant, created_at;
DEFINE INDEX sm_tenant_reason          ON stock_movement FIELDS tenant, reason;

-- ---------------------------------------------------------------------------
-- product_batch  (lotes con vencimiento; FEFO consumption por sales)
-- ---------------------------------------------------------------------------
DEFINE TABLE product_batch SCHEMAFULL;
DEFINE FIELD tenant      ON product_batch TYPE record<tenant>;
DEFINE FIELD product     ON product_batch TYPE record<product>;
DEFINE FIELD batch_code  ON product_batch TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD expiry_date ON product_batch TYPE datetime;
DEFINE FIELD stock       ON product_batch TYPE int DEFAULT 0 ASSERT $value >= 0;
DEFINE FIELD cost        ON product_batch TYPE option<decimal>;
DEFINE FIELD notes       ON product_batch TYPE option<string>;
DEFINE FIELD active      ON product_batch TYPE bool DEFAULT true;
DEFINE FIELD created_at  ON product_batch TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at  ON product_batch TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX pb_tenant_product_expiry ON product_batch FIELDS tenant, product, expiry_date;
DEFINE INDEX pb_tenant_batch          ON product_batch FIELDS tenant, batch_code;
DEFINE INDEX pb_tenant_active_expiry  ON product_batch FIELDS tenant, active, expiry_date;

-- ---------------------------------------------------------------------------
-- falta  (productos faltantes a reponer; opcional referencia a product)
-- ---------------------------------------------------------------------------
DEFINE TABLE falta SCHEMAFULL;
DEFINE FIELD tenant      ON falta TYPE record<tenant>;
DEFINE FIELD product     ON falta TYPE option<record<product>>;
DEFINE FIELD name        ON falta TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD qty         ON falta TYPE int DEFAULT 1 ASSERT $value > 0;
DEFINE FIELD customer_name  ON falta TYPE option<string>;
DEFINE FIELD customer_phone ON falta TYPE option<string>;
DEFINE FIELD notes       ON falta TYPE option<string>;
DEFINE FIELD resolved    ON falta TYPE bool DEFAULT false;
DEFINE FIELD created_at  ON falta TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at  ON falta TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX falta_tenant_resolved_created ON falta FIELDS tenant, resolved, created_at;
DEFINE INDEX falta_tenant_product          ON falta FIELDS tenant, product;

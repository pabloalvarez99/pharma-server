-- pharma-server 0003: catalog (Fase 2 erp-parity)
-- Tables: category, product, product_barcode (tenant-scoped),
--         barcode_catalog, therapeutic_category_mapping (GLOBAL, no tenant).
-- Multi-tenant: every tenant table carries `tenant: record<tenant>` + a
-- composite index that leads with `tenant`. Append-only: never edit once applied.

-- ---------------------------------------------------------------------------
-- category
-- ---------------------------------------------------------------------------
DEFINE TABLE category SCHEMAFULL;
DEFINE FIELD tenant      ON category TYPE record<tenant>;
DEFINE FIELD name        ON category TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD slug        ON category TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD description  ON category TYPE option<string>;
DEFINE FIELD image_url   ON category TYPE option<string>;
DEFINE FIELD active      ON category TYPE bool DEFAULT true;
DEFINE FIELD created_at  ON category TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at  ON category TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX category_tenant_slug ON category FIELDS tenant, slug UNIQUE;

-- ---------------------------------------------------------------------------
-- product
-- ---------------------------------------------------------------------------
DEFINE TABLE product SCHEMAFULL;
DEFINE FIELD tenant             ON product TYPE record<tenant>;
DEFINE FIELD name               ON product TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD slug               ON product TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD description        ON product TYPE option<string>;
DEFINE FIELD price              ON product TYPE decimal ASSERT $value >= 0;
DEFINE FIELD cost_price         ON product TYPE option<decimal>;
DEFINE FIELD stock              ON product TYPE int DEFAULT 0;
DEFINE FIELD category           ON product TYPE option<record<category>>;
DEFINE FIELD image_url          ON product TYPE option<string>;
DEFINE FIELD active             ON product TYPE bool DEFAULT true;
DEFINE FIELD external_id        ON product TYPE option<string>;
DEFINE FIELD laboratory         ON product TYPE option<string>;
DEFINE FIELD therapeutic_action ON product TYPE option<string>;
DEFINE FIELD active_ingredient  ON product TYPE option<string>;
DEFINE FIELD prescription_type  ON product TYPE string DEFAULT 'direct';
DEFINE FIELD presentation       ON product TYPE option<string>;
DEFINE FIELD discount_percent   ON product TYPE option<int>;
DEFINE FIELD created_at         ON product TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at         ON product TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX product_tenant_slug     ON product FIELDS tenant, slug UNIQUE;
DEFINE INDEX product_tenant_active   ON product FIELDS tenant, active;
DEFINE INDEX product_tenant_category ON product FIELDS tenant, category;
DEFINE INDEX product_tenant_lab      ON product FIELDS tenant, laboratory;
DEFINE INDEX product_tenant_extid    ON product FIELDS tenant, external_id;
DEFINE INDEX product_tenant_stock    ON product FIELDS tenant, stock;

-- ---------------------------------------------------------------------------
-- product_barcode  (tenant-private barcodes; N:1 product)
-- ---------------------------------------------------------------------------
DEFINE TABLE product_barcode SCHEMAFULL;
DEFINE FIELD tenant      ON product_barcode TYPE record<tenant>;
DEFINE FIELD product     ON product_barcode TYPE record<product>;
DEFINE FIELD barcode     ON product_barcode TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD created_at  ON product_barcode TYPE datetime DEFAULT time::now();
DEFINE INDEX product_barcode_tenant_code ON product_barcode FIELDS tenant, barcode UNIQUE;
DEFINE INDEX product_barcode_tenant_prod ON product_barcode FIELDS tenant, product;

-- ---------------------------------------------------------------------------
-- barcode_catalog  (GLOBAL — shared Chile EAN/UPC -> external_id; no tenant)
-- Decision: brain/pharma-server-decisions.md "barcode_catalog global".
-- ---------------------------------------------------------------------------
DEFINE TABLE barcode_catalog SCHEMAFULL;
DEFINE FIELD external_id ON barcode_catalog TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD barcode     ON barcode_catalog TYPE string ASSERT string::len($value) > 0;
DEFINE INDEX barcode_catalog_code   ON barcode_catalog FIELDS barcode UNIQUE;
DEFINE INDEX barcode_catalog_extid  ON barcode_catalog FIELDS external_id;

-- ---------------------------------------------------------------------------
-- therapeutic_category_mapping  (GLOBAL lookup; no tenant)
-- ---------------------------------------------------------------------------
DEFINE TABLE therapeutic_category_mapping SCHEMAFULL;
DEFINE FIELD therapeutic_action ON therapeutic_category_mapping TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD category_slug      ON therapeutic_category_mapping TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD created_at         ON therapeutic_category_mapping TYPE datetime DEFAULT time::now();
DEFINE INDEX tcm_action ON therapeutic_category_mapping FIELDS therapeutic_action;

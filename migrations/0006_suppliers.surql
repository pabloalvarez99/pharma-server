-- pharma-server 0006: suppliers + price-lists (Fase 5-subset erp-parity).
-- Scope: supplier, supplier_product_mapping, supplier_price_list.
-- purchase_order / purchase_order_item / purchase_payment quedan para fase
-- posterior (dependen de inventory Fase 3 — stock_movement/product_batch +
-- costo promedio ponderado).
-- Multi-tenant: every table carries `tenant: record<tenant>` + composite
-- index leading with tenant. Append-only: never edit once applied.

-- ---------------------------------------------------------------------------
-- supplier
-- ---------------------------------------------------------------------------
DEFINE TABLE supplier SCHEMAFULL;
DEFINE FIELD tenant                 ON supplier TYPE record<tenant>;
DEFINE FIELD name                   ON supplier TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD rut                    ON supplier TYPE option<string>;
DEFINE FIELD contact_name           ON supplier TYPE option<string>;
DEFINE FIELD contact_email          ON supplier TYPE option<string>;
DEFINE FIELD contact_phone          ON supplier TYPE option<string>;
DEFINE FIELD default_invoice_format ON supplier TYPE option<string>;
DEFINE FIELD active                 ON supplier TYPE bool DEFAULT true;
DEFINE FIELD created_at             ON supplier TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at             ON supplier TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX supplier_tenant_rut  ON supplier FIELDS tenant, rut;
DEFINE INDEX supplier_tenant_name ON supplier FIELDS tenant, name;

-- ---------------------------------------------------------------------------
-- supplier_product_mapping  (supplier_code → product)
-- ---------------------------------------------------------------------------
DEFINE TABLE supplier_product_mapping SCHEMAFULL;
DEFINE FIELD tenant        ON supplier_product_mapping TYPE record<tenant>;
DEFINE FIELD supplier      ON supplier_product_mapping TYPE record<supplier>;
DEFINE FIELD product       ON supplier_product_mapping TYPE record<product>;
DEFINE FIELD supplier_code ON supplier_product_mapping TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD created_at    ON supplier_product_mapping TYPE datetime DEFAULT time::now();
DEFINE INDEX spm_tenant_supplier_code ON supplier_product_mapping FIELDS tenant, supplier, supplier_code UNIQUE;
DEFINE INDEX spm_tenant_product       ON supplier_product_mapping FIELDS tenant, product;

-- ---------------------------------------------------------------------------
-- supplier_price_list  (histórico precios por proveedor)
-- product is optional: una línea puede referenciar solo `supplier_code`
-- cuando aún no existe mapping a producto interno.
-- ---------------------------------------------------------------------------
DEFINE TABLE supplier_price_list SCHEMAFULL;
DEFINE FIELD tenant        ON supplier_price_list TYPE record<tenant>;
DEFINE FIELD supplier      ON supplier_price_list TYPE record<supplier>;
DEFINE FIELD product       ON supplier_price_list TYPE option<record<product>>;
DEFINE FIELD supplier_code ON supplier_price_list TYPE option<string>;
DEFINE FIELD description   ON supplier_price_list TYPE option<string>;
DEFINE FIELD unit_cost     ON supplier_price_list TYPE decimal ASSERT $value >= 0;
DEFINE FIELD currency      ON supplier_price_list TYPE string DEFAULT 'CLP';
DEFINE FIELD valid_from    ON supplier_price_list TYPE datetime DEFAULT time::now();
DEFINE FIELD created_at    ON supplier_price_list TYPE datetime DEFAULT time::now();
DEFINE INDEX spl_tenant_supplier_created ON supplier_price_list FIELDS tenant, supplier, created_at;
DEFINE INDEX spl_tenant_product_created  ON supplier_price_list FIELDS tenant, product, created_at;

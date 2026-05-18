-- pharma-server 0015: local purchase orders (Fase 5-full, BACKLOG #8 slice 1).
--
-- `purchase_order` is a tenant-scoped procurement document against a
-- `supplier` (migration 0006). The header carries a computed `total`
-- (sum of item `subtotal`s). `purchase_order_item` are its lines —
-- relational, mirrors `order_item` (migration 0007) rather than a JSON
-- string, since a PO is structurally a header + child-line document the
-- operator edits and audits per line.
--
-- Status lifecycle: 'draft' (created here). Future BACKLOG #8 slices add
-- the 'received' transition (stock receipt → product_batch + weighted
-- average cost recompute + stock_movement) and accounts-payable
-- ('purchase_payment'); 'cancelled' is reserved so those slices need no
-- new enum migration. Recepción/WAC/AP stay deferred — this migration
-- only models creation.

DEFINE TABLE purchase_order SCHEMAFULL;
DEFINE FIELD tenant       ON purchase_order TYPE record<tenant>;
DEFINE FIELD supplier     ON purchase_order TYPE record<supplier>;
DEFINE FIELD status       ON purchase_order TYPE string DEFAULT 'draft'
    ASSERT $value IN ['draft', 'received', 'cancelled'];
DEFINE FIELD currency     ON purchase_order TYPE string DEFAULT 'CLP';
DEFINE FIELD total        ON purchase_order TYPE decimal DEFAULT 0;
DEFINE FIELD notes        ON purchase_order TYPE option<string>;
DEFINE FIELD external_ref ON purchase_order TYPE option<string>;
DEFINE FIELD created_at   ON purchase_order TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at   ON purchase_order TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX purchase_order_tenant_created  ON purchase_order FIELDS tenant, created_at;
DEFINE INDEX purchase_order_tenant_status   ON purchase_order FIELDS tenant, status, created_at;
DEFINE INDEX purchase_order_tenant_supplier ON purchase_order FIELDS tenant, supplier, created_at;

DEFINE TABLE purchase_order_item SCHEMAFULL;
DEFINE FIELD tenant         ON purchase_order_item TYPE record<tenant>;
DEFINE FIELD purchase_order ON purchase_order_item TYPE record<purchase_order>;
DEFINE FIELD product        ON purchase_order_item TYPE option<record<product>>;
DEFINE FIELD product_name   ON purchase_order_item TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD quantity       ON purchase_order_item TYPE int ASSERT $value > 0;
DEFINE FIELD unit_cost      ON purchase_order_item TYPE decimal;
DEFINE FIELD subtotal       ON purchase_order_item TYPE decimal;
DEFINE FIELD created_at     ON purchase_order_item TYPE datetime DEFAULT time::now();
DEFINE INDEX purchase_order_item_tenant_po      ON purchase_order_item FIELDS tenant, purchase_order;
DEFINE INDEX purchase_order_item_tenant_product ON purchase_order_item FIELDS tenant, product, created_at;

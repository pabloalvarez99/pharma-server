-- pharma-server 0016: accounts payable — payments against a purchase order
-- (Fase 5-full, BACKLOG #8 slice 3 — closes the full PO loop).
--
-- `purchase_payment` records each payment the pharmacy makes against a
-- `purchase_order`. The PO is `paid` once `SUM(purchase_payment.amount) >=
-- purchase_order.total`. Status remains `received` for the stock-side
-- lifecycle (migr 0015); `paid`/`balance` derive from this ledger so a
-- mistaken payment can be reversed without rewriting history.
--
-- Cash payments (`payment_method='cash'`) optionally link to a
-- `cash_register_session` so they're picked up by the drawer's arqueo as
-- an implicit retiro (mirrors the `expense` pattern in migr 0012). Other
-- methods are purely accounting.
--
-- Multi-tenant: every row carries `tenant` and every index includes it.

DEFINE TABLE purchase_payment SCHEMAFULL;
DEFINE FIELD tenant         ON purchase_payment TYPE record<tenant>;
DEFINE FIELD purchase_order ON purchase_payment TYPE record<purchase_order>;
DEFINE FIELD amount         ON purchase_payment TYPE decimal ASSERT $value > 0;
DEFINE FIELD currency       ON purchase_payment TYPE string DEFAULT 'CLP';
DEFINE FIELD payment_method ON purchase_payment TYPE string DEFAULT 'cash'
    ASSERT $value IN ['cash', 'bank', 'card', 'transfer'];
DEFINE FIELD cash_session   ON purchase_payment TYPE option<record<cash_register_session>>;
DEFINE FIELD reference      ON purchase_payment TYPE option<string>;
DEFINE FIELD note           ON purchase_payment TYPE option<string>;
DEFINE FIELD created_by     ON purchase_payment TYPE option<record<user>>;
DEFINE FIELD paid_at        ON purchase_payment TYPE datetime DEFAULT time::now();
DEFINE FIELD created_at     ON purchase_payment TYPE datetime DEFAULT time::now();
DEFINE INDEX purchase_payment_tenant_po       ON purchase_payment FIELDS tenant, purchase_order;
DEFINE INDEX purchase_payment_tenant_paid_at  ON purchase_payment FIELDS tenant, paid_at;
DEFINE INDEX purchase_payment_tenant_session  ON purchase_payment FIELDS tenant, cash_session;

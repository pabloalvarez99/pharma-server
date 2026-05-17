-- pharma-server 0012: gastos + base for sales-by-day reports (Fase 6).
--
-- `expense` is a tenant-scoped operating expense (rent, utilities, supplier
-- invoices, taxes, salaries). When `payment_method='cash'` and a
-- `cash_session` is provided, the expense is included in `arqueo` as an
-- implicit retiro (deducted from the drawer's expected cash). Otherwise
-- the expense is purely accounting (bank, card, transfer).
--
-- Reports (`/api/v1/reports/sales-daily`) are computed on the fly off the
-- existing `order` table — no materialized view yet. The migration adds
-- only the `expense` table; the report endpoint reads `order` directly.

DEFINE TABLE expense SCHEMAFULL;
DEFINE FIELD tenant         ON expense TYPE record<tenant>;
DEFINE FIELD category       ON expense TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD description    ON expense TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD amount         ON expense TYPE decimal ASSERT $value > 0;
DEFINE FIELD payment_method ON expense TYPE string DEFAULT 'cash'
    ASSERT $value IN ['cash', 'bank', 'card', 'transfer'];
DEFINE FIELD cash_session   ON expense TYPE option<record<cash_register_session>>;
DEFINE FIELD supplier       ON expense TYPE option<record<supplier>>;
DEFINE FIELD note           ON expense TYPE option<string>;
DEFINE FIELD created_by     ON expense TYPE option<record<user>>;
DEFINE FIELD incurred_at    ON expense TYPE datetime DEFAULT time::now();
DEFINE FIELD created_at     ON expense TYPE datetime DEFAULT time::now();
DEFINE INDEX expense_tenant_incurred ON expense FIELDS tenant, incurred_at;
DEFINE INDEX expense_tenant_session  ON expense FIELDS tenant, cash_session;
DEFINE INDEX expense_tenant_category ON expense FIELDS tenant, category, incurred_at;

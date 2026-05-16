-- pharma-server 0005: customers + loyalty + prescriptions + pharmacist shifts
-- (Fase 7 subset · erp-parity). Multi-tenant, SCHEMAFULL, append-only.
--
-- Notes:
-- * `customer.rut` uniqueness per tenant is enforced at application layer
--   (service::customers) because Surreal UNIQUE indexes treat repeated NONE
--   values as duplicates and our RUT is optional.
-- * `prescription` is INMUTABLE at application level (Ley 20.000): only CREATE
--   and SELECT are exposed by the API; the schema permits UPDATE so that
--   future controlled corrections require an explicit migration + audit hook.
-- * `loyalty_transaction` is append-only at application level. Writes land in
--   Fase 4 (sales) — Fase-7 subset only reads.

-- ---------------------------------------------------------------------------
-- customer
-- ---------------------------------------------------------------------------
DEFINE TABLE customer SCHEMAFULL;
DEFINE FIELD tenant         ON customer TYPE record<tenant>;
DEFINE FIELD name           ON customer TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD rut            ON customer TYPE option<string>;
DEFINE FIELD phone          ON customer TYPE option<string>;
DEFINE FIELD email          ON customer TYPE option<string>;
DEFINE FIELD loyalty_points ON customer TYPE int DEFAULT 0;
DEFINE FIELD active         ON customer TYPE bool DEFAULT true;
DEFINE FIELD created_at     ON customer TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at     ON customer TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX customer_tenant_rut  ON customer FIELDS tenant, rut;
DEFINE INDEX customer_tenant_name ON customer FIELDS tenant, name;

-- ---------------------------------------------------------------------------
-- loyalty_transaction (append-only at app layer; sales Fase 4 will write)
-- ---------------------------------------------------------------------------
DEFINE TABLE loyalty_transaction SCHEMAFULL;
DEFINE FIELD tenant     ON loyalty_transaction TYPE record<tenant>;
DEFINE FIELD customer   ON loyalty_transaction TYPE record<customer>;
DEFINE FIELD delta      ON loyalty_transaction TYPE int;
DEFINE FIELD reason     ON loyalty_transaction TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD ref        ON loyalty_transaction TYPE option<string>;
DEFINE FIELD created_at ON loyalty_transaction TYPE datetime DEFAULT time::now();
DEFINE INDEX loyalty_tx_tenant_customer ON loyalty_transaction FIELDS tenant, customer, created_at;

-- ---------------------------------------------------------------------------
-- prescription (Ley 20.000; INMUTABLE at app layer — only CREATE / SELECT)
-- ---------------------------------------------------------------------------
DEFINE TABLE prescription SCHEMAFULL;
DEFINE FIELD tenant       ON prescription TYPE record<tenant>;
DEFINE FIELD product      ON prescription TYPE option<record<product>>;
DEFINE FIELD customer     ON prescription TYPE option<record<customer>>;
DEFINE FIELD patient_name ON prescription TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD patient_rut  ON prescription TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD doctor_name  ON prescription TYPE option<string>;
DEFINE FIELD doctor_rut   ON prescription TYPE option<string>;
DEFINE FIELD controlled   ON prescription TYPE bool DEFAULT false;
DEFINE FIELD folio        ON prescription TYPE option<string>;
DEFINE FIELD dispensed_at ON prescription TYPE datetime DEFAULT time::now();
DEFINE FIELD created_at   ON prescription TYPE datetime DEFAULT time::now();
DEFINE INDEX prescription_tenant_rut        ON prescription FIELDS tenant, patient_rut, created_at;
DEFINE INDEX prescription_tenant_controlled ON prescription FIELDS tenant, controlled, created_at;

-- ---------------------------------------------------------------------------
-- pharmacist_shift (químico farmacéutico turno)
-- ---------------------------------------------------------------------------
DEFINE TABLE pharmacist_shift SCHEMAFULL;
DEFINE FIELD tenant     ON pharmacist_shift TYPE record<tenant>;
DEFINE FIELD user       ON pharmacist_shift TYPE record<user>;
DEFINE FIELD started_at ON pharmacist_shift TYPE datetime DEFAULT time::now();
DEFINE FIELD ended_at   ON pharmacist_shift TYPE option<datetime>;
DEFINE FIELD notes      ON pharmacist_shift TYPE option<string>;
DEFINE INDEX pharmacist_shift_tenant_user ON pharmacist_shift FIELDS tenant, user, started_at;

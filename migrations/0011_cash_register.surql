-- pharma-server 0011: cash register (Fase 6 — caja apertura/cierre/arqueo).
--
-- A `cash_register_session` represents a counter shift: opened with the
-- starting cash float (`opening_cash`), closed with the counted physical
-- cash (`closing_cash_counted`). The difference vs. expected close
-- (= opening + cash sales - cash movements out) is the `discrepancia`
-- (cash variance). One open session per (tenant, user) at a time.
--
-- A `cash_movement` is a non-sale cash motion during the session:
--   * `tipo='ingreso'` — extra cash put into the drawer (loan, customer
--     change refill, etc.).
--   * `tipo='retiro'` — cash taken out (vault deposit, mid-shift sweep).
--
-- Sales settle into the session implicitly via `order.payment_method` IN
-- ('pos_cash','pos_mixed') with `cash_amount`. The service layer sums
-- those at close time so we don't have to denormalize the link onto every
-- order row.

DEFINE TABLE cash_register_session SCHEMAFULL;
DEFINE FIELD tenant               ON cash_register_session TYPE record<tenant>;
DEFINE FIELD user                 ON cash_register_session TYPE record<user>;
DEFINE FIELD register_name        ON cash_register_session TYPE string DEFAULT 'caja-1';
DEFINE FIELD opening_cash         ON cash_register_session TYPE decimal;
DEFINE FIELD opening_notes        ON cash_register_session TYPE option<string>;
DEFINE FIELD closing_cash_counted ON cash_register_session TYPE option<decimal>;
DEFINE FIELD closing_cash_expected ON cash_register_session TYPE option<decimal>;
DEFINE FIELD discrepancia         ON cash_register_session TYPE option<decimal>;
DEFINE FIELD closing_notes        ON cash_register_session TYPE option<string>;
DEFINE FIELD opened_at            ON cash_register_session TYPE datetime DEFAULT time::now();
DEFINE FIELD closed_at            ON cash_register_session TYPE option<datetime>;
DEFINE FIELD status               ON cash_register_session TYPE string DEFAULT 'open'
    ASSERT $value IN ['open', 'closed'];
DEFINE INDEX crs_tenant_opened   ON cash_register_session FIELDS tenant, opened_at;
DEFINE INDEX crs_tenant_status   ON cash_register_session FIELDS tenant, status, opened_at;
DEFINE INDEX crs_tenant_user_st  ON cash_register_session FIELDS tenant, user, status;

DEFINE TABLE cash_movement SCHEMAFULL;
DEFINE FIELD tenant      ON cash_movement TYPE record<tenant>;
DEFINE FIELD session     ON cash_movement TYPE record<cash_register_session>;
DEFINE FIELD tipo        ON cash_movement TYPE string
    ASSERT $value IN ['ingreso', 'retiro'];
DEFINE FIELD amount      ON cash_movement TYPE decimal ASSERT $value > 0;
DEFINE FIELD reason      ON cash_movement TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD admin       ON cash_movement TYPE option<record<user>>;
DEFINE FIELD created_at  ON cash_movement TYPE datetime DEFAULT time::now();
DEFINE INDEX cm_session  ON cash_movement FIELDS tenant, session, created_at;

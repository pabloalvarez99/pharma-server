-- pharma-server 0039: customer_ledger — cuenta corriente / FIADO (V1).
--
-- El retail chico chileno vive del "fiado" al cliente habitual. Este ledger es
-- APPEND-ONLY e INMUTABLE: cada movimiento (cargo al fiar una venta, abono al
-- pagar) es una fila que nunca se edita ni borra. El saldo del cliente = suma de
-- cargos - abonos (calculada, nunca almacenada mutando). Multi-tenant (regla #4):
-- `tenant: record<tenant>` + índice compuesto liderado por tenant.
--
-- `kind`:
--   'cargo' → el cliente debe (fió una venta). `amount` > 0. `order` liga la venta.
--   'abono' → el cliente pagó parte/todo. `amount` > 0. `cash_session` opcional
--             (si pagó en efectivo con caja abierta, el ingreso entra al arqueo).
-- El signo lo da `kind`, no el monto (montos siempre positivos) — evita fiaste
-- montos negativos por error.

DEFINE TABLE customer_ledger SCHEMAFULL;
DEFINE FIELD tenant       ON customer_ledger TYPE record<tenant>;
DEFINE FIELD customer     ON customer_ledger TYPE record<customer>;
DEFINE FIELD kind         ON customer_ledger TYPE string
    ASSERT $value IN ["cargo", "abono"];
DEFINE FIELD amount       ON customer_ledger TYPE number
    ASSERT $value > 0;
-- Venta ligada (solo en 'cargo' desde POS fiado). Opcional: un cargo manual
-- (ajuste de saldo inicial migrando del cuaderno) no lleva order.
DEFINE FIELD order        ON customer_ledger TYPE option<record<order>>;
-- Caja donde entró el abono en efectivo (para que salga en el arqueo).
DEFINE FIELD cash_session ON customer_ledger TYPE option<record<cash_register_session>>;
DEFINE FIELD note         ON customer_ledger TYPE option<string>;
DEFINE FIELD created_by   ON customer_ledger TYPE option<record<user>>;
DEFINE FIELD created_at   ON customer_ledger TYPE datetime DEFAULT time::now();

-- Habilitar 'pos_fiado' como método de pago de una venta (el whitelist de
-- 0007 no lo tenía). REDEFINE del campo (append-only: no editamos 0007). Se
-- mantienen todos los valores previos + 'pos_fiado'.
DEFINE FIELD OVERWRITE payment_method ON order TYPE string
    ASSERT $value IN ['pos_cash','pos_debit','pos_credit','pos_mixed','pos_fiado','store','webpay','transfer'];

-- Consulta caliente: estado de cuenta de un cliente, orden cronológico.
DEFINE INDEX cl_tenant_customer ON customer_ledger FIELDS tenant, customer, created_at;
-- Lookup del cargo por venta (idempotencia del fiado — verificada en código, no
-- UNIQUE porque `order` es NONE en abonos/cargos manuales y un UNIQUE sobre NONE
-- rechazaría múltiples abonos).
DEFINE INDEX cl_order ON customer_ledger FIELDS order;

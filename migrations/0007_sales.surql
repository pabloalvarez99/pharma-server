-- pharma-server 0007: sales / POS — orders, order_items, returns, admin_settings,
-- idempotency_key (Fase 4 · erp-parity). Multi-tenant, SCHEMAFULL, append-only.
--
-- Notes:
-- * `order` is INMUTABLE post-create at app layer (except `status` transitions
--   handled by sales service). Cancellation = devolucion with tipo='cancelacion'.
-- * Stock decrement, stock_movement emission, loyalty accumulation and
--   prescription_record creation are ALL done in the same multi-statement
--   SurrealQL BEGIN/COMMIT tx inside `domain::sales::service::post_sale`.
-- * `idempotency_key` enforces POST /pos/sale idempotency per tenant for 24h
--   (TTL purge handled by Fase 8 cron); response cached as JSON.
-- * `admin_setting` is a tenant-scoped key/value store for runtime config
--   (low_stock_threshold, loyalty_points_per_clp, alert_email, etc.).

-- ---------------------------------------------------------------------------
-- order
-- ---------------------------------------------------------------------------
DEFINE TABLE order SCHEMAFULL;
DEFINE FIELD tenant            ON order TYPE record<tenant>;
DEFINE FIELD status            ON order TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending','paid','completed','reserved','cancelled','refunded'];
DEFINE FIELD payment_method    ON order TYPE string
    ASSERT $value IN ['pos_cash','pos_debit','pos_credit','pos_mixed','store','webpay','transfer'];
DEFINE FIELD subtotal          ON order TYPE decimal;
DEFINE FIELD discount          ON order TYPE decimal DEFAULT 0;
DEFINE FIELD total             ON order TYPE decimal;
DEFINE FIELD cash_amount       ON order TYPE option<decimal>;
DEFINE FIELD card_amount       ON order TYPE option<decimal>;
DEFINE FIELD customer          ON order TYPE option<record<customer>>;
DEFINE FIELD customer_name     ON order TYPE option<string>;
DEFINE FIELD customer_phone    ON order TYPE option<string>;
DEFINE FIELD sold_by           ON order TYPE option<record<user>>;
DEFINE FIELD sold_by_name      ON order TYPE option<string>;
DEFINE FIELD notes             ON order TYPE option<string>;
DEFINE FIELD external_ref      ON order TYPE option<string>;
DEFINE FIELD created_at        ON order TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at        ON order TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX order_tenant_created ON order FIELDS tenant, created_at;
DEFINE INDEX order_tenant_status  ON order FIELDS tenant, status, created_at;
DEFINE INDEX order_tenant_customer ON order FIELDS tenant, customer, created_at;
DEFINE INDEX order_tenant_payment ON order FIELDS tenant, payment_method, created_at;

-- ---------------------------------------------------------------------------
-- order_item
-- ---------------------------------------------------------------------------
DEFINE TABLE order_item SCHEMAFULL;
DEFINE FIELD tenant       ON order_item TYPE record<tenant>;
DEFINE FIELD order        ON order_item TYPE record<order>;
DEFINE FIELD product      ON order_item TYPE option<record<product>>;
DEFINE FIELD product_name ON order_item TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD quantity     ON order_item TYPE int ASSERT $value > 0;
DEFINE FIELD unit_price   ON order_item TYPE decimal;
DEFINE FIELD subtotal     ON order_item TYPE decimal;
DEFINE FIELD batch        ON order_item TYPE option<record<product_batch>>;
DEFINE FIELD created_at   ON order_item TYPE datetime DEFAULT time::now();
DEFINE INDEX order_item_tenant_order   ON order_item FIELDS tenant, order;
DEFINE INDEX order_item_tenant_product ON order_item FIELDS tenant, product, created_at;

-- ---------------------------------------------------------------------------
-- devolucion (return / refund / cancellation)
-- ---------------------------------------------------------------------------
DEFINE TABLE devolucion SCHEMAFULL;
DEFINE FIELD tenant           ON devolucion TYPE record<tenant>;
DEFINE FIELD order            ON devolucion TYPE option<record<order>>;
DEFINE FIELD tipo             ON devolucion TYPE string DEFAULT 'venta'
    ASSERT $value IN ['venta','cancelacion','garantia','error'];
DEFINE FIELD motivo           ON devolucion TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD notas            ON devolucion TYPE option<string>;
DEFINE FIELD total_devuelto   ON devolucion TYPE decimal;
DEFINE FIELD metodo_reembolso ON devolucion TYPE option<string>;
DEFINE FIELD procesado_por    ON devolucion TYPE option<record<user>>;
DEFINE FIELD created_at       ON devolucion TYPE datetime DEFAULT time::now();
DEFINE INDEX devolucion_tenant_created ON devolucion FIELDS tenant, created_at;
DEFINE INDEX devolucion_tenant_order   ON devolucion FIELDS tenant, order;

-- ---------------------------------------------------------------------------
-- devolucion_item
-- ---------------------------------------------------------------------------
DEFINE TABLE devolucion_item SCHEMAFULL;
DEFINE FIELD tenant       ON devolucion_item TYPE record<tenant>;
DEFINE FIELD devolucion   ON devolucion_item TYPE record<devolucion>;
DEFINE FIELD product      ON devolucion_item TYPE option<record<product>>;
DEFINE FIELD product_name ON devolucion_item TYPE string;
DEFINE FIELD quantity     ON devolucion_item TYPE int ASSERT $value > 0;
DEFINE FIELD unit_price   ON devolucion_item TYPE decimal;
DEFINE FIELD restock      ON devolucion_item TYPE bool DEFAULT true;
DEFINE FIELD created_at   ON devolucion_item TYPE datetime DEFAULT time::now();
DEFINE INDEX devolucion_item_tenant_dev ON devolucion_item FIELDS tenant, devolucion;

-- ---------------------------------------------------------------------------
-- admin_setting (tenant-scoped key/value runtime config)
-- ---------------------------------------------------------------------------
DEFINE TABLE admin_setting SCHEMAFULL;
DEFINE FIELD tenant     ON admin_setting TYPE record<tenant>;
DEFINE FIELD key        ON admin_setting TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD value      ON admin_setting TYPE string;
DEFINE FIELD updated_at ON admin_setting TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX admin_setting_tenant_key ON admin_setting FIELDS tenant, key UNIQUE;

-- ---------------------------------------------------------------------------
-- idempotency_key (POST /pos/sale dedup, 24h TTL handled by Fase 8 cron)
-- ---------------------------------------------------------------------------
DEFINE TABLE idempotency_key SCHEMAFULL;
DEFINE FIELD tenant         ON idempotency_key TYPE record<tenant>;
DEFINE FIELD key            ON idempotency_key TYPE string ASSERT string::len($value) > 0;
DEFINE FIELD response_json  ON idempotency_key TYPE string;
DEFINE FIELD status_code    ON idempotency_key TYPE int DEFAULT 200;
DEFINE FIELD created_at     ON idempotency_key TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at     ON idempotency_key TYPE datetime;
DEFINE INDEX idempotency_tenant_key ON idempotency_key FIELDS tenant, key UNIQUE;
DEFINE INDEX idempotency_expires    ON idempotency_key FIELDS expires_at;

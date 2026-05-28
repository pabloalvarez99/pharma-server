-- pharma-server 0017: DTE (Documentos Tributarios Electrónicos SII) — Fase 9.1.
--
-- Soporta boleta electrónica (39), factura electrónica (33), nota débito (56),
-- nota crédito (61), guía de despacho (52). El XML firmado vive en `xml_firmado`;
-- el TimbreElectrónico en `timbre`; el track_id de SII en `track_id` cuando
-- existe envío. `order_id` link opcional a la venta POS original (Free tier
-- emite local sin venta link directa también).
--
-- Estados: 'draft' (creado, sin folio) → 'signed' (XML firmado + TED, listo
-- envío local) → 'sent' (POST a maullin/palena hecho, track_id recibido) →
-- 'accepted' (SII OK polling) | 'rejected' (SII rechaza) | 'cancelled'
-- (operador anula pre-envío). Estado lo mueve el módulo `crates/dte` con
-- transacciones SurrealDB; no hay paths que salten.
--
-- `caf` guarda el XML CAF del SII (rango folios autorizados). Folio
-- assignment es atómico: `BEGIN; UPDATE caf SET next_folio = next_folio + 1
-- WHERE ... RETURN BEFORE; COMMIT`. Índice UNIQUE(tenant, tipo_dte,
-- folio_desde) evita re-upload del mismo CAF.
--
-- `cert_digital` guarda el PFX cifrado en bytes (argon2id-derived key con
-- master_key + tenant_id; password también cifrado). Solo decifra en memoria
-- al firmar. Documentado en ADR-0011 §Cert digital encrypt-at-rest.
--
-- Multi-tenant obligatorio: cada tabla lleva `tenant: record<tenant>` y los
-- índices son compuestos con tenant primero.

DEFINE TABLE dte SCHEMAFULL;
DEFINE FIELD tenant                 ON dte TYPE record<tenant>;
DEFINE FIELD tipo                   ON dte TYPE int
    ASSERT $value IN [39, 33, 56, 61, 52];
DEFINE FIELD folio                  ON dte TYPE int ASSERT $value > 0;
DEFINE FIELD fecha_emision          ON dte TYPE datetime DEFAULT time::now();
DEFINE FIELD rut_emisor             ON dte TYPE string
    ASSERT string::len($value) > 0;
DEFINE FIELD rut_receptor           ON dte TYPE string
    ASSERT string::len($value) > 0;
DEFINE FIELD razon_social_receptor  ON dte TYPE string;
DEFINE FIELD monto_neto             ON dte TYPE decimal DEFAULT 0;
DEFINE FIELD iva                    ON dte TYPE decimal DEFAULT 0;
DEFINE FIELD monto_exento           ON dte TYPE decimal DEFAULT 0;
DEFINE FIELD monto_total            ON dte TYPE decimal DEFAULT 0;
DEFINE FIELD items                  ON dte TYPE array DEFAULT [];
DEFINE FIELD estado                 ON dte TYPE string DEFAULT 'draft'
    ASSERT $value IN ['draft', 'signed', 'sent', 'accepted', 'rejected', 'cancelled'];
DEFINE FIELD xml_firmado            ON dte TYPE option<string>;
DEFINE FIELD timbre                 ON dte TYPE option<string>;
DEFINE FIELD track_id               ON dte TYPE option<int>;
DEFINE FIELD sii_glosa              ON dte TYPE option<string>;
DEFINE FIELD order_id               ON dte TYPE option<record<order>>;
DEFINE FIELD created_by             ON dte TYPE option<record<user>>;
DEFINE FIELD created_at             ON dte TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at             ON dte TYPE datetime DEFAULT time::now() VALUE time::now();
DEFINE INDEX dte_tenant_tipo_folio  ON dte FIELDS tenant, tipo, folio UNIQUE;
DEFINE INDEX dte_tenant_estado      ON dte FIELDS tenant, estado, created_at;
DEFINE INDEX dte_tenant_fecha       ON dte FIELDS tenant, fecha_emision;
DEFINE INDEX dte_tenant_track       ON dte FIELDS tenant, track_id;
DEFINE INDEX dte_tenant_order       ON dte FIELDS tenant, order_id;

DEFINE TABLE caf SCHEMAFULL;
DEFINE FIELD tenant              ON caf TYPE record<tenant>;
DEFINE FIELD tipo_dte            ON caf TYPE int
    ASSERT $value IN [39, 33, 56, 61, 52];
DEFINE FIELD folio_desde         ON caf TYPE int ASSERT $value > 0;
DEFINE FIELD folio_hasta         ON caf TYPE int ASSERT $value > 0;
DEFINE FIELD next_folio          ON caf TYPE int ASSERT $value > 0;
DEFINE FIELD fecha_autorizacion  ON caf TYPE datetime;
DEFINE FIELD rut_emisor          ON caf TYPE string;
DEFINE FIELD xml                 ON caf TYPE string;
DEFINE FIELD activo              ON caf TYPE bool DEFAULT true;
DEFINE FIELD created_at          ON caf TYPE datetime DEFAULT time::now();
DEFINE INDEX caf_tenant_tipo_folio  ON caf FIELDS tenant, tipo_dte, folio_desde UNIQUE;
DEFINE INDEX caf_tenant_tipo_activo ON caf FIELDS tenant, tipo_dte, activo;

DEFINE TABLE cert_digital SCHEMAFULL;
DEFINE FIELD tenant              ON cert_digital TYPE record<tenant>;
DEFINE FIELD rut_propietario     ON cert_digital TYPE string;
DEFINE FIELD nombre_propietario  ON cert_digital TYPE option<string>;
DEFINE FIELD pfx_blob            ON cert_digital TYPE bytes;
DEFINE FIELD password_encrypted  ON cert_digital TYPE bytes;
DEFINE FIELD vigencia_desde      ON cert_digital TYPE datetime;
DEFINE FIELD vigencia_hasta      ON cert_digital TYPE datetime;
DEFINE FIELD activo              ON cert_digital TYPE bool DEFAULT true;
DEFINE FIELD created_at          ON cert_digital TYPE datetime DEFAULT time::now();
DEFINE INDEX cert_tenant_activo  ON cert_digital FIELDS tenant, activo;
DEFINE INDEX cert_tenant_rut     ON cert_digital FIELDS tenant, rut_propietario;

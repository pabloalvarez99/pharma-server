-- pharma-server 0023: campos de factura/notas/guía en dte (subtask 9.1.f).
--
-- Los tipos 33/56/61/52 exigen identificación completa del receptor
-- (GiroRecep/DirRecep/CmnaRecep), las notas referencias al documento
-- original (Referencia) y la guía el motivo del traslado (IndTraslado).
-- La 0017 nació con el subset boleta-39; SCHEMAFULL descarta campos no
-- declarados, así que se definen aquí para que persistan.
--
-- `referencias` es FLEXIBLE: array de objetos {tipo_doc_ref, folio_ref,
-- fecha_ref, cod_ref?, razon_ref?} (espejo de `dte::DteReferencia`).

DEFINE FIELD giro_receptor      ON dte TYPE option<string>;
DEFINE FIELD direccion_receptor ON dte TYPE option<string>;
DEFINE FIELD comuna_receptor    ON dte TYPE option<string>;
DEFINE FIELD ind_traslado       ON dte TYPE option<int>;
DEFINE FIELD referencias        ON dte FLEXIBLE TYPE option<array> DEFAULT [];

-- pharma-server 0047: índice de los respaldos cifrados del usuario (ADR-0023).
--
-- Los BYTES del respaldo NO viven acá: viven en el bucket (R2). Esta tabla es
-- sólo el índice — qué sobres tiene cada negocio, cuándo entraron y dónde están
-- dentro del bucket. Nada de lo que hay en esta tabla permite abrir un respaldo.
--
-- ## Por qué el índice está en la base y no se lista el bucket
--
-- Se podría preguntarle al bucket "¿qué objetos tiene este tenant?" con un
-- ListObjects y ahorrarse la tabla. Son dos razones para no hacerlo:
--
--   * Costo. En R2 `ListObjects` es Class A (US$ 4,50 por millón), la misma
--     tarifa que un `PUT`. Listar en cada subida para saber qué rotar duplica
--     exactamente la parte cara de la factura — a 1.000.000 de usuarios
--     subiendo una vez al día son US$ 1.643/año que se convierten en US$ 3.286.
--     `DeleteObject`, en cambio, es gratis: rotar sale cero, saber qué rotar es
--     lo que se paga.
--   * Latencia y verdad. El listado del bucket es eventual; la cuota
--     ("¿cuándo subió la última vez?") tiene que ser exacta o no frena nada.
--
-- ## `retrieval_hash`
--
-- `SHA-256(prueba_retiro)`, donde la prueba se deriva de la tarjeta del
-- cuaderno por un camino separado del de la llave de cifrado (ver
-- `domain::user_backup`). Es lo que deja restaurar desde un teléfono nuevo, sin
-- JWT, sin que el server pueda descifrar nada:
--
--   * de este hash no se llega a la prueba (SHA-256 de una vía), y de la prueba
--     tampoco se llega a la llave del sobre (otra rama de la derivación);
--   * si se filtra esta tabla entera, para llegar a la semilla hay que pasar
--     por el mismo PBKDF2 de 210.000 iteraciones que protege al sobre.
--
-- Es `option<string>`: un cliente viejo que no la manda sigue subiendo igual,
-- sólo que su sobre no se puede rescatar sin sesión.

DEFINE TABLE user_backup SCHEMAFULL;

DEFINE FIELD tenant       ON user_backup TYPE record<tenant>;
-- Id opaco que inventa el server. Es también el nombre del objeto en el bucket.
DEFINE FIELD backup_id    ON user_backup TYPE string
    ASSERT $value != NONE AND string::len($value) > 0;
-- Clave completa dentro del bucket (`<prefix><tenant>/<backup_id>`). Se guarda
-- entera y no se recalcula: si algún día cambia el prefijo de la config, los
-- objetos viejos tienen que seguir encontrándose donde quedaron.
DEFINE FIELD object_key   ON user_backup TYPE string;
DEFINE FIELD format_version      ON user_backup TYPE number;
DEFINE FIELD ciphertext_sha256   ON user_backup TYPE string;
DEFINE FIELD size_bytes          ON user_backup TYPE number;
DEFINE FIELD label        ON user_backup TYPE option<string>;
DEFINE FIELD retrieval_hash ON user_backup TYPE option<string>;
DEFINE FIELD uploaded_at  ON user_backup TYPE datetime DEFAULT time::now();

-- Consulta caliente: los sobres de un negocio, del más nuevo al más viejo.
-- La usan las tres operaciones que importan — listar, decidir qué rotar, y
-- saber cuándo fue la última subida para la cuota de frecuencia.
DEFINE INDEX ub_tenant_fecha ON user_backup FIELDS tenant, uploaded_at;

-- Bajada por id. UNIQUE: dos filas con el mismo `backup_id` serían dos punteros
-- al mismo objeto del bucket, y borrar uno dejaría al otro apuntando a nada.
DEFINE INDEX ub_backup_id ON user_backup FIELDS backup_id UNIQUE;

-- Rescate sin sesión: se entra por el hash de la prueba. No es UNIQUE — el
-- mismo negocio tiene varias versiones y todas comparten la prueba (sale de la
-- misma tarjeta).
DEFINE INDEX ub_retrieval ON user_backup FIELDS retrieval_hash;

-- pharma-server 0048: contador de subidas por día del respaldo (ADR-0023).
--
-- ## Por qué no se cuentan las filas de `user_backup`
--
-- Parece la respuesta obvia: "contá los sobres del tenant de las últimas 24 h".
-- No sirve, y el motivo es que la rotación ya borró la evidencia. `user_backup`
-- guarda como máximo `max_versions_per_tenant` filas (5); la subida número 6
-- del día hace desaparecer la primera. Contar ahí devuelve 5 para siempre, y un
-- tope diario de 6 no se dispararía jamás — una cuota que no frena nada es peor
-- que no tenerla, porque figura en el ADR como si frenara.
--
-- Este contador es la evidencia que la rotación no puede borrar: una fila por
-- negocio y por día, que sólo sube.
--
-- ## Por qué existe el tope diario
--
-- El piso de 900 s entre subidas frena la ráfaga pero **no acota la factura**:
-- deja pasar 96 PUT por día y por tenant, que a 1.000.000 de usuarios son
-- US$ 157.680/año en operaciones Class A. Con el tope en 6/día el peor caso
-- queda en US$ 9.855/año. La app real sube una vez al día.
--
-- ## Costo de la tabla
--
-- Una fila por negocio activo por día, de ~40 bytes, en la base — no en el
-- bucket, así que no paga Class A. La barrida de retención las borra junto con
-- los sobres.

DEFINE TABLE user_backup_uso SCHEMAFULL;

DEFINE FIELD tenant  ON user_backup_uso TYPE record<tenant>;
-- `YYYY-MM-DD` en UTC. String y no datetime a propósito: lo que se necesita es
-- una clave de agrupación exacta, y un datetime invita a comparaciones de rango
-- que acá no hacen falta.
DEFINE FIELD dia     ON user_backup_uso TYPE string;
DEFINE FIELD subidas ON user_backup_uso TYPE number DEFAULT 0;

-- Una sola fila por negocio y día. UNIQUE porque el contador se lee para
-- decidir si entra una subida: dos filas serían dos cuentas parciales, y la
-- cuota dejaría pasar el doble.
DEFINE INDEX ubuso_tenant_dia ON user_backup_uso FIELDS tenant, dia UNIQUE;

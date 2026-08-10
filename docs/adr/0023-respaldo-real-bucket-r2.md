# ADR-0023: El respaldo del feriante vive de verdad — bucket R2, cuotas con números, y restauración desde cero

- **Status**: Accepted
- **Date**: 2026-08-10
- **Deciders**: pabloalvarez99 (founder) + Claude (ejecución)
- **Tags**: backup, infra, costo, seguridad, feria, android
- **Related**: [ADR-0022](./0022-feria-agent-first-identity-backup.md) (el sobre cifrado y la llave del usuario) · [ADR-0005](./0005-core-gratis-no-locked-in.md) (core gratis, sin lock-in)

## Context

ADR-0022 decidió el eje 2 del respaldo: **la llave vive en el usuario**, en algo
que conserva. Eso se construyó completo — sobre `RB1` con PBKDF2 210k y
AES-256-GCM, semilla canónica de 84 bits en 12 palabras o 5 bloques, huella
visual, tarjeta imprimible, y una ceremonia de alta que insiste en escribirla en
el cuaderno y no mandarla por chat.

El eje 1 —**los bytes viven en RutBusiness**— no existía. `user_backup.rs` decía,
literalmente:

> Default: validate upload, return `accepted: false` (bucket not wired).

La app cifraba, subía, el server validaba y **tiraba los bytes**. Toda la
ceremonia estaba montada sobre una llave de rescate que no abría nada. Ése es el
peor tipo de agujero: el que hace que la app se vea confiable mientras no lo es.

Este ADR cierra el eje 1 y responde las seis preguntas que no se pueden dejar sin
número: qué bucket, cuánto cuesta, cómo se cablea sin romper la invariante de
conocimiento cero, qué topes lo acotan, cómo restaura alguien que perdió el
teléfono, y qué pasa con los bytes de quien se va.

## Decision

### 1. Cloudflare R2, y la cuenta que lo justifica

**Cloudflare R2 Standard**, S3-compatible, firmado con SigV4 escrito a mano.

#### Precios (verificados 2026-08-10 en `developers.cloudflare.com/r2/pricing`; GCS tomado el 2026-08-09, la página no re-renderiza para el fetcher)

| | R2 Standard | GCS Standard (single-region) |
|---|---|---|
| Almacenamiento | US$ 0,015 / GB-mes | US$ 0,020 / GB-mes |
| Class A (PUT, LIST) | US$ 4,50 / millón | US$ 5,00 / millón |
| Class B (GET, HEAD) | US$ 0,36 / millón | US$ 0,40 / millón |
| DELETE | **gratis** | gratis |
| Egress | **US$ 0** | US$ 0,12/GB (primer tramo) |
| Free tier | 10 GB-mes · 1M Class A · 10M Class B | 5 GB-mes · 5.000 Class A |

R2 gana **todas** las líneas. No hace falta un argumento fino; pero igual conviene
tener la cuenta, porque la cuenta dice algo que el ranking de precios no dice.

#### Cuánto pesa un respaldo de verdad — medido, no estimado

Todo esto sale de correr el pipeline real (`empaquetarSnapshot` →
`cifrarSobreV1`) sobre datos de un puesto, no de multiplicar a ojo:

| Escenario | Plaintext | **Sobre `RB1`** |
|---|---|---|
| Sábado de feria: 120 boletas, 334 líneas | 65.565 B | **65.795 B** |
| Cola offline al tope (200 boletas) | — | **108.073 B** |
| Negocio entero: 120 productos + 60 fiados + 600 boletas | 341.516 B | ~341.746 B |

- Overhead del formato `RB1`: **230 bytes**. Constante.
- Por boleta: **548 bytes**.
- Un negocio activo genera ~**16 MB/año** de snapshots sin comprimir.
- Deflate **antes** de cifrar comprime **12,7×** (día) y **15,8×** (3.000
  boletas). No está implementado; queda anotado abajo como el próximo apriete
  barato, porque comprimir después de cifrar no sirve para nada.

Planificamos con el sábado de feria (65.795 B) por versión y 5 versiones
retenidas: **328.975 B por usuario**.

#### La factura, a tres escalas

Modelo: 1 subida/día por usuario, 5 versiones retenidas, restauraciones raras.

**R2, aplicando el free tier:**

| Usuarios | Almacenamiento | Class A (PUT) | Egress | **Total/año** |
|---|---|---|---|---|
| 1.000 | 0,33 GB → US$ 0 | 0,37M/año → US$ 0 | US$ 0 | **US$ 0** |
| 100.000 | 32,9 GB → US$ 4,12 | 36,5M/año → US$ 110,25 | US$ 0 | **US$ 114,37** |
| 1.000.000 | 329 GB → US$ 57,42 | 365M/año → US$ 1.588,50 | US$ 0 | **US$ 1.645,92** |

**GCS, mismo modelo (bruto, su free tier es despreciable a esta escala):**

| Usuarios | Almacenamiento | Class A | **Total/año** |
|---|---|---|---|
| 1.000 | US$ 0,08 | US$ 1,83 | **US$ 1,91** |
| 100.000 | US$ 7,90 | US$ 182,50 | **US$ 190,40** |
| 1.000.000 | US$ 78,96 | US$ 1.825,00 | **US$ 1.903,96** |

#### Tres cosas que la cuenta dice y la intuición no

**a) Los PUT son el 96% de la factura, no los bytes.** A 1.000.000 de usuarios:
US$ 1.642,50 de Class A contra US$ 59,22 de almacenamiento (bruto, sin free
tier). Todo el diseño de cuotas se reordenó alrededor de esto: lo que hay que
racionar es *cuántas veces se sube*, no *cuánto pesa*. Y explica por qué el
índice de respaldos vive en SurrealDB y no se resuelve con un `ListObjects`:
listar también es Class A, a la misma tarifa que un PUT, así que preguntarle al
bucket qué rotar **duplicaría exactamente la parte cara**.

**b) El egress no decide esto, y hay que decirlo aunque incomode.** La premisa
razonable era que en un servicio de respaldo el egress es raro pero brutal
cuando pasa, y que el capitán paga, no la feriante. A este tamaño de objeto no
se sostiene: si **todos** los usuarios restauraran el mismo día —1.000.000 ×
328.975 B = 329 GB— GCS cobraría **US$ 39,48**. Una vez. El egress gratis de R2
es una ventaja real pero vale decenas de dólares al año, no miles. Lo que decide
es Class A y el free tier.

Esto cambiaría de golpe si el snapshot creciera 100× (fotos de productos, por
ejemplo). Entonces el egress sí sería el término dominante y R2 ganaría por
goleada en vez de por 14%. Vale la pena tenerlo anotado: la decisión es correcta
hoy y **más** correcta después.

**c) Somos gratis de verdad hasta ~30.000 usuarios.** El free tier de R2 se agota
primero por almacenamiento: 10 GB ÷ 328.975 B = **30.397 usuarios**. (Por Class
A daría 32.873.) Debajo de eso la factura es literalmente cero. En GCS, con
1.000 usuarios ya se paga. Para un producto que promete "gratis para siempre"
en el estado en que está, esa diferencia importa más que el 14% del final.

Y el número que cierra el argumento: a 1.000.000 de usuarios el respaldo cuesta
**US$ 0,0016 por usuario y por año**. El competidor es un cuaderno de mil pesos.

#### Lo demás que pesó

- `wrangler` ya está autenticado en la máquina del founder; `gcloud` no. Es un
  costo real de operación, no una preferencia.
- SigV4 se escribió a mano (`crates/api/src/v1/user_backup/sigv4.rs`): `hmac`,
  `sha2`, `hex`, `base64`, `reqwest` y `chrono` ya estaban en el workspace.
  `aws-sdk-s3` habría traído ~60 crates a un workspace que también compila para
  Android. Fijado contra los vectores publicados de AWS (derivación de llave y
  canonical request) más un pin de regresión verificado contra una
  implementación independiente.

### 2. El bucket cableado sin tocar la invariante ni `AppState`

El server sigue viendo **sólo ciphertext + meta**. No hay llave en el server, no
hay ruta que la pida, y no hay camino de código que descifre.

Eso no es un comentario, es un test: `crates/api/tests/user_backup_zero_knowledge.rs`
cifra un sobre real con un marcador adentro y después barre **todas** las vías de
salida —respuesta de subida, listado, bajada, rescate, los bytes tal como
quedaron en el store, y las filas del índice— buscando el marcador crudo, en
base64 y en hex. Un comentario no se pone rojo; el test sí.

El runtime del respaldo entra por un `Extension` de axum y no por `AppState`.
`AppState` lo construyen a mano dos docenas de tests con literal de struct
—incluidos archivos de otras ramas en vuelo— y Rust no tiene valores por defecto
de campo, así que un campo nuevo los rompe a todos. Con la capa: cero call sites
tocados, y su ausencia degrada exactamente al comportamiento de antes
(`accepted: false`), que es lo que ADR-0005 pide de un módulo opcional.

**Credenciales.** Ninguna key entra al repo, al vault ni a Notion — sólo el
puntero. `config/default.toml` trae los campos vacíos y apunta a
`PHARMA__USER_BACKUP__ACCESS_KEY_ID` y `PHARMA__USER_BACKUP__SECRET_ACCESS_KEY`.
**Ninguna credencial de bucket viaja en el APK**: el teléfono habla con
`pharma-server`, y es el server el que firma contra R2. Un diseño que necesitara
la key del lado del teléfono estaría mal por construcción — se puede extraer de
cualquier APK en diez minutos.

### 3. Cuotas: cuatro topes, y uno de ellos es el único que acota la factura

| Tope | Default | Qué acota |
|---|---|---|
| `max_envelope_bytes` | 4 MiB | El sobre gigante. 38× el techo medido (108 KB). |
| `max_versions_per_tenant` | 5 | Almacenamiento de un tenant **activo**. Rotación: entra la nueva, sale la más vieja. |
| `min_seconds_between_uploads` | 900 | La ráfaga. |
| `max_uploads_per_day` | 6 | **La factura.** |

Los dos últimos parecen redundantes y no lo son, y esto sólo se ve haciendo la
cuenta: **un piso de 900 s deja pasar 96 subidas por día**. A 1.000.000 de
usuarios eso es 35.040M PUT/año = **US$ 157.680/año**. El piso frena la ráfaga y
no acota nada más. Con el tope diario en 6, el peor caso queda en **US$ 9.855/año**
— y la app real sube una vez.

El tope diario no se puede implementar contando filas de `user_backup`: la
rotación ya borró la evidencia (topea en 5 para siempre, así que un tope de 6 no
se dispararía jamás). Hace falta un contador propio, `user_backup_uso`
(migración 0048), una fila por negocio y por día que sólo sube y que la barrida
de retención limpia. Está fijado por
`el_tope_diario_frena_aunque_la_rotacion_ya_borro_las_filas`.

**El orden de los chequeos también es parte de la decisión**, y también está en
un test: forma → tamaño → `guarda()` → frecuencia → **PUT al final**. El PUT es
lo único que cuesta plata, así que un rechazo que ya tocó el bucket es un rechazo
que se paga. `un_sobre_mas_grande_que_el_tope_no_llega_al_bucket` afirma
`store.len() == 0`.

Los 429 no usan el mensaje genérico de `ApiError::rate_limited`. Lo que la dueña
necesita leer es que **su venta no se perdió**, que sigue en el teléfono, y en
cuántos minutos reintentar.

### 4. Restauración desde cero: la prueba de retiro

Teléfono nuevo, app recién instalada, nada adentro. Esa persona tiene la tarjeta
del cuaderno y nada más: no tiene sesión, no tiene el token guardado, y si su
negocio se creó con Google tampoco tiene garantizado el acceso a esa cuenta desde
un aparato nuevo. `GET /api/v1/user-backup/{id}` pide JWT. **Si ésa fuera la
única puerta, el respaldo no existiría igual que antes de este ADR** — sólo se
podría bajar desde el aparato que se perdió, con más código.

Se agrega `POST /api/v1/user-backup/rescue`, **sin JWT**, contra una segunda
prueba derivada del mismo material por una rama separada:

```
semilla       = 84 bits de la tarjeta
salt_retiro   = SHA-256("rutbusiness-retiro:v1:" + slug)[..16]
clave_retiro  = PBKDF2-HMAC-SHA256(semilla, salt_retiro, 210_000, 32)
prueba        = HMAC-SHA256(clave_retiro, "rb1-retiro:v1:" + slug)
el server guarda SHA-256(prueba) — nunca la prueba
```

**El salt tiene que ser determinista**, y no es una concesión: el salt de la
llave de cifrado vive *dentro* del sobre, y para bajar el sobre hay que probar
primero quién sos. Un salt aleatorio sería un huevo dentro de su propia gallina.
Se deriva del slug, que está impreso en la misma tarjeta y no es secreto — lo
secreto son las palabras.

Tres cosas que esto **no** rompe:

1. El server sigue sin poder descifrar: de la prueba no se llega a la llave.
2. Si se filtrara la tabla entera, llegar a la semilla desde `SHA-256(prueba)`
   exige el mismo PBKDF2 de 210k que protege al sobre (~2^101 operaciones).
3. Aunque alguien acertara la prueba, se lleva ciphertext que no puede abrir. La
   confidencialidad nunca dependió de esta puerta.

Defensas de la puerta: devuelve **sólo el sobre más nuevo** (una lista sería un
oráculo de enumeración), contesta **404 uniforme** para todo lo que falla —slug
inexistente, prueba que no calza, negocio sin respaldos—, y tiene un limitador
propio **por slug, no por IP**: el atacante rota IPs gratis, pero no puede
cambiar el slug que quiere abrir.

Del lado de la app, el error de tipeo se detecta **en el teléfono**, con el
número de palabra, antes de salir a la red — porque el 404 del server no
distingue "escribiste mal" de "ese negocio no existe", y mandar el mensaje
equivocado en ese momento es caro.

El caso completo está en `RescatarDesdeCeroTest`: sábado se cobra sin señal y se
sube; domingo, otro teléfono, `ApiFactory` **sin token**, sólo la tarjeta, y el
negocio vuelve. El test afirma que el rescate viajó sin `Authorization`.

### 5. Retención y borrado

- **`retention_days = 400`.** A los 400 días sin que nadie los toque, los bytes
  se van (job diario, 04:20). 400 y no 90 porque un puesto puede estar meses
  parado —invierno, una enfermedad, un viaje— y volver; borrarle el respaldo por
  no haber vendido en tres meses sería castigar exactamente el caso que el
  respaldo cubre. Y no "para siempre" porque un dato que nadie va a volver a
  pedir es costo y riesgo, no servicio.
- **`DELETE /api/v1/user-backup/all`** borra todo, índice y bucket. Tiene que
  vivir en la misma API que los subió: mandar a alguien a escribir un mail para
  que le borren sus datos es no tener borrado.
- Índice primero, objeto después, siempre. Un objeto huérfano cuesta centavos
  hasta la barrida siguiente; una fila viva apuntando a un objeto borrado es una
  app que ofrece un respaldo que no existe.

## Consequences

### Bueno

- El respaldo **existe**. La ceremonia de ADR-0022 dejó de ser decorativa.
- Alguien que pierde el teléfono recupera su negocio con lo que tiene en el
  cuaderno, sin sesión y sin hablar con nadie.
- La invariante de conocimiento cero está fijada por un test que barre todas las
  vías de salida, no por un comentario.
- "Gratis para siempre" tiene un número atrás: US$ 0 hasta ~30.000 usuarios,
  US$ 1.646/año a 1.000.000.
- Cero dependencias nuevas de Rust. `async-trait` ya estaba en el workspace;
  `aes-gcm` entró sólo como dev-dependency del test.

### Malo / lo que se acepta

- **SigV4 propio.** Menos código que un SDK, pero es criptografía escrita a mano.
  Mitigado con vectores publicados de AWS y un pin de regresión.
- **Un PBKDF2 más en la subida** si se quiere el carril de rescate (~2 s en un
  teléfono barato). Se mitiga cacheando el hash: no cambia nunca. Está expuesto
  como `retrievalHashHexPrecalculado`.
- **Sin el hash, el sobre sube pero no se rescata.** Es opcional en el cable para
  no romper apps ya instaladas. Un cliente viejo queda con un respaldo que sólo
  baja desde el teléfono que se va a perder. `PreparacionRespaldo.retrievalHashHex`
  lo expone como `null` explícito, no como silencio.
- **No hay compresión.** Deflate pre-cifrado daría 12,7×–15,8× y bajaría la
  línea de almacenamiento (no la de PUT, que es el 96%). Es el próximo apriete
  barato, y hay que hacerlo antes de tener sobres viejos que no sepan de él.

### Hallazgo colateral, y no menor

Cableando esto apareció un bug **pre-existente** en `CryptoPlataforma.android.kt`.
La implementación pasaba la semilla por `password.decodeToString()` para poder
usar `PBEKeySpec`, que exige `CharArray`. La semilla son **11 bytes crudos**, no
UTF-8: cualquier byte ≥ 0x80 que no formara secuencia válida se convertía en
U+FFFD, y semillas distintas colapsaban en la misma contraseña.

Medido, no estimado: **45 colisiones en 200.000 semillas al azar y 454 en
800.000** — espacio efectivo **~2^29** en vez de 2^84. Un espacio real de 84 bits
predice del orden de 10⁻¹⁵ colisiones a ese tamaño de muestra. Sólo ~0,05% de las
semillas sobrevivía intacta. Eso borraba en silencio el margen de 2^101 que
`ClaveDelNegocio.kt` argumenta explícitamente en su propio docstring.

Corregido: PBKDF2 sobre octetos crudos con `javax.crypto.Mac`. RFC 8018 define la
contraseña como *octet string*; el `CharArray` es una comodidad de la API de Java,
no parte del algoritmo. Fijado por
`dos_semillas_que_solo_difieren_en_bytes_altos_dan_llaves_distintas`.

Por qué vivió tanto: los vectores estándar de PBKDF2 usan contraseñas ASCII
("password"/"salt"), así que **pasaban igual**. El caso que rompía era justamente
el que ningún vector publicado cubre.

Ningún respaldo real se perdió — el server nunca llegó a guardar uno. Pero **un
sobre `RB1` ya escrito localmente por el código viejo no abre con el nuevo**. Si
hubiera APKs en la calle con respaldos locales, hay que decidir la migración
antes de publicar.

## Verificación

- `cargo test -p api -p domain` verde, incluyendo 13 tests nuevos del respaldo
  (4 de conocimiento cero, 9 de cuotas/rotación/retención/borrado).
- `cargo clippy -p api -p domain -p pharma-core --all-targets` sale en 0.
- `:core:testDebugUnitTest` verde: 114 tests, incluidos el vector cruzado
  Kotlin↔Rust↔Python de la prueba de retiro y el E2E de restauración desde cero.
- Migraciones 0047 (índice) y 0048 (contador diario).

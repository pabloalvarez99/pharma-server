package cl.rutbusiness.core.backup

/**
 * La prueba de retiro: lo que deja bajar el respaldo desde un teléfono nuevo,
 * sin sesión (ADR-0023).
 *
 * ## El problema
 *
 * `GET /api/v1/user-backup/{id}` pide JWT. Alguien a quien le robaron el
 * teléfono no tiene JWT, no tiene la app instalada, no tiene nada. Si la única
 * puerta pidiera sesión, el respaldo no existiría: sólo se podría bajar desde
 * el aparato que se perdió.
 *
 * ## Lo que la reemplaza
 *
 * La tarjeta del cuaderno, que la dueña sí conserva. De ese mismo material sale
 * un **segundo** secreto, por un camino separado del de la llave de cifrado:
 *
 * ```
 * semilla       = 84 bits de la tarjeta (12 palabras o 5 bloques, da igual)
 * salt_retiro   = SHA-256("rutbusiness-retiro:v1:" + slug)[..16]
 * clave_retiro  = PBKDF2-HMAC-SHA256(semilla, salt_retiro, 210_000, 32)
 * prueba        = HMAC-SHA256(clave_retiro, "rb1-retiro:v1:" + slug)
 * el server guarda SHA-256(prueba), nunca la prueba
 * ```
 *
 * ## Por qué esto no debilita nada
 *
 * 1. **El server sigue sin poder descifrar.** La prueba sale de otra rama de la
 *    derivación; de la prueba no se llega a la llave AES del sobre.
 * 2. **El margen de fuerza bruta no baja.** Si se filtrara la tabla del server,
 *    llegar a la semilla desde `SHA-256(prueba)` exige el mismo PBKDF2 de 210k
 *    que protege al sobre. Por eso la prueba se estira con PBKDF2 y no con un
 *    hash simple, aunque para frenar adivinanzas online no haría falta.
 * 3. **Aunque alguien acertara la prueba, se lleva ciphertext.** No puede
 *    abrirlo. La confidencialidad nunca dependió de esta puerta.
 *
 * ## Por qué el salt es determinista
 *
 * Porque tiene que serlo. El salt de la llave de cifrado vive **dentro** del
 * sobre, y para bajar el sobre hay que probar primero quién sos. Un salt
 * aleatorio sería un huevo dentro de su propia gallina. Se deriva del slug, que
 * está impreso en la misma tarjeta y no es secreto: lo secreto son las palabras.
 *
 * ## Costo
 *
 * Un PBKDF2 extra, ~2 s en un teléfono de 2015. Es estable para siempre, así
 * que conviene cachearlo (ver [PruebaDeRetiro.hashHex] en la subida). El único
 * momento en que no se puede evitar es en el teléfono nuevo — justo el día en
 * que la persona está esperando de todos modos.
 */
object PruebaDeRetiro {

    /** Parte del formato. Cambiarla invalida toda prueba ya registrada. */
    const val PREFIJO_SALT = "rutbusiness-retiro:v1:"

    /** Etiqueta de dominio del HMAC final. */
    const val PREFIJO_HMAC = "rb1-retiro:v1:"

    /**
     * Slug canónico. Que en el teléfono nuevo alguien escriba `"Puesto-Rosa "`
     * en vez de `"puesto-rosa"` no puede costarle el respaldo.
     */
    fun normalizarSlug(slug: String): String = slug.trim().lowercase()

    /** `SHA-256(prefijo || slug)[..16]`. Mismo cálculo que `domain::user_backup`. */
    fun salt(slug: String): ByteArray {
        val entrada = (PREFIJO_SALT + normalizarSlug(slug)).encodeToByteArray()
        return CryptoPlataforma.sha256(entrada).copyOfRange(0, KDF_SALT_LEN)
    }

    /**
     * Deriva la prueba (32 bytes) desde el material del cuaderno.
     *
     * Devuelve `failure` si el material no decodifica —palabra fuera del
     * vocabulario, bloque mal copiado—, con el mismo criterio que
     * [derivarClaveDeMaterial]: vale más un error concreto acá que un "no se
     * pudo descifrar" diez pantallas después.
     */
    fun derivar(
        material: MaterialRecuperacion,
        tenantSlug: String,
        iterations: Int = KDF_ITERATIONS,
    ): Result<ByteArray> {
        val slug = normalizarSlug(tenantSlug)
        if (slug.isEmpty()) {
            return Result.failure(IllegalArgumentException("tenant_slug vacío"))
        }
        return runCatching {
            val semilla = semillaDeMaterial(material).getOrThrow()
            val claveRetiro = CryptoPlataforma.pbkdf2HmacSha256(
                password = semilla,
                salt = salt(slug),
                iterations = iterations,
                outLen = AES_KEY_LEN,
            )
            CryptoPlataforma.hmacSha256(
                key = claveRetiro,
                message = (PREFIJO_HMAC + slug).encodeToByteArray(),
            )
        }
    }

    /** La prueba en hex, que es como viaja por el cable. */
    fun derivarHex(
        material: MaterialRecuperacion,
        tenantSlug: String,
        iterations: Int = KDF_ITERATIONS,
    ): Result<String> = derivar(material, tenantSlug, iterations).map { bytesToHex(it) }

    /**
     * `SHA-256(prueba)` en hex — lo único que se manda al subir, y lo único que
     * el server guarda.
     *
     * Se manda el hash y no la prueba porque el server no necesita la prueba
     * para nada más que compararla: guardar el original sería darle a quien lea
     * la base el secreto en vez de su huella.
     */
    fun hashHex(prueba: ByteArray): String =
        bytesToHex(CryptoPlataforma.sha256(prueba))
}

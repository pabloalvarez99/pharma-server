package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.VentaEnCola

/**
 * Prepara un snapshot de la cola offline.
 *
 * - Sin material ni clave: solo arma JSON (no sube plaintext).
 * - Con [materialRecuperacion]: PBKDF2 → AES-GCM → sobre v1 listo para
 *   `POST /api/v1/user-backup` (server puede devolver accepted:false sin bucket).
 * - Con [claveAes32] (tests): cifra directo sin KDF.
 */
data class PreparacionRespaldo(
    val snapshot: SnapshotBackupV1,
    /** Bytes del JSON UTF-8 (antes de AEAD). */
    val bytesPlaintext: Int,
    val ventasEnCola: Int,
    val ventasEsperando: Int,
    /**
     * Copy para la dueña. Nunca promete "ya quedó en la nube" si no se
     * subió de verdad al bucket.
     */
    val mensaje: String,
    /** `true` cuando hay algo que valga la pena respaldar (al menos 1 venta). */
    val hayContenido: Boolean,
    /** Sobre cifrado si hubo material/clave; si no, null. */
    val sobre: SobreCifradoV1? = null,
    /**
     * `SHA-256(prueba_retiro)` en hex, si se pudo calcular (hace falta el slug
     * del negocio además del material).
     *
     * Es lo que hay que mandar en la subida para que este sobre después se
     * pueda bajar desde un teléfono nuevo. `null` acá significa: el sobre se
     * sube igual, pero sólo lo va a poder bajar un aparato con sesión — o sea,
     * no el que se compra después de perder el otro. Ver [PruebaDeRetiro].
     */
    val retrievalHashHex: String? = null,
)

/**
 * Arma (y opcionalmente cifra) el snapshot desde la cola.
 */
fun prepararRespaldoDesdeCola(
    tenantId: String,
    cola: List<VentaEnCola>,
    createdAtUnix: Long,
    rubro: String? = null,
    /** @deprecated Usar material + KDF. */
    cifradoListo: Boolean = KDF_LISTO,
    /** Llave AES-256 (32 B) ya derivada. Null = no cifrar salvo material. */
    claveAes32: ByteArray? = null,
    /**
     * Frase o bloques del cuaderno. Si hay ventas, se deriva la llave con
     * PBKDF2 y se cifra. No se guarda ni se manda al server.
     */
    materialRecuperacion: MaterialRecuperacion? = null,
    /**
     * Slug del negocio, el mismo que va impreso en la tarjeta. Con esto (y con
     * material) se calcula la prueba de retiro que habilita la restauración
     * desde un teléfono nuevo.
     */
    tenantSlug: String? = null,
    /**
     * Prueba ya calculada, para no pagar el PBKDF2 dos veces.
     *
     * El hash de retiro no cambia nunca —depende sólo de la semilla y del
     * slug—, así que la app debería calcularlo una vez al crear la cuenta y
     * pasarlo por acá en cada respaldo. Sin esto, cada subida cuesta dos
     * PBKDF2 de 210k: unos 4 s en un teléfono barato, todos los días, para
     * volver a calcular el mismo número.
     */
    retrievalHashHexPrecalculado: String? = null,
): Result<PreparacionRespaldo> {
    val tenant = tenantId.ifBlank { "local" }
    val snap = armarSnapshotDesdeCola(
        tenantId = tenant,
        createdAtUnix = createdAtUnix,
        pendingSales = cola,
        rubro = rubro,
    )
    val bytes = empaquetarSnapshot(snap).getOrElse { return Result.failure(it) }
    val esperando = cola.count { it.esperando }
    val hay = cola.isNotEmpty()

    var sobre: SobreCifradoV1? = null
    if (hay && materialRecuperacion != null) {
        val salt = CryptoPlataforma.randomBytes(KDF_SALT_LEN)
        val key = derivarClaveDeMaterial(materialRecuperacion, salt)
        sobre = cifrarSobreV1(
            key = key,
            plaintext = bytes,
            tenantId = tenant,
            uploadedAtUnix = createdAtUnix,
            salt = salt,
            kdfLabel = KDF_ALG,
        ).getOrElse { return Result.failure(it) }
    } else if (hay && claveAes32 != null) {
        sobre = cifrarSobreV1(
            key = claveAes32,
            plaintext = bytes,
            tenantId = tenant,
            uploadedAtUnix = createdAtUnix,
            kdfLabel = "raw-key",
        ).getOrElse { return Result.failure(it) }
    }

    // La prueba de retiro sale del mismo material, pero por otra rama: no
    // depende del salt del sobre (que vive adentro del sobre) sino del slug,
    // justamente para que se pueda calcular antes de tener nada bajado.
    val hashRetiro = when {
        sobre == null || materialRecuperacion == null -> null
        retrievalHashHexPrecalculado != null -> retrievalHashHexPrecalculado
        tenantSlug.isNullOrBlank() -> null
        else -> PruebaDeRetiro.derivar(materialRecuperacion, tenantSlug)
            .map { PruebaDeRetiro.hashHex(it) }
            .getOrNull()
    }

    val mensaje = when {
        !hay ->
            "No hay ventas en el teléfono para respaldar. " +
                "Cuando cobres sin señal, aparecen acá."
        sobre != null ->
            "Cifrado listo: ${cola.size} venta(s) · sobre ${sobre.envelopeBytes.size} bytes " +
                "(PBKDF2 + AES-GCM). Aún falta el bucket para subirlo. " +
                "La llave del cuaderno sigue siendo tuya."
        cifradoListo ->
            "Escribe las 12 palabras o los 5 bloques de tu tarjeta y toca " +
                "Preparar de nuevo. ${cola.size} venta(s), ${bytes.size} bytes listos."
        else ->
            "Paquete armado: ${cola.size} venta(s) · ${bytes.size} bytes. " +
                "**No se sube nada en claro.**"
    }
    return Result.success(
        PreparacionRespaldo(
            snapshot = snap,
            bytesPlaintext = bytes.size,
            ventasEnCola = cola.size,
            ventasEsperando = esperando,
            mensaje = mensaje,
            hayContenido = hay,
            sobre = sobre,
            retrievalHashHex = hashRetiro,
        ),
    )
}

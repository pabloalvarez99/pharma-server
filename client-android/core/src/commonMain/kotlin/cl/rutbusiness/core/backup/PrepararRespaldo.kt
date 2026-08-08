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

    val mensaje = when {
        !hay ->
            "No hay ventas en el teléfono para respaldar. " +
                "Cuando cobres sin señal, aparecen acá."
        sobre != null ->
            "Cifrado listo: ${cola.size} venta(s) · sobre ${sobre.envelopeBytes.size} bytes " +
                "(PBKDF2 + AES-GCM). Aún falta el bucket para subirlo. " +
                "La llave del cuaderno sigue siendo tuya."
        cifradoListo ->
            "Escribí las 12 palabras o los 8 bloques de tu tarjeta y tocá " +
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
        ),
    )
}

package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.VentaEnCola

/**
 * Prepara un snapshot de la cola offline.
 *
 * Sin [claveAes32]: solo arma el JSON (no sube plaintext).
 * Con [claveAes32]: cifra AES-GCM (sobre v1) y deja el envelope listo para
 * `POST /api/v1/user-backup`. Argon2id (frase → clave) sigue pendiente;
 * la clave de 32 B la provee el caller (tests o KDF futuro).
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
    /** Sobre cifrado si se pasó [claveAes32]; si no, null. */
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
    /**
     * Si true y no hay clave, el mensaje dice "listo para cifrar".
     * Si hay [claveAes32], se cifra de verdad.
     */
    cifradoListo: Boolean = ARGON2ID_LISTO,
    /** Llave AES-256 (32 B). Null = no cifrar todavía. */
    claveAes32: ByteArray? = null,
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
    if (claveAes32 != null && hay) {
        sobre = cifrarSobreV1(
            key = claveAes32,
            plaintext = bytes,
            tenantId = tenant,
            uploadedAtUnix = createdAtUnix,
            // Si Argon2 no está, no mentimos en el header.
            kdfLabel = if (ARGON2ID_LISTO) KDF_ALG else "raw-key",
        ).getOrElse { return Result.failure(it) }
    }

    val mensaje = when {
        !hay ->
            "No hay ventas en el teléfono para respaldar. " +
                "Cuando cobres sin señal, aparecen acá."
        sobre != null ->
            "Cifrado listo: ${cola.size} venta(s) · sobre ${sobre.envelopeBytes.size} bytes " +
                "(AES-GCM). Aún falta el bucket para subirlo. " +
                "La llave del cuaderno sigue siendo tuya."
        cifradoListo ->
            "Listo para cifrar y subir: ${cola.size} venta(s), ${bytes.size} bytes. " +
                "La llave del cuaderno sigue siendo tuya."
        else ->
            "Paquete armado: ${cola.size} venta(s) · ${bytes.size} bytes. " +
                "AES-GCM ya está en el cliente; falta Argon2id (frase→llave) " +
                "y el bucket. **No se sube nada en claro.**"
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

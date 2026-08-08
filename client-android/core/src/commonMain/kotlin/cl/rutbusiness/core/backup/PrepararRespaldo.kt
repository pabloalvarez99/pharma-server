package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.VentaEnCola

/**
 * Prepara un snapshot de la cola offline **sin** cifrar ni subir.
 *
 * El producto no manda plaintext al server (ADR-0022). Esta función deja el
 * paquete listo para AEAD; la UI muestra cuántas ventas y cuántos bytes van
 * a viajar, y un mensaje honesto si el cifrado aún no está cableado.
 */
data class PreparacionRespaldo(
    val snapshot: SnapshotBackupV1,
    /** Bytes del JSON UTF-8 (antes de AEAD). */
    val bytesPlaintext: Int,
    val ventasEnCola: Int,
    val ventasEsperando: Int,
    /**
     * Copy para la dueña. Nunca promete "ya quedó en la nube" si no se cifró
     * y subió de verdad.
     */
    val mensaje: String,
    /** `true` cuando hay algo que valga la pena respaldar (al menos 1 venta). */
    val hayContenido: Boolean,
)

/**
 * Arma y empaqueta el snapshot desde la cola.
 *
 * @return error de validación/empaquetado, o la [PreparacionRespaldo].
 */
fun prepararRespaldoDesdeCola(
    tenantId: String,
    cola: List<VentaEnCola>,
    createdAtUnix: Long,
    rubro: String? = null,
    cifradoListo: Boolean = false,
): Result<PreparacionRespaldo> {
    val snap = armarSnapshotDesdeCola(
        tenantId = tenantId.ifBlank { "local" },
        createdAtUnix = createdAtUnix,
        pendingSales = cola,
        rubro = rubro,
    )
    val bytes = empaquetarSnapshot(snap).getOrElse { return Result.failure(it) }
    val esperando = cola.count { it.esperando }
    val hay = cola.isNotEmpty()
    val mensaje = when {
        !hay ->
            "No hay ventas en el teléfono para respaldar. " +
                "Cuando cobres sin señal, aparecen acá."
        cifradoListo ->
            "Listo para cifrar y subir: ${cola.size} venta(s), ${bytes.size} bytes. " +
                "La llave del cuaderno sigue siendo tuya."
        else ->
            "Paquete armado: ${cola.size} venta(s) · ${bytes.size} bytes. " +
                "El cifrado (Argon2id + AES) y el bucket todavía no están " +
                "cableados: **no se sube nada en claro**. Cuando esté listo, " +
                "usás la misma llave del cuaderno."
    }
    return Result.success(
        PreparacionRespaldo(
            snapshot = snap,
            bytesPlaintext = bytes.size,
            ventasEnCola = cola.size,
            ventasEsperando = esperando,
            mensaje = mensaje,
            hayContenido = hay,
        ),
    )
}

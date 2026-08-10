package cl.rutbusiness.core.backup

import cl.rutbusiness.core.offline.AlmacenDeBloques

/**
 * Último sobre cifrado en el teléfono (ADR-0022).
 *
 * Solo base64 del envelope - **nunca** la frase del cuaderno.
 * Sirve para "Abrir un respaldo" sin pegar el paquete a mano después de
 * "Preparar respaldo" en el mismo aparato.
 */

const val ARCHIVO_ULTIMO_SOBRE: String = "ultimo-respaldo-cifrado.b64"

suspend fun guardarSobreLocal(almacen: AlmacenDeBloques, envelopeBytes: ByteArray) {
    if (envelopeBytes.isEmpty()) return
    almacen.escribir(ARCHIVO_ULTIMO_SOBRE, envelopeToBase64(envelopeBytes))
}

suspend fun leerSobreLocal(almacen: AlmacenDeBloques): ByteArray? {
    val b64 = almacen.leer(ARCHIVO_ULTIMO_SOBRE)?.trim().orEmpty()
    if (b64.isEmpty()) return null
    return try {
        base64ToEnvelope(b64)
    } catch (_: Exception) {
        null
    }
}

suspend fun leerSobreLocalBase64(almacen: AlmacenDeBloques): String? {
    val b64 = almacen.leer(ARCHIVO_ULTIMO_SOBRE)?.trim().orEmpty()
    return b64.ifEmpty { null }
}

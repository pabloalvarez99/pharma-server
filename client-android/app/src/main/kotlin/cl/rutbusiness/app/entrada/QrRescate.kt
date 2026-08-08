package cl.rutbusiness.app.entrada

import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel

/**
 * QR escaneable del payload de rescate (ADR-0022).
 *
 * Solo codifica texto `rutbusiness-rescue:v1:…`. **Nunca** codifica la frase
 * de 12 palabras (esa va escrita en el cuaderno, no en un QR fotografiable).
 *
 * Devuelve una matriz booleana (true = módulo oscuro) lista para Canvas.
 * `null` si el payload no se puede codificar.
 */
fun matrizQrRescate(payload: String): Array<BooleanArray>? {
    val texto = payload.trim()
    if (texto.isEmpty()) return null
    if (!texto.startsWith("rutbusiness-rescue:v1:")) return null
    return try {
        val hints = mapOf(
            EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
            EncodeHintType.MARGIN to 1,
            EncodeHintType.CHARACTER_SET to "UTF-8",
        )
        // Tamaño en módulos lo elige ZXing; pedimos 256 y leemos la BitMatrix.
        val matrix = QRCodeWriter().encode(
            texto,
            BarcodeFormat.QR_CODE,
            256,
            256,
            hints,
        )
        val w = matrix.width
        val h = matrix.height
        Array(h) { y ->
            BooleanArray(w) { x -> matrix.get(x, y) }
        }
    } catch (_: Exception) {
        null
    }
}

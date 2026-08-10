package cl.rutbusiness.app.camara

import android.annotation.SuppressLint
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import cl.rutbusiness.app.ui.scanner.FormatoDeCodigo
import com.google.mlkit.vision.barcode.BarcodeScanner
import com.google.mlkit.vision.barcode.BarcodeScannerOptions
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.common.InputImage

/**
 * Le pasa cada frame de la cámara al detector de ML Kit.
 *
 * Dos cosas que parecen detalle y no lo son:
 *
 * 1. **El [ImageProxy] se cierra siempre**, pase lo que pase. Con
 *    `STRATEGY_KEEP_ONLY_LATEST` la cámara tiene un pozo de dos o tres buffers;
 *    un frame que no se cierra congela el análisis para siempre y el escáner
 *    queda mostrando video sin leer nada. Por eso el `close()` va en el
 *    `addOnCompleteListener`, que corre tanto en éxito como en falla.
 * 2. **No se convierte la imagen a bitmap.** `fromMediaImage` le entrega a ML
 *    Kit el YUV que ya salió del sensor; convertir a RGB serían ~1,8 MB de
 *    basura por frame en un aparato con 1-2 GB.
 */
internal class AnalizadorDeCodigos(
    private val escaner: BarcodeScanner,
    private val alLeer: (String) -> Unit,
) : ImageAnalysis.Analyzer {

    // `imageProxy.image` es experimental en CameraX y el opt-in está declarado
    // en Java, así que no basta el `@OptIn` de Kotlin.
    @SuppressLint("UnsafeOptInUsageError")
    override fun analyze(proxy: ImageProxy) {
        val imagen = proxy.image
        if (imagen == null) {
            proxy.close()
            return
        }

        escaner.process(InputImage.fromMediaImage(imagen, proxy.imageInfo.rotationDegrees))
            .addOnSuccessListener { codigos ->
                // El primero con valor y nada más: si en el encuadre entran dos
                // productos, leer los dos de un frame le cargaría al cliente
                // algo que la cajera no quiso pasar. Uno por pasada.
                codigos.firstNotNullOfOrNull { it.rawValue?.trim()?.takeIf(String::isNotEmpty) }
                    ?.let(alLeer)
            }
            .addOnCompleteListener { proxy.close() }
    }
}

/** Traduce los formatos del dominio a la máscara de bits de ML Kit. */
internal fun opcionesDe(formatos: Set<FormatoDeCodigo>): BarcodeScannerOptions {
    val codigos = formatos.map {
        when (it) {
            FormatoDeCodigo.Ean13 -> Barcode.FORMAT_EAN_13
            FormatoDeCodigo.Ean8 -> Barcode.FORMAT_EAN_8
            FormatoDeCodigo.Code128 -> Barcode.FORMAT_CODE_128
        }
    }
    // `setBarcodeFormats` toma el primero aparte del resto; sin al menos uno,
    // ML Kit habilita TODOS los formatos, que es justo lo que no queremos.
    return BarcodeScannerOptions.Builder()
        .setBarcodeFormats(codigos.first(), *codigos.drop(1).toIntArray())
        .build()
}

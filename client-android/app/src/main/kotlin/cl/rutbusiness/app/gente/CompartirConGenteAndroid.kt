package cl.rutbusiness.app.gente

import android.content.Context
import android.content.Intent
import cl.rutbusiness.app.ui.gente.CompartirConGente

/**
 * Implementación Android de [CompartirConGente]: `ACTION_SEND` de texto plano.
 *
 * Vive acá y no en `ui/` porque es el único lado donde se puede importar
 * `android.*` (ADR-0021, verificado por `FronteraDePlataformaTest`). La pantalla
 * habla con la interfaz y no sabe que esto existe.
 *
 * **No hay WhatsApp Business API ni paquete clavado.** No se pone
 * `setPackage("com.whatsapp")`: el mensaje sale por la app que la dueña elija,
 * desde su propio número. Clavar el paquete rompería el envío en un teléfono sin
 * WhatsApp instalado —con un `ActivityNotFoundException`, no con un aviso— y
 * dejaría afuera a quien usa Telegram, SMS o el chat del banco.
 */
class CompartirConGenteAndroid(private val context: Context) : CompartirConGente {

    override fun compartir(texto: String) {
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "text/plain"
            putExtra(Intent.EXTRA_TEXT, texto)
        }
        // `createChooser` y no el share directo: sin él Android puede mandar
        // solo a la última app usada. Mandarle sin querer el recordatorio de
        // deuda de Don Juan a otra persona es el peor error posible de esta
        // pantalla, y no se puede deshacer.
        val chooser = Intent.createChooser(send, "Mandar por")
        // Se puede lanzar desde un Context que no es Activity.
        chooser.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(chooser)
    }
}

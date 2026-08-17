package cl.rutbusiness.app.entrada

import android.content.Context
import android.content.Intent
import cl.rutbusiness.app.ui.entrada.CompartirTarjeta

/**
 * Implementación Android de [CompartirTarjeta]: `Intent` y diálogo de
 * impresión del sistema.
 *
 * Vive acá y no en `ui/` porque es el único lado donde se puede importar
 * `android.*` (ADR-0021, verificado por `FronteraDePlataformaTest`). La
 * pantalla de la tarjeta habla con la interfaz y no sabe que esto existe.
 */
class CompartirTarjetaAndroid(private val context: Context) : CompartirTarjeta {

    override fun compartirTexto(asunto: String, texto: String) {
        val send = Intent(Intent.ACTION_SEND).apply {
            type = "text/plain"
            putExtra(Intent.EXTRA_SUBJECT, asunto)
            putExtra(Intent.EXTRA_TEXT, texto)
        }
        // `createChooser` y no el share directo: sin él Android puede elegir
        // sola la última app usada, que en un teléfono chileno es WhatsApp. La
        // tarjeta abre el respaldo del negocio; mandarla por chat es regalarla.
        val chooser = Intent.createChooser(send, "Guarda en Notas. No mandes por chat.")
        // La pantalla puede lanzarse desde un Context que no es Activity.
        chooser.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(chooser)
    }

    override fun imprimirHtml(html: String) {
        PaginaRescatePrint.imprimirOGuardarPdf(context, html)
    }
}

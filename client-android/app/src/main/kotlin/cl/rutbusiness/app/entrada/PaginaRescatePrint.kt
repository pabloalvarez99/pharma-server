package cl.rutbusiness.app.entrada

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.print.PrintAttributes
import android.print.PrintManager
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Toast

/**
 * Imprime / guarda como PDF la tarjeta de rescate (ADR-0022).
 *
 * Usa el framework de impresión de Android: el sheet del sistema ofrece
 * "Guardar como PDF" sin librerías extra ni FileProvider. El HTML es de una
 * página A4 ([cl.rutbusiness.core.backup.htmlTarjetaImprimible]).
 *
 * El WebView se retiene un rato para que el adaptador de impresión no se GC
 * a mitad del job.
 */
object PaginaRescatePrint {

    @Volatile
    private var webViewActiva: WebView? = null

    /**
     * Abre el diálogo de impresión del sistema con [html] de una página.
     *
     * [context] preferible Activity (LocalContext en Compose). Si no hay
     * servicio de impresión, muestra un toast y no crashea.
     */
    fun imprimirOGuardarPdf(context: Context, html: String, jobName: String = "Tarjeta de rescate") {
        val appCtx = context.applicationContext
        val printManager = context.getSystemService(Context.PRINT_SERVICE) as? PrintManager
        if (printManager == null) {
            Toast.makeText(
                appCtx,
                "Este teléfono no tiene impresión. Usá copiar o compartir texto.",
                Toast.LENGTH_LONG,
            ).show()
            return
        }
        // WebView exige el hilo principal.
        val main = Handler(Looper.getMainLooper())
        main.post {
            val webView = WebView(appCtx)
            webViewActiva = webView
            webView.settings.apply {
                javaScriptEnabled = false
                // Sin red: el HTML es self-contained (SVG inline).
                blockNetworkLoads = true
                allowFileAccess = false
            }
            webView.webViewClient = object : WebViewClient() {
                override fun onPageFinished(view: WebView, url: String?) {
                    try {
                        val adapter = view.createPrintDocumentAdapter(jobName)
                        val attrs = PrintAttributes.Builder()
                            .setMediaSize(PrintAttributes.MediaSize.ISO_A4)
                            .setColorMode(PrintAttributes.COLOR_MODE_MONOCHROME)
                            .setMinMargins(PrintAttributes.Margins.NO_MARGINS)
                            .build()
                        printManager.print(jobName, adapter, attrs)
                    } catch (e: Exception) {
                        Toast.makeText(
                            appCtx,
                            "No se pudo abrir la impresión: ${e.message ?: "error"}",
                            Toast.LENGTH_LONG,
                        ).show()
                    } finally {
                        // Liberar tras un respiro: el adapter puede seguir leyendo.
                        main.postDelayed({
                            if (webViewActiva === view) {
                                webViewActiva = null
                                view.destroy()
                            }
                        }, 60_000L)
                    }
                }
            }
            webView.loadDataWithBaseURL(
                /* baseUrl = */ null,
                /* data = */ html,
                /* mimeType = */ "text/html",
                /* encoding = */ "UTF-8",
                /* historyUrl = */ null,
            )
        }
    }
}

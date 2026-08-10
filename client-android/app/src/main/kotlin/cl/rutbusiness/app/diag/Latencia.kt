package cl.rutbusiness.app.diag

import android.util.Log
import kotlin.time.TimeMark
import kotlin.time.TimeSource

/**
 * Mide cuánto pasa entre que el dedo toca y el frame que muestra el resultado.
 *
 * Existe porque "se siente rápido" es una opinión y la fluidez es la razón
 * entera de haber dejado el WebView. El número sale por logcat:
 *
 * ```
 * adb logcat -s RbLatencia
 * ```
 *
 * Vive fuera de `ui/` a propósito: es el único archivo del carril que importa
 * `android.*`, y así ningún composable lo hereda.
 */
object Latencia {

    private var marca: TimeMark? = null

    /** Se llama en el handler del toque, antes de mutar el estado. */
    fun marcar() {
        marca = TimeSource.Monotonic.markNow()
    }

    /**
     * Se llama desde `withFrameNanos`, o sea cuando el frame con el cambio ya
     * está por dibujarse. La diferencia es lo que el usuario espera.
     */
    fun cerrar(etiqueta: String) {
        val desde = marca ?: return
        marca = null
        Log.i(TAG, "$etiqueta: ${desde.elapsedNow().inWholeMicroseconds / 1000.0} ms")
    }

    private const val TAG = "RbLatencia"
}

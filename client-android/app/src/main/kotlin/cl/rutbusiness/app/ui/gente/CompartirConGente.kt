package cl.rutbusiness.app.ui.gente

import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf

/**
 * Sacar un texto de la app hacia la gente: el chat que la dueña ya usa.
 *
 * Interfaz y no una llamada directa a `Intent` por la regla de ADR-0021 que
 * verifica `FronteraDePlataformaTest`: nada bajo `ui/` importa el SDK de
 * Android. La implementación con `Intent.ACTION_SEND` vive en
 * `cl.rutbusiness.app.gente`, del otro lado de la frontera, igual que la cámara,
 * la impresora y el dictado.
 *
 * Es un puerto aparte de
 * [cl.rutbusiness.app.ui.entrada.CompartirTarjeta] a propósito, aunque los dos
 * terminen en un `ACTION_SEND`. Aquél rotula el selector *"Guardá en Notas. No
 * mandes por chat"*, porque lo que saca es la tarjeta de rescate y mandarla por
 * chat es regalar el respaldo del negocio. Acá el chat es exactamente el
 * destino. Un solo puerto obligaría a que el rótulo fuera correcto para los dos
 * usos, y no hay rótulo que lo sea.
 */
interface CompartirConGente {
    /** Abre el selector del sistema con [texto] listo para mandar. */
    fun compartir(texto: String)
}

/**
 * `null` cuando no hay plataforma detrás: previews, tests de pantalla, y iOS
 * mientras no exista su implementación.
 *
 * La pantalla que lee `null` **esconde el botón** en vez de dibujar uno que no
 * abre nada. Es el mismo criterio que `LocalIrAlAgente` y `LocalCompartirTarjeta`.
 */
val LocalCompartirConGente = staticCompositionLocalOf<CompartirConGente?> { null }

/**
 * Cómo se manda un texto desde acá, o `null` si desde acá todavía no se puede.
 *
 * Azúcar para que la pantalla no tenga que nombrar el `CompositionLocal` ni la
 * interfaz: pide una función, y si viene `null` no dibuja el botón.
 */
@Composable
fun compartirConGente(): ((String) -> Unit)? {
    val puerto = LocalCompartirConGente.current ?: return null
    return puerto::compartir
}

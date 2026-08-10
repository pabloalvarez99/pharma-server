package cl.rutbusiness.app.ui.entrada

import androidx.compose.runtime.staticCompositionLocalOf

/**
 * Sacar la tarjeta de rescate de la app: compartirla a una nota, imprimirla.
 *
 * Es una interfaz y no una llamada directa a `Intent` por la regla de ADR-0021
 * que verifica `FronteraDePlataformaTest`: **nada bajo `ui/` importa el SDK de
 * Android**, para que el día que se sume iOS esta pantalla compile tal cual.
 * La implementación con `Intent` vive en `cl.rutbusiness.app.entrada`, del otro
 * lado de la frontera, igual que la cámara y la impresora.
 *
 * Nota de producto, no técnica: el chooser se rotula "guardá en Notas, no
 * mandes por chat". La tarjeta abre el respaldo del negocio; mandarla por
 * WhatsApp es entregarla. La app no puede impedirlo, pero sí puede no
 * sugerirlo.
 */
interface CompartirTarjeta {
    /** Abre el selector del sistema para guardar [texto] en una nota. */
    fun compartirTexto(asunto: String, texto: String)

    /** Manda [html] al diálogo de impresión / "Guardar como PDF" del sistema. */
    fun imprimirHtml(html: String)
}

/**
 * `null` cuando no hay plataforma detrás: previews, tests de Robolectric, y
 * iOS mientras no exista su implementación. La pantalla esconde los botones en
 * vez de reventar.
 */
val LocalCompartirTarjeta = staticCompositionLocalOf<CompartirTarjeta?> { null }

package cl.rutbusiness.app.ui.assist

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.staticCompositionLocalOf

/**
 * El micrófono, visto desde la pantalla del agente.
 *
 * Mismo trato que la cámara (`ui/scanner/CamaraDeCodigos`): acá está el
 * contrato y del otro lado —en `cl.rutbusiness.app.voz`— vive todo lo que sabe
 * de `SpeechRecognizer`. `ui/` no importa `android.*`, y eso lo mide
 * `FronteraDePlataformaTest`.
 *
 * El dictado **no reemplaza nada**: ni los chips de ejemplo ni el teclado. Es
 * un camino más al mismo campo, y el teclado sigue siendo el que siempre está.
 */
@Stable
interface SesionDeDictado {

    /** True mientras el reconocedor está escuchando. */
    val escuchando: Boolean

    /**
     * Una línea corta para poner bajo el campo, o `null`.
     *
     * Es la única forma que tiene el dictado de decir que algo no salió, y por
     * eso está acotada a una frase en español que termina en cómo seguir
     * («…puedes escribir igual»). Nunca un toast, nunca un código de error: en
     * un puesto, un cartel rojo que interrumpe asusta más que el problema que
     * describe, y lo que hay que sostener es que la venta se pueda anotar.
     */
    val aviso: String?

    /**
     * Toca el micrófono.
     *
     * Escuchando, corta y manda lo que alcanzó a entender. Quieto, pide el
     * permiso si falta y arranca. Sin permiso o sin reconocedor no pasa nada
     * visible salvo, quizás, un [aviso]: el campo y el teclado siguen enteros.
     */
    fun alternar()

    /** Baja el [aviso]. Lo llama la pantalla apenas la dueña empieza a escribir. */
    fun olvidarAviso()
}

/**
 * El dictado del aparato.
 *
 * [recordarSesion] se llama desde la composición y devuelve el control vivo.
 * [onTexto] recibe **sólo el texto final** —los parciales no entran al campo,
 * porque un texto que se corrige solo mientras se lee es imposible de revisar—
 * y lo hace en el hilo principal.
 */
interface DictadoDeVoz {

    @Composable
    fun recordarSesion(onTexto: (String) -> Unit): SesionDeDictado
}

/**
 * El dictado de este aparato, o `null` si acá no hay ninguno.
 *
 * `null` no es un caso raro que haya que blindar: es lo que ven los tests, un
 * teléfono sin motor de reconocimiento instalado y uno sin micrófono. Con
 * `null` la pantalla del agente simplemente no dibuja el control de voz, y se
 * escribe — que es lo que la app hacía ayer.
 */
val LocalDictadoDeVoz = staticCompositionLocalOf<DictadoDeVoz?> { null }

package cl.rutbusiness.app.ui.assist

/**
 * Copy del control de voz (`BotonDeVoz.kt`), mismo patrón que `CopyPropuesta.kt`
 * y `CopyEscaner.kt`: puro JVM, la pantalla arma el botón, este archivo fija las
 * palabras, y el test las prueba sin Compose.
 *
 * Dos palabras nomás, y no cambian entre feria y retail: el micrófono del
 * puesto se llama igual en cualquier pack. Los avisos de qué salió mal -sin
 * permiso, sin señal, no disponible- no viven acá: son del motor de verdad
 * (`cl.rutbusiness.app.voz.DictadoDeVozAndroid`), la única pieza que sabe que
 * existe un `SpeechRecognizer`.
 */

/** Lo que dice el botón cuando está quieto. */
internal const val ETIQUETA_QUIETO = "Hablar"

/** Y mientras escucha, para que se vea que el micrófono está abierto. */
internal const val ETIQUETA_ESCUCHANDO = "Te escucho"

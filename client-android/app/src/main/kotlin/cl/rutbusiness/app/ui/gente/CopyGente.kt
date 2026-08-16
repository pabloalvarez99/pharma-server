package cl.rutbusiness.app.ui.gente

/**
 * Copy de los recados hacia la gente (chat del día / recordatorio de deuda).
 *
 * Puro y sin Compose: las etiquetas se prueban sin montar la pantalla. El tono
 * es de recado — lo que le dirías a alguien — no de exportar un informe.
 */

/** Pista sobre la nota: sale por el chat que la dueña elija. */
fun pistaDelRecado(): String = "Se manda por el chat que uses"

/**
 * CTA para contar cómo va el día.
 *
 * Verbo de chat, no de "compartir el resumen": eso suena a exportar un tablero.
 * Misma frase en feria y formal: se manda la nota, punto.
 *
 * @param feria lo reciben las pantallas; el verbo no cambia con el rubro.
 */
fun etiquetaCompartirDia(@Suppress("UNUSED_PARAMETER") feria: Boolean): String =
    "Mandar por chat"

/**
 * CTA para el recordatorio de deuda.
 *
 * Con nombre: "Recordarle a Don Juan". Sin nombre: "Mandar por chat".
 * Ninguno dice "compartir el saldo".
 *
 * @param feria lo reciben las pantallas; el verbo no cambia con el rubro.
 * @param nombre opcional; si viene, el botón nombra a la persona.
 */
fun etiquetaCompartirDeuda(
    @Suppress("UNUSED_PARAMETER") feria: Boolean,
    nombre: String = "",
): String {
    val limpio = nombre.trim()
    return if (limpio.isEmpty()) "Mandar por chat" else "Recordarle a $limpio"
}

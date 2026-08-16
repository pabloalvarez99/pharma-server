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
 * En feria y en formal se habla de **contar**, no de "compartir el resumen":
 * eso suena a exportar un tablero.
 */
fun etiquetaCompartirDia(feria: Boolean): String =
    if (feria) "Contar cómo va el día" else "Contarle cómo va el día"

/**
 * CTA para el recordatorio de deuda.
 *
 * Feria: mandar. Formal: recordar. Ninguno dice "compartir el saldo".
 */
fun etiquetaCompartirDeuda(feria: Boolean): String =
    if (feria) "Mandarle lo que debe" else "Recordarle lo que debe"

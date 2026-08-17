package cl.rutbusiness.app.ui.gente

/**
 * Copy de los recados hacia la gente (chat del día / recordatorio de deuda).
 *
 * Puro y sin Compose: las etiquetas se prueban sin montar la pantalla. El tono
 * es de recado — lo que le dirías a alguien — no de exportar un informe.
 */

/**
 * Pista sobre la nota: sale por el chat que la dueña elija.
 *
 * "De siempre" y no "WhatsApp" a propósito: el puerto es un `ACTION_SEND`
 * genérico (ver [CompartirConGente]), el selector puede abrir cualquier chat
 * instalado, y nombrar una marca que después no aparece confunde más de lo
 * que tranquiliza. La frase igual habla directo a la dueña que ya manda
 * mensajes todos los días: es el chat de siempre, no uno nuevo que aprender.
 */
fun pistaDelRecado(): String = "Se manda por tu chat de siempre"

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

/**
 * Todas las cadenas de usuario de este archivo, para el gate de tono.
 *
 * Mismo criterio que `todoCopyFeriaUsuario` en `CopyFiado`: si se agrega copy
 * nuevo acá, sumarlo a esta lista para que el gate lo revise.
 */
internal fun todoCopyGenteUsuario(): List<String> = listOf(
    pistaDelRecado(),
    etiquetaCompartirDia(feria = true),
    etiquetaCompartirDia(feria = false),
    etiquetaCompartirDeuda(feria = true),
    etiquetaCompartirDeuda(feria = false),
    etiquetaCompartirDeuda(feria = true, nombre = "Don Juan"),
    etiquetaCompartirDeuda(feria = false, nombre = "Rosa"),
)

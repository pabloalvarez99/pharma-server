package cl.rutbusiness.app.ui.caja

/**
 * Copy extra del paso «abrir» que no vive en [copyAbrirCaja].
 *
 * El monto / CTA salen de [CopyCaja]. Acá: elegir entre varios puestos o cajas
 * y la nota opcional de apertura (solo retail). Funciones puras para gate JVM.
 *
 * Feria: cuaderno / puesto / mesa. Nunca cajón, caja registradora ni sistema.
 * Retail puede seguir con «caja».
 */

/** Cuando el negocio tiene más de un register físico y hay que elegir. */
internal data class CopyElegirCajaApertura(
    val tituloCard: String,
    val ayuda: String,
)

internal fun copyElegirCajaApertura(feria: Boolean): CopyElegirCajaApertura =
    if (feria) {
        CopyElegirCajaApertura(
            tituloCard = "¿Cuál mesa?",
            ayuda = "Hay más de un puesto o mesa. Elegí en cuál estás: lo que " +
                "vendés se anota ahí, como en el cuaderno.",
        )
    } else {
        CopyElegirCajaApertura(
            tituloCard = "¿Cuál caja?",
            ayuda = "El negocio tiene más de una. Elige en cuál estás parada: lo que " +
                "vendas descuenta del local donde está esa caja.",
        )
    }

/**
 * Nota opcional al abrir — solo se muestra en retail formal (feria no anota
 * apertura: el día arranca en cero sin ritual de cajón).
 */
internal data class CopyNotaApertura(
    val tituloCard: String,
    val etiqueta: String,
    val placeholder: String,
    val ayuda: String,
)

internal fun copyNotaApertura(): CopyNotaApertura =
    CopyNotaApertura(
        tituloCard = "¿Algo que anotar?",
        etiqueta = "Nota de la apertura (opcional)",
        placeholder = "Por ejemplo: quedaron $5.000 de anoche",
        ayuda = "Se guarda con la caja. Sirve para acordarse en el cierre.",
    )

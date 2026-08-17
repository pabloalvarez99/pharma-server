package cl.rutbusiness.app.ui.caja

/**
 * Copy de la tarjeta del motivo al sacar/meter plata a mano.
 *
 * El monto ya sale de [copyMovimientoCaja]. Acá solo el “por qué”: en feria
 * es una línea de cuaderno; en retail sigue el tono de caja.
 */
internal data class CopyMotivoMovimiento(
    val tituloCard: String,
    val etiqueta: String,
    val placeholder: String,
    val ayuda: String,
)

/**
 * Motivo al anotar plata a mano: feria habla de mesa/cuaderno; retail de cierre.
 */
internal fun copyMotivoMovimiento(feria: Boolean, esRetiro: Boolean): CopyMotivoMovimiento =
    if (feria) {
        if (esRetiro) {
            CopyMotivoMovimiento(
                tituloCard = "¿Pa qué lo sacas?",
                etiqueta = "En una línea",
                placeholder = "Le pagué al del pan",
                ayuda = "Como en el cuaderno: una raya con el porqué. " +
                    "Al final del día sabes pa dónde se fue esa plata.",
            )
        } else {
            CopyMotivoMovimiento(
                tituloCard = "¿De dónde viene?",
                etiqueta = "En una línea",
                placeholder = "Traje cambio de mi casa",
                ayuda = "Como en el cuaderno: una raya con el porqué. " +
                    "Al final del día sabes de dónde entró esa plata.",
            )
        }
    } else {
        CopyMotivoMovimiento(
            tituloCard = "¿Por qué?",
            etiqueta = "Motivo",
            placeholder = if (esRetiro) "Le pagué al del pan" else "Traje cambio de mi casa",
            ayuda = "Con esto vas a saber en el cierre qué pasó con esa plata.",
        )
    }

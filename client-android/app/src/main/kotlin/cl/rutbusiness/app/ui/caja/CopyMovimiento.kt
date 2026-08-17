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
 * Motivos de un toque para el campo de arriba (`PasoMovimiento`), sólo en feria.
 *
 * El motivo es obligatorio de verdad: el server (`crates/domain/src/cash_register
 * /service.rs`, `add_movement`) rechaza el movimiento con `reason` vacío, así que
 * sacarlo no es opción. El problema no es que sea obligatorio, es que **escribirlo
 * cuesta**: con las manos ocupadas y un teléfono viejo, tipear "le pagué al que
 * trae la mercadería" es lo que hace que la dueña no anote el retiro — y por eso
 * al cerrar el día "falta" plata que en realidad se gastó.
 *
 * Estas son las razones reales de por qué se saca o se mete plata en un puesto de
 * feria: quedan cortas a propósito para no pedir scroll y para no taparle el
 * campo de escribir a mano a quien prefiere tipear la suya. Vacío en retail: ahí
 * el motivo ya sale del cierre formal (`copyMotivoMovimiento`) y esta ola no tocó
 * ese flujo.
 */
internal fun motivosDeUnToque(feria: Boolean, esRetiro: Boolean): List<String> {
    if (!feria) return emptyList()
    return if (esRetiro) {
        listOf("Almuerzo", "Le pagué la mercadería", "Movilización", "Plata para la casa")
    } else {
        listOf("Vuelto de la casa", "Plata para dar cambio")
    }
}

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

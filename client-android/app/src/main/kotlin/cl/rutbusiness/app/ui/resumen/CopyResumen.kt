package cl.rutbusiness.app.ui.resumen

/**
 * Copy de Resumen según feria vs retail formal (ADR-0022).
 *
 * Extraído para tests unitarios sin montar Compose: feria habla de «venta» y
 * «puesto»; farmacia/minimarket siguen con «boleta» y el computador del negocio.
 */

/**
 * Cuántas ventas/boletas del día, en la línea de soporte bajo la cifra grande.
 *
 * @param conteo pedidos del día que mandó el server (`orders`).
 */
internal fun copyTarjetaConteo(feria: Boolean, conteo: Long): String = when (conteo) {
    0L -> "Todavía no hay ninguna venta."
    1L -> if (feria) "1 venta." else "1 boleta."
    else -> if (feria) "$conteo ventas." else "$conteo boletas."
}

/**
 * Una línea para compartir cómo va el día (chat / WhatsApp).
 *
 * Mismos números que la tarjeta: el mensaje tiene que decir lo que la dueña
 * estaba mirando cuando tocó el botón.
 */
internal fun copyResumenDelDia(feria: Boolean, monto: String, conteo: Long): String =
    if (feria) {
        if (conteo == 1L) "$monto en 1 venta" else "$monto en $conteo ventas"
    } else {
        if (conteo == 1L) "$monto en 1 boleta" else "$monto en $conteo boletas"
    }

/**
 * Explica de dónde sale el monto «en la caja».
 *
 * En feria la tarjeta está oculta hoy, pero el copy se deja listo por si se
 * muestra: nunca «computador del negocio».
 */
internal fun copyEnCajaExplicacion(feria: Boolean, nombreDeCaja: String?): String =
    buildString {
        append("Es lo que debería haber ahora")
        nombreDeCaja?.let { append(" en «$it»") }
        if (feria) {
            append(". Lo calcula el puesto con lo que anotaste hoy.")
        } else {
            append(". Lo calcula el computador del negocio con la apertura, ")
            append("las ventas en efectivo y los movimientos.")
        }
    }

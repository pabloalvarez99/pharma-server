package cl.rutbusiness.app.ui.cobrar

/**
 * Copy del paso Comprobante: la pantalla más repetida del día.
 *
 * Puro JVM: [PasoComprobante] arma la pantalla, este archivo fija las frases.
 * La clienta ya pagó y espera el vuelto con la mano estirada, así que cada
 * frase de acá se lee de reojo, no se estudia. Se habla como en el puesto:
 * "listo, cobrado", "vuelto", "imprimir", "mandar por chat". Nunca "voucher",
 * "ticket" a secas, "DTE", "boleta electrónica", "documento tributario" ni
 * "transacción".
 *
 * Reusa [copyTituloCarrito], [copySinDetalleComprobante] y [copyRefPapel] de
 * `CopyCobrarFeria.kt` (ADR-0022): esa tabla ya resuelve feria vs. retail para
 * el nombre del carrito y el papel, y no hace falta un segundo lugar que diga
 * lo mismo distinto.
 */

/** Etiqueta arriba de la cifra hero cuando hay vuelto que dar. */
internal fun copyEtiquetaVuelto(): String = "Vuelto"

/** Etiqueta arriba de la cifra hero cuando no hubo vuelto (exacto o no efectivo). */
internal fun copyEtiquetaCobrado(): String = "Cobrado"

/** Bajo el vuelto: recuerda el total cuando la cifra grande ya es otra. */
internal fun copyTotalDeLaVenta(total: String): String = "Total de la venta: $total"

/** Título cuando la venta se cobró pero el papel del server no llegó. */
internal fun copyTituloSinDetalle(): String = "La venta quedó registrada"

/**
 * Botón para volver a vender. Igual en ambos packs: después de cobrar el
 * gesto es el mismo, sea feria o retail.
 */
internal fun copyBotonCobrarOtraVenta(): String = "Cobrar otra venta"

/** Puntos de fidelidad sumados en esta venta. Singular sólo con exactamente 1. */
internal fun copyPuntosGanados(puntos: Long): String {
    val unidad = if (puntos == 1L) "punto" else "puntos"
    return "Sumó $puntos $unidad de fidelidad."
}

/** Título del cartel de venta guardada (se cobró sin señal). */
internal fun copyTituloVentaGuardada(): String = "Venta guardada"

/**
 * Cuerpo del cartel de venta guardada: por qué no hay que volver a cobrarla.
 * Cero plata acá (ver [PasoComprobante]): esto es lo único que importa saber
 * en el momento, con los billetes todavía en la mano.
 */
internal fun copyDetalleVentaGuardada(): String =
    "Se va a enviar sola apenas vuelva la señal. No la vuelvas a cobrar: " +
        "aunque se mande de nuevo, no se cobra dos veces."

// Etiquetas de las líneas del detalle: son plata, no cambian entre packs.
internal fun copyEtiquetaSubtotal(): String = "Subtotal"
internal fun copyEtiquetaDescuento(): String = "Descuento"
internal fun copyEtiquetaTotalLinea(): String = "Total"
internal fun copyEtiquetaPagoCon(): String = "Pagó con"
internal fun copyEtiquetaVueltoLinea(): String = "Vuelto"

/**
 * Botón para mandar el papel por chat en vez de (o además de) imprimirlo.
 *
 * En feria muchas veces no hay impresora en el puesto: mandar por chat no es
 * un segundo premio, es la salida de todos los días. Nunca "compartir" —eso
 * suena a exportar un informe, no a mandarle su comprobante a la clienta.
 */
internal fun copyBotonMandarPorChat(): String = "Mandar por chat"

/**
 * Mensaje listo para el chat: el mismo papel, en texto plano.
 *
 * [lineas] llega ya formateada (cantidad, nombre, monto) porque este archivo
 * es JVM puro y no conoce `Moneda`: quien calcula los montos es la pantalla,
 * como en el resto de esta serie de Copy. Sin membrete ni pie legal: eso es
 * cosa del papel impreso, un chat no lo necesita.
 */
internal fun copyMensajeParaCompartir(
    titulo: String,
    referencia: String,
    lineas: List<String>,
    total: String,
    vuelto: String?,
): String = buildString {
    appendLine(titulo)
    appendLine(referencia)
    lineas.forEach { appendLine(it) }
    append(if (vuelto != null) "Total $total · Vuelto $vuelto" else "Total $total")
}

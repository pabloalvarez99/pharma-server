package cl.rutbusiness.app.ui.ventas

/**
 * Copy de "Lo que vendiste" según feria vs retail formal (ADR-0022).
 *
 * Extraído para tests unitarios sin montar Compose, mismo criterio que
 * `CopyMenu.kt` y `CopyResumen.kt`.
 *
 * Nunca en feria: boleta, catálogo, stock, cajón, arqueo, transacción, orden,
 * pedido, SKU, ítem. En feria se dice: lo que vendiste, una venta, lo que
 * llevaba, papel, el día, el puesto.
 *
 * Español de Chile, tuteo chileno. Nunca voseo argentino.
 */

// --- pantalla principal -------------------------------------------------

internal fun tituloHistorial(feria: Boolean): String =
    if (feria) "Lo que vendiste hoy" else "Ventas de hoy"

internal fun subtituloHistorial(feria: Boolean): String =
    if (feria) {
        "Revisa el día y deshace una venta si te equivocaste"
    } else {
        "Revisa las ventas del día y anula una si hubo un error"
    }

/** Entrada del menú del puesto que abre esta pantalla. */
internal fun tituloEntradaHistorial(feria: Boolean): String =
    if (feria) "Lo que vendiste" else "Ventas de hoy"

internal fun subtituloEntradaHistorial(feria: Boolean): String =
    if (feria) "Ve el día y deshace una venta si te equivocaste" else "Ver y anular ventas del día"

// --- fila de una venta ----------------------------------------------------

internal fun etiquetaComoPago(metodo: String, feria: Boolean): String = when (metodo) {
    "cash" -> "Efectivo"
    "card" -> "Tarjeta"
    "transfer" -> "Transferencia"
    "credit" -> if (feria) "Fiado" else "Crédito"
    else -> metodo
}

internal fun etiquetaVentaDevuelta(feria: Boolean): String =
    if (feria) "Deshecha" else "Devuelta"

// --- vacío del día ----------------------------------------------------------

internal fun tituloVacioHistorial(feria: Boolean): String =
    if (feria) "Todavía no vendes nada hoy" else "Todavía no hay ventas hoy"

internal fun beneficioVacioHistorial(feria: Boolean): String =
    if (feria) {
        "Cuando vendas algo, aparece acá para que puedas revisarlo o deshacerlo si te equivocaste."
    } else {
        "Cada venta que hagas aparece acá, para revisarla o anularla si hubo un error."
    }

internal fun pistaVacioHistorial(feria: Boolean): String =
    if (feria) "Vende algo y vuelve a mirar." else "Registra una venta y vuelve a mirar."

// --- error de carga ----------------------------------------------------------

internal fun tituloErrorHistorial(feria: Boolean): String =
    if (feria) "No pudimos traer lo que vendiste" else "No pudimos traer las ventas"

// --- detalle de una venta ----------------------------------------------------

internal fun tituloDetalleVenta(feria: Boolean): String =
    if (feria) "Lo que llevó" else "Detalle de la venta"

internal fun subtituloDetalleVenta(): String = "Qué se llevó y cómo pagó"

internal fun tituloQueLlevaba(feria: Boolean): String =
    if (feria) "Lo que llevaba" else "Lo que compró"

internal fun tituloComoPago(): String = "Cómo pagó"

internal fun tituloCuandoFue(): String = "Cuándo fue"

// --- botón y confirmación de deshacer ----------------------------------------

internal fun ctaDeshacerVenta(feria: Boolean): String =
    if (feria) "Me equivoqué" else "Devolver esta venta"

internal fun ctaDeshaciendo(feria: Boolean): String =
    if (feria) "Deshaciendo..." else "Devolviendo..."

internal fun tituloConfirmarDeshacer(feria: Boolean): String =
    if (feria) "¿Deshacer esta venta?" else "¿Devolver esta venta?"

/**
 * Qué va a pasar en plata y en mercadería — no "¿estás seguro?". [montoFormateado]
 * ya viene formateado con la moneda del negocio, no se recalcula acá.
 */
internal fun mensajeConfirmarDeshacer(feria: Boolean, montoFormateado: String): String =
    if (feria) {
        "Se deshace la venta completa por $montoFormateado y las cosas vuelven a lo que tienes."
    } else {
        "Se anula la venta completa por $montoFormateado y los productos vuelven al stock."
    }

internal fun botonConfirmarDeshacer(feria: Boolean): String =
    if (feria) "Sí, deshacer" else "Sí, devolver"

internal fun botonCancelarDeshacer(): String = "No, dejarla como está"

/** Motivo que se manda al server; el server exige que no vaya vacío. */
internal fun motivoDeDevolucion(feria: Boolean): String =
    if (feria) "La feriante deshizo la venta desde la app" else "Devolución anulada desde la app"

// --- resultado de deshacer ----------------------------------------------------

internal fun avisoDeshecha(feria: Boolean): String =
    if (feria) "Listo, quedó deshecha." else "Listo, quedó devuelta."

internal fun tituloErrorDeshacer(feria: Boolean): String =
    if (feria) "No se pudo deshacer" else "No se pudo devolver"

internal fun mensajeSinPermisoDeshacer(feria: Boolean): String =
    if (feria) {
        "Deshacer una venta lo hace quien administra el puesto. Pídeselo a esa persona."
    } else {
        "Devolver una venta lo hace quien administra el negocio. Pídeselo a esa persona."
    }

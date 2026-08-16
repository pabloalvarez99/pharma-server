package cl.rutbusiness.app.ui.cobrar

/**
 * Copy de Cobrar / Vender según pack (ADR-0022).
 *
 * Extraído de las pantallas para tests unitarios sin montar ViewModel ni
 * cámara: feria habla de nombre/atado/puesto; retail formal sigue con
 * código de barras y "producto".
 */

internal data class CopyBuscarCobrar(
    val etiqueta: String,
    val placeholder: String,
    val ayudaOnline: String,
)

internal fun copyBuscarCobrar(barcode: Boolean): CopyBuscarCobrar =
    if (barcode) {
        CopyBuscarCobrar(
            etiqueta = "Buscar producto",
            placeholder = "Nombre o código de barras",
            // Una línea calmada: no es un tutorial, es un campo de búsqueda.
            ayudaOnline = "Nombre o código de barras.",
        )
    } else {
        CopyBuscarCobrar(
            etiqueta = "¿Qué vendiste?",
            placeholder = "Tomate, cilantro…",
            ayudaOnline = "Escribí el nombre (tomate, atado, bolsa…).",
        )
    }

/** Subtítulo del paso buscar: mesa del puesto, no panel de POS. */
internal fun copySubtituloBuscar(feria: Boolean): String =
    if (feria) "Anotá lo que se lleva" else "Busca el producto y agrégalo"

/**
 * Resumen de la barra inferior del carrito.
 *
 * Feria habla de "cosas"; retail conserva "productos". Sin total (offline) se
 * dice con palabras, no con un número inventado en el teléfono.
 */
internal fun copyBarraCarrito(unidades: Int, total: String?, feria: Boolean): String {
    if (unidades <= 0) {
        return if (feria) "Nada en la venta todavía" else "Sin productos todavía"
    }
    val cosa = if (feria) {
        if (unidades == 1) "1 cosa" else "$unidades cosas"
    } else {
        if (unidades == 1) "1 producto" else "$unidades productos"
    }
    val monto = total ?: "el sistema confirma el total al cobrar"
    return "$cosa · $monto"
}

/** Título de la tarjeta del carrito en el paso de pago. */
internal fun copyTituloCarrito(feria: Boolean): String =
    if (feria) "Lo que se lleva" else "Lo que lleva"

/** Aviso offline en el paso de pago: feria sin "sistema del negocio". */
internal fun copyOfflinePago(feria: Boolean): String =
    if (feria) {
        "Sin señal. La venta queda en el teléfono y se anota sola cuando vuelva " +
            "la red. El total se confirma al enviarse."
    } else {
        "Sin conexión con el sistema del negocio. La venta se guarda en el " +
            "teléfono y se envía sola cuando vuelva la señal. El total se confirma " +
            "cuando se envíe."
    }

/** `true` si la UI debe ofrecer el botón de cámara. */
internal fun escanerVisible(barcode: Boolean, hayCamara: Boolean): Boolean =
    barcode && hayCamara

/**
 * Pista cuando la búsqueda no trae filas.
 *
 * - Consulta en blanco: el caller sigue usando [pistaSinCatalogo] del catálogo.
 * - Feria con texto: enseña agregar la cosa o decírselo al agente con precio.
 * - Retail: el copy histórico exacto (revisar escritura / acortar / agregar).
 */
internal fun pistaBusquedaVacia(
    feria: Boolean,
    consulta: String,
    puedeCargar: Boolean,
): String {
    val q = consulta.trim()
    if (feria && q.isNotEmpty()) {
        val fraseAgente = "«vendí $q a 2000»"
        return if (puedeCargar) {
            "No está en lo que vendes. Tocá «Agregar una cosa» o decile al agente: $fraseAgente."
        } else {
            "Decile al agente: $fraseAgente."
        }
    }
    return if (puedeCargar) {
        "Revisa cómo se escribe, prueba con una palabra más corta, " +
            "o agrégalo si todavía no está cargado."
    } else {
        "Revisa cómo se escribe, o prueba con una palabra más corta."
    }
}

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
            ayudaOnline = "Escribe el nombre (tomate, atado, bolsa…).",
        )
    }

/** Subtítulo del paso buscar: mesa del puesto, no panel de POS. */
internal fun copySubtituloBuscar(feria: Boolean): String =
    if (feria) "Anota lo que se lleva" else "Busca el producto y agrégalo"

/**
 * Resumen de la barra inferior del carrito.
 *
 * Feria habla de "cosas"; retail conserva "productos". Sin total (offline o
 * el server no mandó número) se dice con palabras: el teléfono no inventa
 * un monto. Feria no nombra "sistema" — el cobro confirma el total.
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
    val monto = when {
        total != null -> total
        // Mesa de feria: palabras honestas, sin "sistema" ni cifra inventada.
        feria -> "se confirma al cobrar"
        else -> "el sistema confirma el total al cobrar"
    }
    return "$cosa · $monto"
}

/**
 * Ayuda del buscador cuando el listado viene del teléfono (sin red).
 *
 * Feria no habla de "stock" de góndola: solo de cuándo se guardó.
 */
internal fun copyAyudaCatalogoGuardado(feria: Boolean, antiguedad: String): String =
    if (feria) {
        "Guardado $antiguedad. Puede no estar al día."
    } else {
        "Guardado $antiguedad. El stock puede haber cambiado."
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

/**
 * Pie del cobro rápido: feria habla del puesto; retail de la caja.
 *
 * La venta suelta entra al mismo ledger en ambos casos; lo que cambia es cómo
 * se nombra el lugar del día.
 */
internal fun copyNotaMontoSuelto(feria: Boolean): String =
    if (feria) {
        "Queda anotada como \"Venta suelta\" en tu día y en el puesto."
    } else {
        "Queda anotada como \"Venta suelta\" en tu día y en la caja."
    }

/**
 * Cuando el cobro pasó pero el detalle del papel no llegó.
 *
 * Feria no nombra "sistema del negocio": el cobro ya está y «Tu día» lo cuenta.
 */
internal fun copySinDetalleComprobante(feria: Boolean): String =
    if (feria) {
        "No alcanzamos a traer el detalle del papel, pero el cobro ya está hecho. " +
            "Lo ves en «Tu día», que ya cuenta esta venta."
    } else {
        "No alcanzamos a traer el detalle del comprobante, pero el cobro ya " +
            "está hecho. Lo ves en «Tu día», que ya cuenta esta venta."
    }

/**
 * Pie de la venta encolada offline: sin total porque el server no la confirmó.
 */
internal fun copyPieVentaEncolada(feria: Boolean, unidades: Int): String {
    val unidadesTxt = if (unidades == 1) "1 unidad" else "$unidades unidades"
    return if (feria) {
        "$unidadesTxt · el total se confirma cuando vuelva la señal"
    } else {
        "$unidadesTxt · el total lo confirma el sistema cuando reciba la venta"
    }
}

/**
 * Referencia quieta del papel — no "ticket fiscal" con la palabra Comprobante.
 *
 * El folio completo a veces es un UUID; basta un tramo corto para reconocerlo.
 */
internal fun copyRefPapel(folio: String): String {
    val corto = folio.trim().take(8)
    return if (corto.isEmpty()) "Sin número" else "Nº $corto"
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
            "No está en lo que vendes. Toca «Agregar una cosa» o dile al agente: $fraseAgente."
        } else {
            "Dile al agente: $fraseAgente."
        }
    }
    return if (puedeCargar) {
        "Revisa cómo se escribe, prueba con una palabra más corta, " +
            "o agrégalo si todavía no está cargado."
    } else {
        "Revisa cómo se escribe, o prueba con una palabra más corta."
    }
}

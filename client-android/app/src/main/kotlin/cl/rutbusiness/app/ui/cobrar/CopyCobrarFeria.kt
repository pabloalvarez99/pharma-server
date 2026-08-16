package cl.rutbusiness.app.ui.cobrar

/**
 * Copy de Cobrar / Vender según [barcode] del pack (ADR-0022).
 *
 * Extraído de [PasoBuscar] para tests unitarios sin montar el ViewModel ni
 * la cámara: feria (`barcode=false`) habla de nombre/atado; retail formal
 * sigue con código de barras.
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
            ayudaOnline = "Escribe parte del nombre, o el código de barras completo.",
        )
    } else {
        CopyBuscarCobrar(
            etiqueta = "¿Qué vendiste?",
            placeholder = "Nombre (tomate, cilantro…)",
            ayudaOnline = "Escribe el nombre de lo que vendes (tomate, atado, bolsa…).",
        )
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

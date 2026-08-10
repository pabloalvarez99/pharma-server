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

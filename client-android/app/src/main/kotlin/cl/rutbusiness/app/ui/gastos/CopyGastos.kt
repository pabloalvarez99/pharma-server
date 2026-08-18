package cl.rutbusiness.app.ui.gastos

/**
 * Copy de "Lo que gastaste" según feria vs retail formal (ADR-0022).
 *
 * Extraído para tests unitarios sin montar Compose, mismo criterio que
 * `CopyVentas.kt` y `CopyMenu.kt`.
 *
 * Nunca en feria: stock, catálogo, boleta, cajón, arqueo, SKU, ítem,
 * transacción, proveedor, egreso, comprobante. Se dice: lo que gastaste, lo
 * que salió, la plata que saliste, el puesto, el flete, las bolsas. "Gasto"
 * sí se puede usar — es palabra normal, no jerga contable.
 */
object CopyGastos {

    // --- pantalla principal ---------------------------------------------------

    fun titulo(feria: Boolean): String = if (feria) "Lo que gastaste" else "Gastos"

    fun subtitulo(feria: Boolean): String =
        if (feria) {
            "Para saber cuánto te queda de verdad, no sólo cuánto vendiste"
        } else {
            "Para saber cuánto te queda de verdad, no sólo cuánto vendiste"
        }

    // --- rango: hoy / mes -------------------------------------------------------

    fun chipHoy(): String = "Hoy"

    fun chipEsteMes(): String = "Este mes"

    // --- fila de un gasto ---------------------------------------------------

    /** Descripción + monto + cuándo, en ese orden. Sin categoría en la fila: va en el detalle. */
    fun cuando(feria: Boolean, fechaCorta: String): String =
        if (feria) "Gastado el $fechaCorta" else "Registrado el $fechaCorta"

    // --- vacío ---------------------------------------------------------------

    fun tituloVacio(feria: Boolean): String =
        if (feria) "Todavía no anotas nada" else "No hay gastos en este rango"

    fun beneficioVacio(feria: Boolean): String =
        if (feria) {
            "Anotando lo que gastas sabes cuánto te queda de verdad, no sólo cuánto vendiste"
        } else {
            "Registra los gastos del negocio para ver la ganancia real, no sólo las ventas"
        }

    fun pistaVacio(feria: Boolean): String =
        if (feria) {
            "Toca \"Anotar algo que gastaste\" y escribe cuánto, en qué, y con qué pagaste"
        } else {
            "Toca \"Anotar gasto\" y escribe cuánto, en qué, y con qué pagaste"
        }

    // --- error ---------------------------------------------------------------

    fun tituloError(feria: Boolean): String =
        if (feria) "No pudimos ver lo que gastaste" else "No pudimos cargar los gastos"

    fun mensajeSinPermiso(feria: Boolean): String =
        if (feria) {
            "No puedes ver los gastos del puesto. Pídeselo a quien lleva el negocio."
        } else {
            "No puedes ver los gastos del negocio. Pídeselo a quien administra la cuenta."
        }

    // --- botón de anotar ------------------------------------------------------

    fun ctaAnotar(feria: Boolean): String =
        if (feria) "Anotar algo que gastaste" else "Anotar gasto"

    fun ctaAnotando(feria: Boolean): String =
        if (feria) "Anotando..." else "Guardando..."

    // --- formulario ---------------------------------------------------------

    fun tituloFormulario(feria: Boolean): String =
        if (feria) "¿En qué gastaste?" else "Nuevo gasto"

    fun labelMonto(): String = "Cuánto"

    fun labelCategoria(feria: Boolean): String =
        if (feria) "En qué" else "Categoría"

    fun placeholderCategoria(): String = "Ej: arriendo del puesto"

    fun labelDescripcion(): String = "Descripción"

    fun placeholderDescripcion(feria: Boolean): String =
        if (feria) "Ej: arriendo del puesto de hoy" else "Ej: arriendo de local, agosto"

    fun tituloComoPagaste(): String = "Con qué pagaste"

    fun chipEfectivo(): String = "Efectivo"

    fun chipTransferencia(): String = "Transferencia"

    fun chipTarjeta(): String = "Tarjeta"

    fun botonCancelar(): String = "Volver"

    // --- categorías sugeridas (chips de cara a la dueña) ------------------------
    // El valor de protocolo (lo que se manda en `category`) es la palabra corta en
    // minúscula, listado aparte en [CATEGORIAS_SUGERIDAS] — la copia y el valor
    // de protocolo son dos cosas distintas a propósito.

    fun etiquetaArriendo(): String = "Arriendo del puesto"
    fun etiquetaFlete(): String = "Flete"
    fun etiquetaBolsas(): String = "Bolsas"
    fun etiquetaMercaderia(): String = "Mercadería"
    fun etiquetaComida(): String = "Comida"
    fun etiquetaOtros(): String = "Otros"

    val CATEGORIAS_SUGERIDAS: List<Pair<String, String>> = listOf(
        etiquetaArriendo() to "arriendo",
        etiquetaFlete() to "flete",
        etiquetaBolsas() to "bolsas",
        etiquetaMercaderia() to "mercadería",
        etiquetaComida() to "comida",
        etiquetaOtros() to "otros",
    )

    // --- éxito y error del formulario -------------------------------------------

    fun avisoGuardado(feria: Boolean): String =
        if (feria) "Anotado. Así sabes cuánto te queda de verdad." else "Gasto registrado."

    fun tituloErrorAnotar(feria: Boolean): String =
        if (feria) "No se pudo anotar" else "No se pudo guardar el gasto"

    /** El mensaje del server se muestra tal cual cuando lo hay; esto es el genérico. */
    fun mensajeErrorGenerico(feria: Boolean): String =
        if (feria) {
            "No pudimos anotar lo que gastaste. Prueba de nuevo."
        } else {
            "No pudimos registrar el gasto. Prueba de nuevo."
        }
}

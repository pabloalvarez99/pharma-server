package cl.rutbusiness.app.ui.offline

/**
 * Copy de la cola de ventas offline (ADR-0022).
 *
 * Extraído para tests unitarios sin montar Compose: feria no habla del
 * "sistema del negocio" (PC on-prem); el APK de feria anota en la nube / RutAgent.
 *
 * El tono es de **recado** (sin conexión · ventas esperando), no de error de
 * red ni de panel de sync. La venta rechazada quedó en el teléfono: se dice
 * eso, no "sistema".
 */
internal fun hintColaVacia(feria: Boolean): String =
    if (feria) {
        "Todas las ventas que cobraste ya se anotaron."
    } else {
        "Todas las ventas que cobraste ya llegaron al sistema del negocio."
    }

/**
 * Recado de la franja cuando no hay señal y no hay cola.
 * Callado a propósito: no es un error, es un aviso de estado.
 */
internal fun recadoSinConexion(): String = "Sin conexión"

/** Segunda línea del recado sin señal y sin cola. */
internal fun detalleSinConexion(): String = "Ves lo último que se cargó."

/**
 * Fallback si el server rechazó la venta y no mandó motivo legible.
 *
 * Feria: sin "sistema". Retail puede nombrar el sistema del negocio.
 */
internal fun motivoRechazoSinDetalle(feria: Boolean): String =
    if (feria) {
        "No se pudo anotar en tu día."
    } else {
        "El sistema no la aceptó."
    }

/** Chip de venta rechazada: se lee como cuaderno, no como error de sync. */
internal fun etiquetaNoSeAnoto(): String = "No se anotó"

/** CTA para sacar de la cola una venta que ya no se reintenta. */
internal fun etiquetaDescartar(): String = "Descartar"

/**
 * Ayuda bajo el rechazo: no reintenta sola; la dueña decide.
 *
 * Feria subraya que la venta quedó en el teléfono (no en un PC del negocio).
 */
internal fun ayudaVentaRechazada(feria: Boolean): String =
    if (feria) {
        "Quedó en el teléfono y no se va a reintentar sola. Revisa qué pasó y " +
            "vuelve a cobrarla si corresponde; recién ahí descártala de acá."
    } else {
        "No se va a reintentar sola. Revisa qué pasó y vuelve a cobrarla si " +
            "corresponde; recién ahí descártala de acá."
    }

/**
 * Pie de la fila: productos · unidades · total (sin inventar montos).
 *
 * Feria no nombra "sistema": el total se confirma al anotar, no al "recibirla"
 * un PC del negocio.
 */
internal fun detalleLineasCola(
    lineas: Int,
    unidades: Int,
    feria: Boolean,
): String {
    val productos = if (lineas == 1) "1 producto" else "$lineas productos"
    val piezas = if (unidades == 1) "1 unidad" else "$unidades unidades"
    val total = if (feria) {
        "el total se confirma cuando se anote"
    } else {
        "el total lo confirma el sistema al recibirla"
    }
    return "$productos · $piezas · $total"
}

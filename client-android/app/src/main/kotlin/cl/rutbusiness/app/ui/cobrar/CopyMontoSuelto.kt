package cl.rutbusiness.app.ui.cobrar

/**
 * Copy del cobro rápido / venta suelta (monto sin catálogo).
 *
 * Solo lo usa [PasoMontoSuelto]. Puro JVM para tests sin Compose: feria
 * habla de mesa ("¿Cuánto es?", "Cobrar", "Anotando…"); retail conserva
 * el copy formal del cobro por monto.
 */

/** Rótulo del campo de monto. */
internal fun copyLabelMontoSuelto(feria: Boolean): String =
    if (feria) "¿Cuánto es?" else "¿Cuánto le cobras?"

/**
 * Ayuda bajo el campo: moneda + que no hace falta catálogo.
 *
 * [simbolo] viene del ViewModel (no se inventa aquí).
 */
internal fun copyAyudaMontoSuelto(feria: Boolean, simbolo: String): String =
    if (feria) {
        "En $simbolo. Solo el monto, sin cargar nada."
    } else {
        "En $simbolo. No hace falta que tengas nada cargado."
    }

/**
 * CTA principal del cobro suelto.
 *
 * Feria: verbo corto de mesa («Cobrar» / «Anotando…»), alineado con
 * [copyCtaPago]. Retail: sigue explícito («Cobrar este monto»).
 */
internal fun copyCtaMontoSuelto(feria: Boolean, preparando: Boolean): String = when {
    preparando && feria -> "Anotando…"
    preparando -> "Preparando…"
    feria -> "Cobrar"
    else -> "Cobrar este monto"
}

/** Secundario: salir del cobro rápido. Igual en ambos packs. */
internal fun copyVolverMontoSuelto(): String = "Volver"

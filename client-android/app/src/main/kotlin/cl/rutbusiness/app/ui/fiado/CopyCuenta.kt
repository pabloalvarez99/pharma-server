package cl.rutbusiness.app.ui.fiado

/**
 * Copy propio de la hoja de cuenta de una persona (`DetalleDeCuenta.kt`).
 *
 * `CopyFiado.kt` cubre la lista de deudores y el paso de abono; esto es lo
 * que sólo hace falta al abrir a **una** persona: desde cuándo le debe, y
 * cómo se ve la hoja cuando quedó al día o todavía no tiene nada anotado.
 * Extraído para fijarlo con un test JVM puro, mismo patrón que `CopyFiado.kt`
 * y `CopyCatalogo.kt`.
 */

/**
 * Bajo el monto grande, cuando hay deuda viva: desde cuándo la tiene.
 *
 * @param fecha ya formateada por `fechaCorta` (orden chileno, no ISO).
 */
internal fun debeDesde(feria: Boolean, fecha: String): String =
    if (feria) "Te debe desde el $fecha" else "Debe desde el $fecha"

/** Chip de la hoja cuando ya no debe nada: el «quedó al día» del cuaderno. */
internal fun chipAlDia(): String = "Quedó al día"

/**
 * Por qué el monto grande dice $0 y no es un error ni una hoja rota.
 *
 * Sin esta frase un $0 se lee como que algo falló, no como la mejor
 * noticia posible de la hoja de esta persona.
 */
internal fun mensajeAlDia(feria: Boolean): String =
    if (feria) {
        "Ya te pagó todo lo que se llevó. Por ahora no te debe nada."
    } else {
        "Ya pagó todo lo que se llevó. Por ahora no debe nada."
    }

/** Chip de la hoja recién abierta: todavía no tiene nada anotado. */
internal fun chipHojaNueva(): String = "Hoja nueva"

/**
 * Todo el copy nuevo de esta pantalla, para el gate de vocabulario.
 *
 * Si se agrega copy nuevo acá, sumarlo a esta lista: el test la recorre.
 */
internal fun todoCopyCuentaUsuario(): List<String> = listOf(
    debeDesde(feria = true, fecha = "16-08-2026"),
    debeDesde(feria = false, fecha = "16-08-2026"),
    chipAlDia(),
    mensajeAlDia(feria = true),
    mensajeAlDia(feria = false),
    chipHojaNueva(),
)

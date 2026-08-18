package cl.rutbusiness.app.ui.resumen

import cl.rutbusiness.core.reports.TopProductoDto

/**
 * "¿Qué es lo que más vendo?", bajo la cifra del día.
 *
 * `abc_class` de [TopProductoDto] nunca aparece acá — es jerga de inventario,
 * ver la KDoc de `ReportsApi.topProductos`. Lo único que se muestra es el
 * nombre, la plata (texto del server, intacta) y una barra proporcional a
 * `revenuePct`, que el server ya calculó: nada de esto suma, promedia ni
 * recalcula un porcentaje.
 */

/**
 * `revenuePct` (texto, "0".."100") a fracción de ancho para la barra.
 *
 * Es la única conversión de plata a `Float` que se permite en este bloque, y
 * es para píxeles, no para mostrar: nunca se pinta este número como cifra.
 * Texto basura o fuera de rango no revienta el bloque — cae a un extremo
 * seguro, igual que un día sin ventas en [alturaDeBarra].
 */
internal fun fraccionDeBarra(revenuePct: String): Float {
    val porcentaje = revenuePct.toFloatOrNull() ?: return 0f
    val fraccion = (porcentaje / 100f).coerceIn(0f, 1f)
    if (fraccion <= 0f) return 0f
    // Piso visible: una barra de 0,4% de ancho es indistinguible de nada.
    return fraccion.coerceAtLeast(0.004f)
}

/**
 * La frase que reemplaza el bloque entero para un lector de pantalla.
 *
 * Dice el orden, no la plata: sumar los montos de la lista sería armar un
 * total que el server no mandó para este recorte (ver KDoc de
 * `ReportsApi.topProductos` sobre `limite`).
 *
 * @param nombres ya en el orden del ranking del server, nunca reordenados acá.
 */
internal fun descripcionMasVendes(nombres: List<String>): String {
    if (nombres.isEmpty()) return ""
    val cuerpo = nombres.mapIndexed { indice, nombre ->
        if (indice == 0) "$nombre primero" else "después $nombre"
    }.joinToString(", ")
    return "Lo que más vendes este mes: $cuerpo."
}

/** Los nombres de [productos], en el mismo orden que mandó el server. */
internal fun nombresOrdenados(productos: List<TopProductoDto>): List<String> =
    productos.map { it.productName }

package cl.rutbusiness.app.ui.cobrar

/**
 * Copy del paso Escanear (leer código de barras), ADR-0022.
 *
 * Puro JVM: [PasoEscaner] arma la pantalla, este archivo fija las frases. Se
 * lee como un aparato de la feria -se apunta, se lee, se sabe qué hacer si no
 * lee-, nunca como un log: nada de "reintentar la petición" ni "buffer".
 *
 * Feria evita "carrito" y "stock" -habla de "lo que vendes" / "lo que
 * llevas"-, igual que [copyBarraCarrito] y [copyTituloCarrito]; retail formal
 * conserva catálogo, carrito y boleta. La salida a mano (escribir el código)
 * está siempre disponible: en feria el escáner es un atajo, no un requisito.
 */

/** Título de la barra superior. */
internal fun copyTituloEscaner(creandoProducto: Boolean): String =
    if (creandoProducto) "Producto nuevo" else "Leer código"

/**
 * Bajada de la barra superior.
 *
 * Dando de alta explica por qué se abrió el formulario; con cámara en pantalla
 * indica dónde apoyar el código. Sin cámara -pantalla [SinCamara]- no hace
 * falta bajada: el cartel de abajo ya dice qué hacer.
 */
internal fun copySubtituloEscaner(
    feria: Boolean,
    creandoProducto: Boolean,
    hayCamara: Boolean,
): String? = when {
    creandoProducto -> if (feria) "No estaba en lo que vendes" else "No estaba en tu catálogo"
    hayCamara -> "Acerca el código a la luz"
    else -> null
}

/** Cartel de espera: la cámara está lista y todavía no leyó nada. */
internal fun copyTituloEsperando(): String = "Acerca el código de barras"

internal fun copyDetalleEsperando(): String =
    "Uno tras otro, sin salir entre medio. Como una linterna: apuntas y listo."

/** Cartel cuando el código se leyó bien pero no está entre lo que vende la dueña. */
internal fun copyTituloSinProducto(feria: Boolean): String =
    if (feria) "Ese código no está en lo que vendes" else "Ese código no está en tu catálogo"

/** [codigo] es el número leído: la dueña lo reconoce, no adivina qué pasó. */
internal fun copyDetalleSinProducto(codigo: String): String =
    "Leímos $codigo. Puedes crearlo ahora o escribirlo a mano."

/** Cartel de éxito: el producto ya entró a la venta. */
internal fun copyTituloListo(nombre: String): String = "Listo: $nombre"

/**
 * Feria dice "lo que llevas" y no "el carrito", igual que [copyTituloCarrito];
 * retail conserva "carrito".
 */
internal fun copyDetalleListo(feria: Boolean, enCarrito: Int): String {
    val unidad = if (enCarrito > 1) "$enCarrito unidades" else "1 unidad"
    return if (feria) "$unidad en lo que llevas" else "$unidad en el carrito"
}

internal fun copyBotonCrearProducto(): String = "Crear producto"
internal fun copyBotonListo(): String = "Listo"

/** Igual en ambos packs: la salida a mano siempre dice lo mismo. */
internal fun copyBotonEscribirAMano(): String = "Escribir a mano"

internal fun copyEtiquetaCodigoLeido(): String = "Código leído"
internal fun copyEtiquetaNombreNuevo(): String = "Nombre"

internal fun copyPlaceholderNombreNuevo(feria: Boolean): String =
    if (feria) "Como quieres que se vea en el papel" else "Como quieres verlo en la boleta"

internal fun copyEtiquetaPrecioNuevo(): String = "Precio"
internal fun copyPlaceholderPrecioNuevo(): String = "Sólo números"

/**
 * Ayuda bajo el precio: con algo escrito confirma cuánto se cobra; vacío,
 * dice qué precio es. Feria evita "venta al público" -habla de la clienta-.
 */
internal fun copyAyudaPrecioNuevo(feria: Boolean, cobrado: String?): String =
    cobrado?.let { "Se cobra $it" }
        ?: if (feria) "Lo que le cobras a la clienta." else "El precio de venta al público."

/**
 * Nota de cierre del formulario: nace con una unidad, el resto se completa
 * después. Feria evita "stock" -igual que [copyAyudaCatalogoGuardado]-.
 */
internal fun copyNotaProductoNuevo(feria: Boolean): String =
    if (feria) {
        "Queda con una unidad, la que tienes en la mano. El resto —categoría, costo, " +
            "proveedor— lo completas después con calma, pidiéndoselo al agente."
    } else {
        "Nace con una unidad en stock: la que tienes en la mano. El resto de la ficha " +
            "—categoría, costo, proveedor— se completa después con calma, pidiéndoselo al agente."
    }

internal fun copyBotonCrearYAgregar(guardando: Boolean): String =
    if (guardando) "Creando…" else "Crear y agregar"

package cl.rutbusiness.app.ui.impresora

/**
 * Copy de la tarjeta de impresión: imprimir la venta y reimprimir la última.
 *
 * Puro JVM: [TarjetaDeImpresion] y [TarjetaDeReimpresion] arman la pantalla,
 * este archivo fija las frases. Mismo patrón que `CopyComprobante.kt` y
 * `CopyEscaner.kt` en `ui/cobrar`, y que las cuatro funciones de
 * `Impresora.kt` (`copyQueHacerSinBluetooth` y hermanas) para las fallas.
 *
 * **Por qué existían dos convenciones en el mismo archivo.** `Impresora.kt`
 * ya prueba (`CopyImpresoraTest`) que feria nunca dice "boleta": no tiene
 * computador ni papel fiscal de día 1 (ADR-0022), así que habla de "papel del
 * puesto" o de "ticket". Pero la mitad **exitosa** de esta tarjeta —el botón
 * de imprimir, "Boleta impresa.", reimprimir la última, "Seguir sin boleta"—
 * decía "boleta" siempre, sin mirar el rubro. Un puesto de feria que nunca
 * emitió una boleta en su vida terminaba leyendo esa palabra en el momento
 * exacto en que la venta salió bien. Acá se cierra esa grieta con el mismo
 * criterio que ya usa `Impresora.kt`: feria dice "papel", retail formal
 * conserva "boleta".
 */

/** Título de la tarjeta principal, en la pantalla de "la venta quedó". */
internal fun copyTituloTarjetaImpresion(): String = "La impresora"

/** Título de la tarjeta de reimpresión, servida desde el menú del puesto. */
internal fun copyTituloTarjetaReimpresion(): String = "Otra copia del papel"

/**
 * Hay algo guardado para reimprimir: se ofrece repetirlo.
 *
 * Retail formal queda igual a como estaba (ver [ImpresoraFlujoTest]); feria
 * dice "papel" en vez de "boleta", como el resto de esta serie.
 */
internal fun copyReimpresionDisponible(feria: Boolean): String =
    if (feria) {
        "Sale el mismo papel de la última venta. Reimprimir no vuelve a cobrar."
    } else {
        "Sale la misma boleta de la última venta. Reimprimir no vuelve a cobrar."
    }

internal fun copyBotonReimprimir(): String = "Imprimir de nuevo"

/** Todavía no se imprimió nada desde este teléfono: no hay qué repetir. */
internal fun copySinReimpresionDisponible(feria: Boolean): String {
    val loQueFalta = if (feria) "ningún papel" else "ninguna boleta"
    return "Todavía no imprimiste $loQueFalta desde este teléfono. Cobra una venta y en la " +
        "pantalla de «Listo» va a aparecer el botón de imprimir; después vuelve a estar acá " +
        "para repetirla."
}

internal fun copyBotonCerrarReimpresion(): String = "Cerrar"

/** Sacando el papel: habla del aparato, no de un socket abriéndose. */
internal fun copyImprimiendo(nombre: String): String =
    "La impresora «$nombre» está sacando el papel…"

/**
 * La dueña ya decidió seguir sin papel. Nombra el estado en el que quedó, sin
 * culpa: la venta está guardada igual.
 */
internal fun copyEstadoSinBoleta(feria: Boolean): String {
    val loQueFalta = if (feria) "papel" else "boleta"
    return "Seguiste sin $loQueFalta. La venta quedó guardada igual."
}

internal fun copyBotonProbarDeImprimir(): String = "Probar de imprimir"

/** El comprobante todavía no llegó del server: no hay nada que mandar al papel. */
internal fun copySinDetalleParaImprimir(): String =
    "Todavía no tenemos el detalle de esta venta para imprimirlo. La venta sí quedó registrada."

/** Primera vez que se imprime en este teléfono: avisa antes de preguntar. */
internal fun copyPrimeraVezEligeImpresora(): String =
    "La primera vez te vamos a preguntar cuál es tu impresora. Después sale sola."

/** Bajo el nombre de la impresora recordada: por dónde y en qué rollo sale. */
internal fun copyDetallePlaca(nombre: String, anchoEtiqueta: String): String =
    "Sale por «$nombre», rollo de $anchoEtiqueta."

/**
 * El botón grande de imprimir. Retail formal queda igual a como estaba (ver
 * [ImpresoraFlujoTest] e [ImpresoraEscalaTest]); feria dice "papel".
 */
internal fun copyBotonImprimir(feria: Boolean): String =
    if (feria) "Imprimir el papel" else "Imprimir boleta"

internal fun copyBotonCambiarImpresora(): String = "Cambiar impresora"

internal fun copyEtiquetaTuImpresora(): String = "Tu impresora"

internal fun copyChipRollo(anchoEtiqueta: String): String = "Rollo $anchoEtiqueta"

/** El aparato terminó: nombre de la impresora si se conoce, genérico si no. */
internal fun copyTituloPapelSalio(nombre: String?): String =
    if (nombre != null) "«$nombre» soltó el papel" else "El papel salió"

/**
 * Confirmación bajo el título de éxito.
 *
 * Retail formal conserva el literal exacto de siempre —"Frase canónica" según
 * el comentario original en [TarjetaDeImpresion], y lo prueba
 * [ImpresoraFlujoTest]—; acá sólo se le suma la mitad que faltaba: feria dice
 * "papel", nunca "boleta".
 */
internal fun copyConfirmacionImpresion(feria: Boolean): String =
    if (feria) "Papel impreso." else "Boleta impresa."

internal fun copyBotonImprimirOtraCopia(): String = "Imprimir otra copia"

internal fun copyBotonListoImpresion(): String = "Listo"

internal fun copyBotonDarPermiso(): String = "Dar permiso"

internal fun copyBotonReintentar(): String = "Reintentar"

internal fun copyBotonElegirOtraImpresora(): String = "Elegir otra impresora"

/**
 * La salida de todo camino de falla: la venta ya está cobrada, así que nunca
 * hay que arreglar la impresora para poder seguir. Retail formal queda igual
 * (ver [ImpresoraFlujoTest] e [ImpresoraEscalaTest]); feria dice "papel".
 */
internal fun copyBotonSeguirSinBoleta(feria: Boolean): String =
    if (feria) "Seguir sin papel" else "Seguir sin boleta"

internal fun copyNotaVentaCobrada(): String =
    "La venta ya está cobrada y guardada. Nada de esto la deshace."

internal fun copyTituloElegirImpresora(): String = "¿Cuál es tu impresora?"

internal fun copyAyudaElegirImpresora(): String =
    "Estas son las que este teléfono ya tiene emparejadas. Si la tuya no está, enciéndela y " +
        "emparéjala desde Ajustes › Bluetooth."

internal fun copySubtituloTocarParaUsar(): String = "Toca para usarla"

internal fun copyBotonAhoraNo(): String = "Ahora no"

internal fun copyTituloElegirAncho(nombre: String): String =
    "¿De qué ancho es el papel de «$nombre»?"

internal fun copyAyudaElegirAncho(): String =
    "Mirá el rollo que le pusiste: el angosto cabe en la palma; el ancho es el de supermercado."

internal fun copyNotaCambiarAnchoDespues(): String =
    "Si te equivocas no pasa nada: se cambia después desde «Cambiar impresora»."

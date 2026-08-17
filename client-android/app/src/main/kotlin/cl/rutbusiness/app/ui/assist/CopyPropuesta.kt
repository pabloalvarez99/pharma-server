package cl.rutbusiness.app.ui.assist

/**
 * Copy de la tarjeta de propuesta (`TarjetaPropuesta.kt`), el momento en que la
 * dueña lee qué entendió el agente antes de que se anote algo. Puro JVM, mismo
 * patrón que `CopyEscaner.kt` y `CopyCuenta.kt`: la pantalla arma la tarjeta,
 * este archivo fija las frases, y el test las prueba sin Compose.
 *
 * El encabezado y el desglose de plata ([DetalleAccion]) son iguales en ambos
 * packs a propósito: la voz de puesto vive acá, en el botón de confirmar, en
 * el de cancelar y en los cierres — no en un "¿estás segura?" de admin.
 */

/**
 * Encabezado de la tarjeta: todavía no se anotó nada, y no suena a
 * "Confirmación" de formulario. Igual en ambos packs.
 */
internal fun copyEncabezado(): String = "Antes de hacerlo, revisa"

/** Mientras se está guardando, entre que se toca confirmar y la respuesta. */
internal fun copyCargando(feria: Boolean): String =
    if (feria) "Anotándolo…" else "Guardándolo…"

/**
 * Lo que queda una vez cancelada: reafirma que no pasó nada en el negocio, no
 * sólo que se cerró la tarjeta. Sin esa segunda parte, "cancelar" se siente
 * como que algo pudo haber quedado a medias.
 */
internal fun copyCierreCancelada(feria: Boolean): String =
    if (feria) {
        "Listo, no lo anoté. En tu puesto no cambió nada."
    } else {
        "No lo hice. No cambió nada en tu negocio."
    }

/** Botón para volver a pedir la acción cuando el token ya venció. */
internal fun copyBotonVolverAPedir(feria: Boolean): String =
    if (feria) "Decírmelo de nuevo" else "Pedirlo de nuevo"

/**
 * El verbo exacto de lo que va a pasar al tocar el botón principal.
 *
 * En feria es una sola frase hablada ("Sí, anota eso"): en el puesto nadie
 * dice "registrar la venta". En retail el botón nombra la acción del server
 * (`Action::label` en `crates/assist`); una desconocida cae en genérico
 * honesto, nunca en un verbo inventado.
 */
internal fun etiquetaDeConfirmar(nombreDeAccion: String, feria: Boolean = false): String {
    if (feria) return "Sí, anota eso"
    return when (nombreDeAccion) {
        "registrar_gasto" -> "Sí, registrar el gasto"
        "registrar_abono" -> "Sí, registrar el abono"
        "crear_cliente" -> "Sí, crear el cliente"
        "crear_proveedor" -> "Sí, crear el proveedor"
        "crear_producto_rapido" -> "Sí, crear el producto"
        "ajustar_precio" -> "Sí, cambiar el precio"
        "ajustar_stock" -> "Sí, ajustar el stock"
        "abrir_caja" -> "Sí, abrir la caja"
        "cerrar_caja" -> "Sí, cerrar la caja"
        "crear_orden_compra_draft" -> "Sí, crear la orden de compra"
        "dispensar_receta" -> "Sí, dispensar la receta"
        "registrar_venta" -> "Sí, registrar la venta"
        "registrar_fiado" -> "Sí, registrar el fiado"
        else -> "Sí, hazlo"
    }
}

/**
 * Salida sin escribir nada, un no sin culpa: ni "Cancelar" ni "Rechazar".
 * Feria suena a conversación; retail, a deshacer con calma.
 */
internal fun etiquetaDeCancelar(feria: Boolean): String =
    if (feria) "Mejor no" else "No, déjalo así"

/**
 * Tiempo que queda para decidir, en castellano hablado. Nunca un timestamp
 * crudo: un número de segundos contando hacia abajo mete presión y no ayuda a
 * leer con calma si el monto está bien.
 *
 * Feria: "anotar" / "decirme que sí". Retail: el tramo de [Vencimiento].
 */
internal fun vencimientoEnPalabras(segundosRestantes: Long, feria: Boolean): String {
    if (!feria) return Vencimiento.enPalabras(segundosRestantes)
    return when {
        segundosRestantes <= 0 -> "Ya se me pasó el tiempo para anotarlo."
        segundosRestantes <= 45 -> "Queda poquito para anotarlo."
        else -> "Tienes unos minutos para decirme que sí."
    }
}

/**
 * Todo el copy de la tarjeta, para el gate de vocabulario.
 *
 * Si se agrega copy nuevo a esta tarjeta, sumarlo acá: el test lo recorre
 * buscando jerga técnica o de admin que nunca debería llegarle a la dueña.
 */
internal fun todoCopyPropuestaUsuario(): List<String> {
    val nombresDeAccion = listOf(
        "registrar_gasto",
        "registrar_abono",
        "crear_cliente",
        "crear_proveedor",
        "crear_producto_rapido",
        "ajustar_precio",
        "ajustar_stock",
        "abrir_caja",
        "cerrar_caja",
        "crear_orden_compra_draft",
        "dispensar_receta",
        "registrar_venta",
        "registrar_fiado",
        "accion_que_no_existe_todavia",
    )
    val botonesDeConfirmar = nombresDeAccion.flatMap { nombre ->
        listOf(
            etiquetaDeConfirmar(nombre, feria = true),
            etiquetaDeConfirmar(nombre, feria = false),
        )
    }
    return listOf(
        copyEncabezado(),
        copyCargando(feria = true),
        copyCargando(feria = false),
        copyCierreCancelada(feria = true),
        copyCierreCancelada(feria = false),
        copyBotonVolverAPedir(feria = true),
        copyBotonVolverAPedir(feria = false),
        etiquetaDeCancelar(feria = true),
        etiquetaDeCancelar(feria = false),
        vencimientoEnPalabras(segundosRestantes = 120, feria = true),
        vencimientoEnPalabras(segundosRestantes = 30, feria = true),
        vencimientoEnPalabras(segundosRestantes = 0, feria = true),
        vencimientoEnPalabras(segundosRestantes = 120, feria = false),
        vencimientoEnPalabras(segundosRestantes = 30, feria = false),
        vencimientoEnPalabras(segundosRestantes = 0, feria = false),
    ) + botonesDeConfirmar
}

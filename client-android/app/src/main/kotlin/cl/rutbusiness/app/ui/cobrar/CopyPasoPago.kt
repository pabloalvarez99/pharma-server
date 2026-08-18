package cl.rutbusiness.app.ui.cobrar

import cl.rutbusiness.core.pos.MedioDePago

/**
 * Copy del paso «cómo paga» (mesa de feria, no caja de mall).
 *
 * Solo lo usa [PasoPago]. Puro JVM para que los tests atrapen regresión
 * a jerga de POS / "sistema" / "Confirmar transacción" sin montar Compose.
 */

/** Título de la tarjeta de medio de pago. */
internal fun copyComoPaga(feria: Boolean): String =
    if (feria) "¿Cómo te pagan?" else "Cómo paga"

/**
 * Chip de medio: es-CL hablado. Feria alarga un poco para que suene a mesa,
 * no a enum de API.
 */
internal fun copyEtiquetaMedio(medio: MedioDePago, feria: Boolean): String =
    if (!feria) {
        medio.etiqueta
    } else {
        when (medio) {
            MedioDePago.Efectivo -> "En efectivo"
            MedioDePago.Transferencia -> "Por transferencia"
            MedioDePago.Fiado -> "Fiado"
        }
    }

/**
 * El total que **no** se inventa en el teléfono.
 *
 * Offline o sin total del server: palabras honestas, sin número ni "sistema".
 */
internal fun copyTotalPendiente(feria: Boolean, hayConexion: Boolean): String =
    when {
        !hayConexion -> "se confirma al enviarse"
        feria -> "se confirma al cobrar"
        else -> "se confirma al cobrar"
    }

/** Etiqueta a la izquierda del total. */
internal fun copyEtiquetaTotal(feria: Boolean): String =
    if (feria) "A cobrar" else "Total"

/**
 * CTA principal del cobro.
 *
 * Verbo de feriante: «Cobrar», «Anotar fiado», «Anotar venta». Nunca
 * «Confirmar transacción» ni «Confirmar venta» cuando la plata aún no se mandó.
 */
internal fun copyCtaPago(
    feria: Boolean,
    cobrando: Boolean,
    hayConexion: Boolean,
    medio: MedioDePago,
): String = when {
    cobrando && medio == MedioDePago.Fiado ->
        if (feria) "Anotando fiado…" else "Anotando fiado..."
    // Sin señal no hay cobro que mandar: lo que pasa al tocar el botón es
    // que la venta se guarda en el teléfono para salir sola después. Decir
    // "Cobrando" en ese instante promete un viaje a la red que no ocurre;
    // aunque el tramo dura milisegundos, es la misma mentira chica que el
    // resto de esta pantalla evita.
    cobrando && !hayConexion ->
        if (feria) "Anotando venta…" else "Guardando venta…"
    cobrando ->
        if (feria) "Cobrando…" else "Cobrando..."
    !hayConexion ->
        if (feria) "Anotar venta" else "Guardar venta"
    medio == MedioDePago.Fiado -> "Anotar fiado"
    feria -> "Cobrar"
    else -> "Cobrar"
}

/** Secundario: volver a sumar cosas al carrito. */
internal fun copySeguirAgregando(feria: Boolean): String =
    if (feria) "Sumar otra cosa" else "Seguir agregando"

/** Carrito sin líneas en el paso de pago. */
internal fun copyCarritoVacioTitulo(feria: Boolean): String =
    if (feria) "Nada anotado todavía" else "Carrito vacío"

internal fun copyCarritoVacioBeneficio(feria: Boolean): String =
    if (feria) {
        "Así el monto que ves siempre es el que se cobra, sin sorpresas al pagar."
    } else {
        "Así el total que ves siempre es el que se cobra, sin diferencias al pagar."
    }

internal fun copyCarritoVacioPista(feria: Boolean): String =
    if (feria) {
        "Vuelve y suma lo que se lleva. Acá no se inventa un total: " +
            "el monto lo confirma el puesto al cobrar."
    } else {
        "Vuelve y agrega al menos un producto. El total lo confirma el " +
            "cobro, no el teléfono."
    }

/** Campo de billete entregado (solo con conexión + efectivo). */
internal fun copyLabelMontoEntregado(feria: Boolean): String =
    if (feria) "¿Con cuánto te pagan?" else "¿Con cuánto paga?"

/**
 * Ayuda del vuelto: el server calcula; el teléfono no inventa.
 * Feria no dice «sistema».
 */
internal fun copyAyudaVuelto(feria: Boolean): String =
    if (feria) {
        "El vuelto se confirma al cobrar, no lo inventa el teléfono."
    } else {
        "El vuelto lo calcula el cobro, no la app."
    }

/** Fiado: a quién se le anota. */
internal fun copyLabelClienteFiado(feria: Boolean): String =
    if (feria) "¿A quién se lo fías?" else "¿A quién se le fía?"

internal fun copyClientesVaciosTitulo(feria: Boolean): String =
    if (feria) "Todavía no tienes a quién fiarle" else "Todavía no tienes clientes anotados"

internal fun copyClientesVaciosBeneficio(feria: Boolean): String =
    if (feria) {
        "Así el fiado queda anotado a nombre de alguien, no perdido en el aire."
    } else {
        "Así el fiado queda en la cuenta de la persona correcta."
    }

internal fun copyClientesVaciosPista(feria: Boolean): String =
    if (feria) {
        "El fiado va a nombre de alguien. Cobra esta venta de otra forma " +
            "para no hacer esperar, y después dile al agente: " +
            "«agrega el cliente Juan Pérez»."
    } else {
        "El fiado queda en la cuenta de una persona, así que primero hay que " +
            "anotarla. Cobra esta venta de otra forma para no hacer esperar, y después " +
            "pídeselo al agente: «agrega el cliente Juan Pérez»."
    }

/** Precio unitario bajo el nombre: no es subtotal. */
internal fun copyPrecioUnitario(precioFormateado: String): String =
    "$precioFormateado c/u"

package cl.rutbusiness.app.ui.resumen

import cl.rutbusiness.app.ui.agente.ASI_SE_ANOTA_UNA_VENTA
import cl.rutbusiness.app.ui.agente.ASI_SE_FIA

/**
 * Copy de Resumen («Hoy») según feria vs retail formal (ADR-0022).
 *
 * Extraído para tests unitarios sin montar Compose: feria se lee como el
 * cuaderno del día (venta, puesto, día); farmacia/minimarket siguen con
 * boleta, caja y el computador del negocio.
 *
 * Nunca en feria: sistema, cajón, arqueo, computador, tablero, KPI, dashboard.
 */

// --- barra de arriba --------------------------------------------------------

/** Título del top bar. */
internal fun tituloHoy(feria: Boolean): String =
    if (feria) "Hoy" else "Tu día"

/**
 * Subtítulo por defecto cuando no hay copia offline ni nombre de puesto.
 *
 * El offline («Datos guardados…») y el nombre del puesto se arman en pantalla.
 */
internal fun subtituloHoy(feria: Boolean): String =
    if (feria) "Cómo va el puesto hoy" else "Cómo va el negocio hoy"

/** Botón de la barra: en feria se «mira» el día, no se «actualiza un tablero». */
internal fun labelActualizarHoy(feria: Boolean): String =
    if (feria) "Mirar de nuevo" else "Actualizar"

/** Primera carga sin cifras todavía. */
internal fun cargandoHoy(feria: Boolean): String =
    if (feria) {
        // Sin «cuenta» de ledger: es el cuaderno del día, no un arqueo.
        "Viendo cómo va el día..."
    } else {
        "Sacando la cuenta del día..."
    }

// --- tarjeta del día (hero) -------------------------------------------------

/** Rótulo chico sobre la cifra grande. */
internal fun tituloVendidoHoy(): String = "Vendiste hoy"

/**
 * Cuántas ventas/boletas del día, en la línea de soporte bajo la cifra grande.
 *
 * @param conteo pedidos del día que mandó el server (`orders`).
 */
internal fun copyTarjetaConteo(feria: Boolean, conteo: Long): String = when (conteo) {
    0L -> "Todavía no hay ninguna venta."
    1L -> if (feria) "1 venta." else "1 boleta."
    else -> if (feria) "$conteo ventas." else "$conteo boletas."
}

/** Chip de cómo va contra ayer. */
internal fun etiquetaComparacion(comparacion: Comparacion): String = when (comparacion) {
    Comparacion.Mejor -> "Mejor que ayer"
    Comparacion.Igual -> "Igual que ayer"
    Comparacion.Peor -> "Menos que ayer"
    Comparacion.SinDatoDeAyer -> "Sin comparación"
}

/**
 * Línea bajo el chip de comparación.
 *
 * @param vendidoAyerFormateado monto de ayer ya con símbolo; `null` si no vino.
 */
internal fun pistaComparacion(feria: Boolean, vendidoAyerFormateado: String?): String =
    if (vendidoAyerFormateado == null) {
        "No pudimos traer lo de ayer para comparar. Toca «${labelActualizarHoy(feria)}»."
    } else {
        // Se dice "día completo" porque es lo que se está comparando: hoy va a
        // medias y ayer ya terminó.
        "Ayer, día completo: $vendidoAyerFormateado."
    }

/**
 * Una línea para compartir cómo va el día (chat / WhatsApp).
 *
 * Mismos números que la tarjeta: el mensaje tiene que decir lo que la dueña
 * estaba mirando cuando tocó el botón.
 */
internal fun copyResumenDelDia(feria: Boolean, monto: String, conteo: Long): String =
    if (feria) {
        if (conteo == 1L) "$monto en 1 venta" else "$monto en $conteo ventas"
    } else {
        if (conteo == 1L) "$monto en 1 boleta" else "$monto en $conteo boletas"
    }

// --- vacío del día (feria, antes de la primera venta) -----------------------

internal fun tituloHoySinVentas(): String = "Todavía no vendiste nada hoy"

/** Enseña la frase del agente; no inventa un dashboard vacío. */
internal fun pistaHoySinVentas(): String =
    "Dile al agente: «$ASI_SE_ANOTA_UNA_VENTA». " +
        "Acá vas a ver cuánto llevas, sin sumar a mano."

internal fun ctaHablarleAlAgenteHoy(): String = "Hablarle al agente"

// --- te deben (siempre visible) ---------------------------------------------

internal fun tituloTeDebenHoy(): String = "Te deben"

internal fun errorFiadoHoy(feria: Boolean): String =
    "No pudimos traer lo del fiado. El resto sí está al día: toca " +
        "«${labelActualizarHoy(feria)}» arriba para volver a pedirlo."

internal fun vacioFiadoHoy(feria: Boolean): String =
    if (feria) {
        "Nadie te debe. Cuando fíes, díselo al agente: «$ASI_SE_FIA»."
    } else {
        "Nadie te debe plata. Cuando fíes una venta, la deuda aparece acá hasta " +
            "que te la paguen."
    }

internal fun cuantosTeDebenHoy(cuantos: Int): String =
    if (cuantos == 1) "1 persona te debe." else "$cuantos personas te deben."

internal fun ctaFiadoHoy(hayDeuda: Boolean): String =
    if (hayDeuda) "Ver quién me debe" else "Quién me debe"

// --- en la caja (retail formal; oculta en feria) ----------------------------

internal fun tituloEnCajaHoy(): String = "En la caja"

/**
 * Explica de dónde sale el monto «en la caja».
 *
 * En feria la tarjeta está oculta hoy, pero el copy se deja listo por si se
 * muestra: nunca «computador del negocio».
 */
internal fun copyEnCajaExplicacion(feria: Boolean, nombreDeCaja: String?): String =
    buildString {
        append("Es lo que debería haber ahora")
        nombreDeCaja?.let { append(" en «$it»") }
        if (feria) {
            append(". Lo calcula el puesto con lo que anotaste hoy.")
        } else {
            append(". Lo calcula el computador del negocio con la apertura, ")
            append("las ventas en efectivo y los movimientos.")
        }
    }

/** Sin caja abierta: solo retail formal (en feria la card no se dibuja). */
internal fun sinCajaAbiertaHoy(): String =
    "No hay ninguna caja abierta. Abrir la caja es lo primero del día: " +
        "desde ahí se empieza a contar la plata que entra."

internal fun errorEnCajaHoy(feria: Boolean): String =
    "No pudimos traer la cuenta de la caja. El resto sí está al día: toca " +
        "«${labelActualizarHoy(feria)}» arriba para volver a pedirla."

internal fun ctaCajaHoy(sinCajaAbierta: Boolean): String =
    if (sinCajaAbierta) "Abrir la caja" else "Ver la caja"

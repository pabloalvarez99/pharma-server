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

/**
 * Botón de la barra que abre el menú del puesto (caja, impresora, ventas
 * pendientes, cambiar de rubro — ver `MenuDelPuestoScreen.kt`). Una sola
 * palabra a propósito: comparte la barra con [labelActualizarHoy] y las dos
 * tienen que caber juntas sin cortarse al 200% de letra.
 */
internal fun labelAbrirMenuHoy(): String = "Más"

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

// --- semana (bajo la tarjeta del día) ----------------------------------------

/** Título de la tarjeta con el gráfico de la semana. */
internal fun tituloSemana(): String = "Cómo te fue la semana"

internal fun errorSemanaHoy(feria: Boolean): String =
    "No pudimos traer cómo te fue en la semana. El resto sí está al día: toca " +
        "«${labelActualizarHoy(feria)}» arriba para volver a pedirla."

private val LETRAS_DIA_SEMANA = listOf("lu", "ma", "mi", "ju", "vi", "sá", "do")
private val NOMBRES_DIA_SEMANA = listOf(
    "lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo",
)

/** Letra corta bajo cada barra (`0` lunes .. `6` domingo). La de hoy no la usa: dice «Hoy». */
internal fun letraDiaSemana(diaDeLaSemana: Int): String = LETRAS_DIA_SEMANA[diaDeLaSemana]

/** Nombre completo del día, para la frase que resume el gráfico a quien no lo ve. */
internal fun nombreDiaSemana(diaDeLaSemana: Int): String = NOMBRES_DIA_SEMANA[diaDeLaSemana]

/** Etiqueta bajo la barra de hoy: nunca la letra del día, para que se note distinta. */
internal fun etiquetaHoySemana(): String = "Hoy"

/**
 * La frase que reemplaza el gráfico entero para un lector de pantalla.
 *
 * Nunca dice "vendiste $X esta semana": eso sería sumar siete montos del
 * server en el teléfono, y la regla de plata no permite armar un total que
 * el server no mandó. Lo único que se puede decir sin inventar un peso es una
 * comparación — cuál día fue el mejor — así que eso es lo único que dice.
 *
 * @param mejorDiaFrase cómo se dice el día ya con su artículo resuelto: "el
 *   sábado", pero "hoy" sin "el" — nunca "el hoy". `null` si ningún día tuvo
 *   ventas todavía.
 * @param mejorDiaMontoFormateado ya con [cl.rutbusiness.core.money.Moneda.formatear] aplicado.
 */
internal fun descripcionSemana(mejorDiaFrase: String?, mejorDiaMontoFormateado: String?): String =
    if (mejorDiaFrase == null || mejorDiaMontoFormateado == null) {
        "Gráfico de la semana. Todavía no hay ventas para comparar los días."
    } else {
        "Gráfico de la semana. El mejor día fue $mejorDiaFrase, con $mejorDiaMontoFormateado."
    }

/** "el sábado", pero "hoy" sin artículo — nunca "el hoy". */
internal fun mejorDiaFrase(nombreDia: String, esHoy: Boolean): String =
    if (esHoy) "hoy" else "el $nombreDia"

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

// --- orden de las tarjetas ---------------------------------------------------

/**
 * Qué tarjeta va primero en «Hoy», de mayor a menor importancia para la dueña.
 *
 * Extraído a función pura para que el orden quede fijado por un test y no
 * dependa de leer bien el `LazyColumn`: cuánto entró, cómo va la semana,
 * cuánto le deben, y —solo en retail formal— qué falta cerrar (caja, stock,
 * vencimientos). "semana" va pegado a "ventas" porque es el mismo dato visto
 * con más contexto — sin eso la cifra grande queda sin con qué compararse
 * más allá de un adjetivo. En feria no hay tercer bloque: no hay caja que
 * cuadrar ni FEFO que mirar (ADR-0022), así que la lista termina en el fiado.
 */
internal fun ordenDeBloquesHoy(feria: Boolean): List<String> =
    if (feria) {
        listOf("ventas", "semana", "fiado")
    } else {
        listOf("ventas", "semana", "fiado", "caja", "faltantes", "vencimientos")
    }

// --- se está por acabar (retail formal; oculta en feria) --------------------

/** Vacío: nada bajo el umbral todavía. */
internal fun vacioSePorAcabar(umbral: Int): String =
    "No se está acabando nada. Cuando a un producto le queden $umbral o menos, " +
        "aparece acá para que lo encargues antes de quedarte sin."

/** Cuántos productos están bajo el umbral. */
internal fun tituloSePorAcabar(cuantos: Int): String =
    if (cuantos == 1) "1 producto se está acabando." else "$cuantos productos se están acabando."

/** Una fila de la lista corta: nombre y cuánto queda. */
internal fun filaSePorAcabar(nombre: String, stock: Long): String =
    "· $nombre — ${if (stock <= 0) "sin stock" else "quedan $stock"}"

/** Cuando hay más productos bajos de los que se listaron. */
internal fun masSePorAcabar(restantes: Int): String =
    "Y $restantes más. Pídeselos al agente: «¿qué se está por acabar?»."

// --- se está por vencer (retail formal; oculta en feria) --------------------

/** Vacío: nada se vence en la ventana de días que mira el server. */
internal fun vacioPorVencer(dias: Int): String =
    "Nada se vence en los próximos $dias días. Cuando algo se acerque a la fecha, " +
        "aparece acá con cuántos días le quedan."

/** Cuántos lotes se vencen pronto. */
internal fun tituloPorVencer(cuantos: Int): String =
    if (cuantos == 1) "1 lote se vence pronto." else "$cuantos lotes se vencen pronto."

/** Una fila de la lista corta: producto y plazo. */
internal fun filaPorVencer(producto: String, plazo: String): String = "· $producto — $plazo"

/** Cuando hay más lotes de los que se listaron. */
internal fun masPorVencer(restantes: Int): String = "Y $restantes más."

/** Cuánto le queda a un lote, dicho como lo diría una persona y no una fecha ISO. */
internal fun plazoDeVencimiento(diasParaVencer: Long, expired: Boolean): String = when {
    expired -> "ya vencido"
    diasParaVencer <= 0L -> "vence hoy"
    diasParaVencer == 1L -> "vence mañana"
    else -> "le quedan $diasParaVencer días"
}

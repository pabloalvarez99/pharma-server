package cl.rutbusiness.app.ui.fiado

/**
 * Copy de Fiado según feria vs retail formal (ADR-0022).
 *
 * Extraído para tests unitarios sin Compose ni ViewModel: feria se lee como
 * nombres y plata («quién me debe»), no como ledger de cuenta corriente;
 * farmacia/minimarket pueden seguir con caja/cajón/computador.
 */

/** Frase del agente que el vacío de feria enseña a copiar. */
internal const val EJEMPLO_FIADO_FERIA = "fié tomates a Rosa a 2000"

// --- lista -----------------------------------------------------------------

internal fun tituloListaDeuda(): String = "Quién me debe"

internal fun subtituloListaDeuda(): String = "Gente que te debe"

internal fun labelBuscarDeudor(): String = "Buscar a quien vino a pagar"

internal fun placeholderBuscarDeudor(): String = "Nombre o teléfono"

internal fun cargandoListaDeuda(): String = "Viendo quién te debe..."

internal fun tituloTotalPorCobrar(): String = "Te deben"

internal fun cuantosTeDeben(cuantos: Int): String =
    if (cuantos == 1) "1 persona te debe." else "$cuantos personas te deben."

internal fun tituloVacioDeuda(feria: Boolean): String =
    if (feria) "Nadie te debe ahora" else "Nadie te debe plata"

internal fun pistaVacioDeuda(feria: Boolean): String =
    if (feria) {
        "Dile al agente: «$EJEMPLO_FIADO_FERIA». Queda acá hasta que te pague."
    } else {
        "Cuando fíes una venta, la deuda de esa persona aparece acá hasta que " +
            "te la termine de pagar."
    }

internal fun ctaHablarleAlAgente(): String = "Hablarle al agente"

internal fun vacioBusquedaTitulo(): String = "Nadie con ese nombre"

internal fun vacioBusquedaPista(): String =
    "Revisa cómo lo escribiste, o borra la búsqueda para ver a todos los que te deben."

internal fun ctaVerTodos(): String = "Ver a todos"

// --- detalle de una persona ------------------------------------------------

/** Fallback del top bar si no hay nombre (no debería pasar con lista real). */
internal fun tituloDetalleFallback(feria: Boolean): String =
    if (feria) "Quién te debe" else "La cuenta"

internal fun subtituloDetalle(): String = "Lo que se llevó y lo que fue pagando"

/** Título de la tarjeta con el monto grande: feria = «Te debe», no «Saldo». */
internal fun tituloDebeAhora(feria: Boolean): String =
    if (feria) "Te debe" else "Debe ahora"

internal fun cargandoDetalle(feria: Boolean): String =
    if (feria) {
        "Viendo cuánto te debe..."
    } else {
        "Trayendo la cuenta..."
    }

/** No se pudo traer el detalle de la persona (monto vacío arriba). */
internal fun errorSinCuenta(feria: Boolean): String =
    if (feria) {
        "No pudimos ver cuánto te debe esta persona. Toca atrás y vuelve a " +
            "abrirla; si sigue igual, revisa la señal e intenta de nuevo."
    } else {
        "No pudimos traer la cuenta de esta persona. Toca atrás y vuelve a " +
            "abrirla; si sigue igual, revisa que el computador del negocio esté " +
            "encendido."
    }

internal fun sinMovimientosCargados(feria: Boolean): String =
    if (feria) {
        "No alcanzamos a traer lo que se llevó y lo que pagó. No quiere decir " +
            "que no haya: vuelve a abrir a esta persona en un momento."
    } else {
        "No alcanzamos a traer los movimientos. No quiere decir que no " +
            "haya: vuelve a abrir esta cuenta en un momento."
    }

internal fun tituloMovimientos(): String = "Lo que se llevó y lo que pagó"

/**
 * Primera vez que se abre a alguien sin líneas todavía.
 *
 * Feria enseña la frase del agente; formal manda a «Cobrar».
 */
internal fun vacioMovimientos(feria: Boolean): String =
    if (feria) {
        "Todavía no hay nada anotado de esta persona. Aparece solo: cuando le " +
            "fíes algo — «$EJEMPLO_FIADO_FERIA» — y cuando anotes que te pagó."
    } else {
        "Todavía no hay movimientos en esta cuenta. Aparecen solos: cuando " +
            "le fíes una venta desde «Cobrar», y cuando anotes que te pagó."
    }

/** Una línea del historial: se llevó o dejó plata (no «cargo»/«abono»). */
internal fun etiquetaMovimiento(esAbono: Boolean, feria: Boolean): String =
    when {
        feria && esAbono -> "Dejó plata"
        feria && !esAbono -> "Se llevó"
        esAbono -> "Te pagó"
        else -> "Se llevó fiado"
    }

internal fun totalSeLlevo(): String = "Se llevó fiado en total"

internal fun totalTePago(): String = "Te ha pagado en total"

internal fun ctaMeEstaPagando(): String = "Me está pagando"

internal fun chipQuedoAnotado(): String = "Quedó anotado"

// --- anotar un pago --------------------------------------------------------

/** Top bar del paso: alguien te está pagando en el puesto. */
internal fun tituloPasoAbono(): String = "Me está pagando"

/**
 * Pregunta del monto: feria suena a mesa («¿cuánto me das?»); retail al
 * mostrador formal.
 */
internal fun tituloCuantoPaga(feria: Boolean = false): String =
    if (feria) "¿Cuánto me das?" else "¿Cuánto te está pagando?"

/**
 * Referencia a cuánto debe, al lado del campo de monto.
 *
 * @param montoFormateado ya formateado con la moneda del negocio (sale del server).
 */
internal fun referenciaDeuda(feria: Boolean, montoFormateado: String): String =
    if (feria) {
        "Te debe $montoFormateado. Puede ser todo o una parte."
    } else {
        "Debe $montoFormateado. Puede pagarte todo o una parte."
    }

/** Etiqueta del campo de plata: lo que la persona te está dando ahora. */
internal fun labelMontoPago(feria: Boolean = false): String =
    if (feria) "Lo que te da" else "Plata que te da"

internal fun tituloComoPaga(): String = "¿Cómo te paga?"

internal fun chipEfectivo(): String = "En efectivo"

internal fun chipTransferencia(): String = "Transferencia"

/**
 * Ayuda bajo el toggle efectivo / transferencia.
 *
 * @param entraALaCaja `true` = billete que cuenta en el cierre (retail) o en
 *   la plata del día del puesto (feria).
 */
internal fun ayudaComoPaga(feria: Boolean, entraALaCaja: Boolean): String =
    when {
        feria && entraALaCaja -> "Esta plata cuenta en el día del puesto."
        feria && !entraALaCaja ->
            "No cuenta en la plata del día: es transferencia u otro medio " +
                "que no está en el puesto."
        entraALaCaja -> "El billete entra a la caja y va a estar en el cierre de hoy."
        else ->
            "No toca la caja: esa plata no está en el cajón, así que no aparece " +
                "en el cierre."
    }

internal fun tituloNotaPago(): String = "¿Algo que anotar?"

internal fun labelNotaPago(feria: Boolean = false): String =
    if (feria) "Algo que anotar (opcional)" else "Nota del pago (opcional)"

internal fun placeholderNotaPago(feria: Boolean = false): String =
    if (feria) {
        "Trae el resto el viernes"
    } else {
        "Quedó de traer el resto el viernes"
    }

internal fun previaAnotarPago(feria: Boolean = false): String =
    if (feria) "Vas a anotar lo que te dio:" else "Vas a anotar que te pagó:"

internal fun ctaAnotarPago(guardando: Boolean, feria: Boolean = false): String =
    when {
        guardando -> "Anotando..."
        feria -> "Anotar lo que me dio"
        else -> "Anotar el pago"
    }

/** Cerrar el paso de pago sin «cuenta» en feria. */
internal fun ctaVolverDelAbono(feria: Boolean): String =
    if (feria) "Volver" else "Volver a la cuenta"

/** Remate del aviso tras anotar un pago en efectivo. */
internal fun remateAbonoEfectivo(feria: Boolean): String =
    if (feria) {
        " Esa plata cuenta en el día."
    } else {
        " Esa plata entró a la caja y va a estar en el cierre."
    }

/**
 * Botón de la barra en la lista: feria «mira» quién debe, no «actualiza un sistema».
 * Misma voz que Hoy / Caja.
 */
internal fun labelActualizarFiado(feria: Boolean): String =
    if (feria) "Mirar de nuevo" else "Actualizar"

/**
 * Todas las cadenas de usuario del camino feria, para el gate de vocabulario.
 *
 * Si se agrega copy nuevo de feria, sumarlo acá: el test recorre esta lista.
 */
internal fun todoCopyFeriaUsuario(): List<String> = listOf(
    tituloListaDeuda(),
    subtituloListaDeuda(),
    labelBuscarDeudor(),
    placeholderBuscarDeudor(),
    cargandoListaDeuda(),
    labelActualizarFiado(feria = true),
    tituloTotalPorCobrar(),
    cuantosTeDeben(1),
    cuantosTeDeben(3),
    tituloVacioDeuda(feria = true),
    pistaVacioDeuda(feria = true),
    ctaHablarleAlAgente(),
    vacioBusquedaTitulo(),
    vacioBusquedaPista(),
    ctaVerTodos(),
    tituloDetalleFallback(feria = true),
    subtituloDetalle(),
    tituloDebeAhora(feria = true),
    cargandoDetalle(feria = true),
    errorSinCuenta(feria = true),
    sinMovimientosCargados(feria = true),
    tituloMovimientos(),
    vacioMovimientos(feria = true),
    etiquetaMovimiento(esAbono = true, feria = true),
    etiquetaMovimiento(esAbono = false, feria = true),
    totalSeLlevo(),
    totalTePago(),
    ctaMeEstaPagando(),
    chipQuedoAnotado(),
    tituloPasoAbono(),
    tituloCuantoPaga(feria = true),
    referenciaDeuda(feria = true, montoFormateado = "$2.000"),
    labelMontoPago(feria = true),
    tituloComoPaga(),
    chipEfectivo(),
    chipTransferencia(),
    ayudaComoPaga(feria = true, entraALaCaja = true),
    ayudaComoPaga(feria = true, entraALaCaja = false),
    tituloNotaPago(),
    labelNotaPago(feria = true),
    placeholderNotaPago(feria = true),
    previaAnotarPago(feria = true),
    ctaAnotarPago(guardando = false, feria = true),
    ctaAnotarPago(guardando = true, feria = true),
    ctaVolverDelAbono(feria = true),
    remateAbonoEfectivo(feria = true),
    EJEMPLO_FIADO_FERIA,
)

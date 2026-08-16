package cl.rutbusiness.app.ui.caja

/**
 * Copy de Caja según feria vs retail formal (ADR-0022).
 *
 * Extraído para tests unitarios sin montar Compose ni el ViewModel: feria habla
 * de "puesto" y del día; farmacia/minimarket siguen con "cajón" y el ritual de
 * apertura con billetes.
 */
internal data class CopyAbrirCaja(
    val tituloCard: String,
    val ayuda: String,
    val etiquetaMonto: String,
    val ayudaMonto: String,
    val cta: String,
    val ctaGuardando: String,
)

internal fun copyAbrirCaja(feria: Boolean): CopyAbrirCaja =
    if (feria) {
        CopyAbrirCaja(
            tituloCard = "Abrir el puesto",
            ayuda = "El puesto arranca sin contar monedas. Escribí 0 si querés " +
                "anotar algo distinto.",
            etiquetaMonto = "Plata con la que parte el puesto",
            ayudaMonto = "El puesto arranca sin contar monedas. Escribí 0.",
            cta = "Empezar el día",
            ctaGuardando = "Abriendo el puesto...",
        )
    } else {
        CopyAbrirCaja(
            tituloCard = "¿Con cuánta plata partes?",
            ayuda = "Cuenta lo que hay en el cajón para dar vuelto y escríbelo. " +
                "Desde que abres la caja, todo lo que vendas en efectivo se va sumando " +
                "solo hasta el cierre.",
            etiquetaMonto = "Plata con la que parte el cajón",
            ayudaMonto = "Si el cajón arranca vacío, escribe 0.",
            cta = "Abrir la caja",
            ctaGuardando = "Abriendo...",
        )
    }

internal data class CopyCajaAbierta(
    val chipEstado: String,
    val tituloEsperado: String,
    val ctaCerrar: String,
    val tituloMovimientos: String,
    val vacioMovimientos: String,
    val ctaSacar: String,
    val ctaMeter: String,
)

internal fun copyCajaAbierta(feria: Boolean): CopyCajaAbierta =
    if (feria) {
        CopyCajaAbierta(
            chipEstado = "Puesto abierto",
            tituloEsperado = "Debería haber en el puesto",
            ctaCerrar = "Cerrar el día",
            tituloMovimientos = "Plata que sacaste o metiste a mano",
            vacioMovimientos = "Todavía no sacaste ni metiste plata a mano. " +
                "Lo que cobrás en billete ya está contado arriba.",
            ctaSacar = "Sacar plata",
            ctaMeter = "Meter plata",
        )
    } else {
        CopyCajaAbierta(
            chipEstado = "Caja abierta",
            tituloEsperado = "Debería haber en el cajón",
            ctaCerrar = "Cerrar la caja",
            tituloMovimientos = "Lo que entró y salió a mano",
            vacioMovimientos = "Todavía no sacaste ni metiste plata a mano. Las ventas en efectivo " +
                "no aparecen acá: entran solas y ya están contadas arriba.",
            ctaSacar = "Sacar plata",
            ctaMeter = "Meter plata",
        )
    }

/**
 * Copy del paso «contar la plata» / cierre.
 *
 * En feria es cerrar el cuaderno del día: puesto, contar, sin «arqueo» ni
 * «cajón» ni «sesión». En retail sigue el ritual del cajón.
 */
internal data class CopyArqueoCaja(
    val tituloCard: String,
    val ayuda: String,
    val etiquetaMonto: String,
    val ayudaMonto: String,
    val tituloNota: String,
    val etiquetaNota: String,
    val placeholderNota: String,
    val ayudaNota: String,
    val cta: String,
    val ctaGuardando: String,
    val ctaVolver: String,
    val confirmarTitulo: String,
    val confirmarLabel: String,
    val cancelarLabel: String,
)

/**
 * Mensaje del diálogo de confirmación del cierre: calmado, irreversible, sin
 * inglés. [montoFormateado] ya viene con símbolo/separadores de la moneda.
 */
internal fun copyConfirmarCierre(
    feria: Boolean,
    montoFormateado: String,
): String =
    if (feria) {
        "El día queda cerrado con los $montoFormateado que contaste. No se " +
            "puede deshacer: mañana se abre el puesto de nuevo."
    } else {
        "La caja queda cerrada con los $montoFormateado que contaste. Después " +
            "no se puede volver a abrir la misma caja: mañana se abre una nueva."
    }

internal fun copyArqueoCaja(feria: Boolean): CopyArqueoCaja =
    if (feria) {
        CopyArqueoCaja(
            tituloCard = "Contar la plata",
            ayuda = "Contá la plata del puesto, como en el cuaderno, y escribí " +
                "el total. Después te mostramos cómo quedó el día.",
            etiquetaMonto = "Plata contada",
            ayudaMonto = "Solo lo que tenés en la mano. Si no quedó plata, escribí 0.",
            tituloNota = "¿Algo que anotar?",
            etiquetaNota = "Nota del día (opcional)",
            placeholderNota = "El vuelto que se dio mal, compré cambio…",
            ayudaNota = "Si ya sabés que algo no va a cuadrar — el vuelto que se " +
                "dio mal, compré cambio, un gasto suelto — escribilo acá. Después " +
                "de cerrar el día no se puede agregar.",
            cta = "Cerrar el día",
            ctaGuardando = "Cerrando el día…",
            ctaVolver = "Todavía no",
            confirmarTitulo = "¿Cerramos el día?",
            confirmarLabel = "Sí, cerrar",
            cancelarLabel = "Seguir contando",
        )
    } else {
        CopyArqueoCaja(
            tituloCard = "¿Cuánta plata hay en el cajón?",
            ayuda = "Saca la plata, cuéntala tranquila y escribe el total. Recién después de " +
                "cerrar te mostramos cómo quedó contra lo que el sistema tenía anotado.",
            etiquetaMonto = "Plata contada",
            ayudaMonto = "Cuenta billetes y monedas. Si el cajón quedó vacío, escribe 0.",
            tituloNota = "¿Algo que anotar?",
            etiquetaNota = "Nota del cierre (opcional)",
            placeholderNota = "Le di mal el vuelto a un cliente",
            ayudaNota = "Si ya sabes que algo no va a cuadrar, escríbelo acá: después de " +
                "cerrar no se puede agregar.",
            cta = "Cerrar la caja",
            ctaGuardando = "Cerrando...",
            ctaVolver = "Todavía no",
            confirmarTitulo = "¿Cerramos la caja?",
            confirmarLabel = "Sí, cerrar",
            cancelarLabel = "Seguir contando",
        )
    }

internal data class CopyMovimientoCaja(
    val tituloCard: String,
    val ayuda: String,
    val etiquetaMonto: String,
    val cta: String,
    val ctaGuardando: String,
    val ctaVolver: String,
)

/**
 * Sacar o meter plata a mano: feria habla del día/puesto, retail del cajón.
 */
internal fun copyMovimientoCaja(feria: Boolean, esRetiro: Boolean): CopyMovimientoCaja =
    if (feria) {
        if (esRetiro) {
            CopyMovimientoCaja(
                tituloCard = "¿Cuánto sacás?",
                ayuda = "La plata que sacás del puesto para pagar algo o para guardarla. " +
                    "Se descuenta de lo que debería haber al cerrar el día.",
                etiquetaMonto = "Plata que sacás",
                cta = "Anotar que sacaste",
                ctaGuardando = "Anotando...",
                ctaVolver = "Volver al puesto",
            )
        } else {
            CopyMovimientoCaja(
                tituloCard = "¿Cuánto metés?",
                ayuda = "La plata que le agregás al puesto: cambio que trajiste, un vuelto " +
                    "que devolvieron. Se suma a lo que debería haber al cerrar el día.",
                etiquetaMonto = "Plata que metés",
                cta = "Anotar que metiste",
                ctaGuardando = "Anotando...",
                ctaVolver = "Volver al puesto",
            )
        }
    } else {
        if (esRetiro) {
            CopyMovimientoCaja(
                tituloCard = "¿Cuánto sacas?",
                ayuda = "La plata que sacas del cajón para pagar algo o para guardarla. Se descuenta " +
                    "de lo que debería haber al cerrar.",
                etiquetaMonto = "Plata que sacas",
                cta = "Anotar que sacaste",
                ctaGuardando = "Anotando...",
                ctaVolver = "Volver a la caja",
            )
        } else {
            CopyMovimientoCaja(
                tituloCard = "¿Cuánto metes?",
                ayuda = "La plata que le agregas al cajón: cambio que trajiste, un vuelto que " +
                    "devolvieron. Se suma a lo que debería haber al cerrar.",
                etiquetaMonto = "Plata que metes",
                cta = "Anotar que metiste",
                ctaGuardando = "Anotando...",
                ctaVolver = "Volver a la caja",
            )
        }
    }

internal fun tituloPasoCaja(paso: PasoDeCaja, feria: Boolean, tipoMovimiento: String): String =
    when (paso) {
        PasoDeCaja.Abrir -> if (feria) "Abrir el puesto" else "Abrir la caja"
        PasoDeCaja.Abierta -> if (feria) "El puesto de hoy" else "La caja de hoy"
        PasoDeCaja.Movimiento ->
            if (tipoMovimiento == NuevoMovimiento.RETIRO) "Sacar plata" else "Meter plata"
        PasoDeCaja.Arqueo -> "Contar la plata"
        PasoDeCaja.Cerrada -> if (feria) "Día cerrado" else "Caja cerrada"
    }

/**
 * Si un error de apertura significa "ya hay caja abierta" (409 o copy del server).
 *
 * Segundo `POST /cash-sessions` del mismo cajero es éxito en feria: otro camino
 * (Cobrar, el agente) ya abrió el puesto.
 */
fun esCajaYaAbierta(error: cl.rutbusiness.core.error.AppError): Boolean {
    if (error is cl.rutbusiness.core.error.AppError.ErrorDelServidor && error.status == 409) {
        return true
    }
    val msg = error.userMessage.lowercase()
    return msg.contains("ya tiene una caja") ||
        msg.contains("caja abierta") ||
        msg.contains("already has an open")
}

/**
 * Qué acción falló por red — el cuerpo del mensaje cambia; feria nunca nombra
 * «computador del negocio».
 */
internal enum class AccionErrorRed {
    Apertura,
    Cierre,
    Cobro,
}

/**
 * Mensaje de [AppError.ServidorNoResponde] en Caja / Cobrar.
 *
 * Feria: señal del teléfono, sin IP ni «computador». Retail/farmacia on-prem
 * sigue con la frase del computador del negocio.
 */
internal fun copyErrorRed(feria: Boolean, accion: AccionErrorRed = AccionErrorRed.Apertura): String =
    when (accion) {
        AccionErrorRed.Apertura ->
            if (feria) {
                "No llegamos. Revisá la señal del teléfono y reintentá; si el puesto " +
                    "igual quedó abierto, lo vas a ver al actualizar."
            } else {
                "No llegamos al computador del negocio. Revisa que el teléfono tenga " +
                    "señal y vuelve a intentar; si la caja igual quedó abierta, la vas a " +
                    "ver al actualizar."
            }

        AccionErrorRed.Cierre ->
            if (feria) {
                "No llegamos. Revisá la señal del teléfono y reintentá: si el día ya " +
                    "se había cerrado, te lo vamos a decir."
            } else {
                "No llegamos al computador del negocio. Revisa que el teléfono tenga " +
                    "señal y vuelve a intentar: si ya se había cerrado, te lo vamos a decir."
            }

        AccionErrorRed.Cobro ->
            if (feria) {
                "No alcanzó a anotarse la venta. Revisá la señal y tocá «Reintentar»: " +
                    "aunque la mandes de nuevo, no se cobra dos veces."
            } else {
                "No alcanzamos a mandarle la venta al computador del negocio. Revisa que " +
                    "el teléfono tenga señal y toca «Reintentar»: aunque la mandes de nuevo, " +
                    "no se cobra dos veces."
            }
    }

/**
 * Ayuda bajo el monto «debería haber»: de dónde sale el número.
 *
 * Feria habla del puesto; retail sigue con el computador del negocio.
 */
internal fun copyEsperadoEnPuesto(feria: Boolean): String =
    if (feria) {
        "Lo calcula el puesto con lo que anotaste al abrir, lo que cobraste y los movimientos."
    } else {
        "Lo calcula el computador del negocio con lo que pusiste al abrir, lo que " +
            "vendiste en efectivo y lo que sacaste o metiste a mano."
    }

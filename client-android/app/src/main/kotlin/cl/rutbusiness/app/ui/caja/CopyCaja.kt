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
    val tituloEsperado: String,
    val ctaCerrar: String,
)

internal fun copyCajaAbierta(feria: Boolean): CopyCajaAbierta =
    if (feria) {
        CopyCajaAbierta(
            tituloEsperado = "Debería haber en el puesto",
            ctaCerrar = "Cerrar el día",
        )
    } else {
        CopyCajaAbierta(
            tituloEsperado = "Debería haber en el cajón",
            ctaCerrar = "Cerrar la caja",
        )
    }

internal data class CopyArqueoCaja(
    val tituloCard: String,
    val ayuda: String,
    val ayudaMonto: String,
    val cta: String,
    val ctaGuardando: String,
    val confirmarTitulo: String,
)

internal fun copyArqueoCaja(feria: Boolean): CopyArqueoCaja =
    if (feria) {
        CopyArqueoCaja(
            tituloCard = "¿Cuánta plata hay?",
            ayuda = "Contá la plata del día y escribí el total. Después te " +
                "mostramos cómo quedó contra lo anotado en el sistema.",
            ayudaMonto = "Si no quedó plata, escribí 0.",
            cta = "Cerrar el día",
            ctaGuardando = "Cerrando...",
            confirmarTitulo = "¿Cerramos el día?",
        )
    } else {
        CopyArqueoCaja(
            tituloCard = "¿Cuánta plata hay en el cajón?",
            ayuda = "Saca la plata, cuéntala tranquila y escribe el total. Recién después de " +
                "cerrar te mostramos cómo quedó contra lo que el sistema tenía anotado.",
            ayudaMonto = "Cuenta billetes y monedas. Si el cajón quedó vacío, escribe 0.",
            cta = "Cerrar la caja",
            ctaGuardando = "Cerrando...",
            confirmarTitulo = "¿Cerramos la caja?",
        )
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

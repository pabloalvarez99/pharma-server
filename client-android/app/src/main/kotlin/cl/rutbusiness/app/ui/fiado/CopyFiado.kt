package cl.rutbusiness.app.ui.fiado

/**
 * Copy de Fiado según feria vs retail formal (ADR-0022).
 *
 * Extraído para tests unitarios sin Compose ni ViewModel: feria habla de señal,
 * puesto y plata del día; farmacia/minimarket siguen con computador y cajón.
 */

/** No se pudo traer la cuenta del cliente (saldo vacío en el detalle). */
internal fun errorSinCuenta(feria: Boolean): String =
    if (feria) {
        "No pudimos traer la cuenta de esta persona. Toca atrás y vuelve a " +
            "abrirla; si sigue igual, revisá la señal e intentá de nuevo."
    } else {
        "No pudimos traer la cuenta de esta persona. Toca atrás y vuelve a " +
            "abrirla; si sigue igual, revisa que el computador del negocio esté " +
            "encendido."
    }

/**
 * Ayuda bajo el toggle efectivo / transferencia del abono.
 *
 * @param entraALaCaja `true` = billete que cuenta en el cierre / día.
 */
internal fun ayudaComoPaga(feria: Boolean, entraALaCaja: Boolean): String =
    when {
        feria && entraALaCaja -> "Esta plata cuenta en el día."
        feria && !entraALaCaja ->
            "No cuenta en la plata del día: es transferencia u otro medio " +
                "que no está en el puesto."
        entraALaCaja -> "El billete entra a la caja y va a estar en el cierre de hoy."
        else ->
            "No toca la caja: esa plata no está en el cajón, así que no aparece " +
                "en el cierre."
    }

/** Remate del aviso tras anotar un abono en efectivo. */
internal fun remateAbonoEfectivo(feria: Boolean): String =
    if (feria) {
        " Esa plata cuenta en el día."
    } else {
        " Esa plata entró a la caja y va a estar en el cierre."
    }

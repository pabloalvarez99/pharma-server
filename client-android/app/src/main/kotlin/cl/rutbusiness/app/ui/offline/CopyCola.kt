package cl.rutbusiness.app.ui.offline

/**
 * Copy de la cola de ventas offline (ADR-0022).
 *
 * Extraído para tests unitarios sin montar Compose: feria no habla del
 * "sistema del negocio" (PC on-prem); el APK de feria anota en la nube / RutAgent.
 */
internal fun hintColaVacia(feria: Boolean): String =
    if (feria) {
        "Todas las ventas que cobraste ya se anotaron."
    } else {
        "Todas las ventas que cobraste ya llegaron al sistema del negocio."
    }

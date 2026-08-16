package cl.rutbusiness.app.ui.offline

/**
 * Copy de la cola de ventas offline (ADR-0022).
 *
 * Extraído para tests unitarios sin montar Compose: feria no habla del
 * "sistema del negocio" (PC on-prem); el APK de feria anota en la nube / RutAgent.
 *
 * El tono es de **recado** (sin conexión · ventas esperando), no de error de
 * red ni de panel de sync.
 */
internal fun hintColaVacia(feria: Boolean): String =
    if (feria) {
        "Todas las ventas que cobraste ya se anotaron."
    } else {
        "Todas las ventas que cobraste ya llegaron al sistema del negocio."
    }

/**
 * Recado de la franja cuando no hay señal y no hay cola.
 * Callado a propósito: no es un error, es un aviso de estado.
 */
internal fun recadoSinConexion(): String = "Sin conexión"

/** Segunda línea del recado sin señal y sin cola. */
internal fun detalleSinConexion(): String = "Ves lo último que se cargó."

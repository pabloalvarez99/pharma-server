package cl.rutbusiness.app.ui.caja

/**
 * Copy del chrome de Caja (top bar + empty offline + carga).
 *
 * Extraído de [CajaScreen] para tests JVM sin Compose. Feria = ritual del día
 * (puesto, señal, mirar de nuevo); retail sigue con cajón / sistema / arqueo.
 *
 * Nunca en feria: sistema, cajón, arqueo, computador, tablero.
 * El teléfono no inventa lo que «debería haber»: sin red no se cierra el día.
 */

/**
 * Botón de la barra: en feria se «mira» el día, no se «actualiza un sistema».
 * Misma voz que [cl.rutbusiness.app.ui.resumen.labelActualizarHoy].
 */
internal fun labelActualizarCaja(feria: Boolean): String =
    if (feria) "Mirar de nuevo" else "Actualizar"

/** Primera carga del estado de caja / puesto. */
internal fun cargandoCaja(feria: Boolean): String =
    if (feria) "Abriendo el puesto..." else "Viendo cómo está la caja..."

/**
 * Empty offline: sin señal no hay contra qué comparar el contado.
 *
 * Feria: honestidad de feriante — la plata la confirma el cobro; se sigue
 * vendiendo en efectivo y se cierra el día cuando vuelva la red.
 * Retail: el cajón y el sistema del negocio (copy histórico).
 */
internal data class CopyCajaSinConexion(
    val titulo: String,
    val hint: String,
    val accion: String,
)

internal fun copyCajaSinConexion(feria: Boolean): CopyCajaSinConexion =
    if (feria) {
        CopyCajaSinConexion(
            titulo = "Sin señal no se cierra el día",
            hint = "La plata del puesto la confirma el cobro, no el teléfono. " +
                "Seguí vendiendo en efectivo; cuando vuelva la red cerrás el día.",
            accion = "Volver",
        )
    } else {
        CopyCajaSinConexion(
            titulo = "La caja necesita el sistema prendido",
            hint = "Sin conexión no se puede abrir la caja, anotar retiros ni cerrar con " +
                "arqueo: lo que debería haber en el cajón lo calcula el sistema del " +
                "negocio sumando las ventas del día, y desde el teléfono no hay contra " +
                "qué comparar. Mientras tanto puedes seguir cobrando en efectivo.",
            accion = "Volver",
        )
    }

/**
 * Subtítulo del top bar por paso: ritual del día en feria; admin de cajón en retail.
 *
 * [nombreCaja] solo se usa en retail con caja abierta (el nombre del register).
 */
internal fun subtituloPasoCaja(
    paso: PasoDeCaja,
    feria: Boolean,
    nombreCaja: String? = null,
): String? = when (paso) {
    PasoDeCaja.Abrir -> if (feria) "Sin contar monedas" else "Lo primero del día"
    // Feria: el register suele llamarse "puesto"; no lo repite el subtítulo.
    PasoDeCaja.Abierta -> if (feria) {
        "Día en marcha"
    } else {
        nombreCaja?.ifBlank { null }
    }
    PasoDeCaja.Movimiento -> if (feria) "Queda en la cuenta del día" else "Queda anotado hasta el cierre"
    PasoDeCaja.Arqueo -> if (feria) "Contá primero, después cerrás" else "Cuenta primero, después cierra"
    PasoDeCaja.Cerrada -> if (feria) "Listo por hoy" else null
}

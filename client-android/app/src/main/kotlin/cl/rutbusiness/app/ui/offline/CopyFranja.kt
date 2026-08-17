package cl.rutbusiness.app.ui.offline

/**
 * Copy de la franja de conexión (arriba del shell).
 *
 * Se lee como **recado**, no como error de red: sin gerundio inglés, sin
 * "sistema", con "puesto" / "señal" en feria. Extraído para tests JVM sin
 * montar Compose.
 *
 * Corto a propósito: cada renglón se le saca a la lista de productos
 * (medido en API 23, panel 640 dp).
 */

/** "1 venta" / "3 ventas". El singular importa: lo lee una persona. */
internal fun ventas(cuantas: Int): String =
    if (cuantas == 1) "1 venta" else "$cuantas ventas"

/**
 * Título de la franja según estado.
 *
 * Orden igual que la UI: sin señal manda; si hay cola en camino y también
 * rechazos, el título habla del camino (el rojo del fondo ya avisa rechazo).
 */
internal fun tituloFranja(
    conectado: Boolean,
    esperando: Int,
    rechazadas: Int,
    feria: Boolean,
): String = when {
    !conectado && esperando > 0 ->
        "${recadoSinConexion()} · ${ventas(esperando)} esperando"
    !conectado -> recadoSinConexion()
    rechazadas > 0 && esperando == 0 -> tituloRechazadas(rechazadas, feria)
    esperando > 0 -> tituloEnCamino(esperando, feria)
    else -> tituloRechazadas(rechazadas, feria)
}

/**
 * Con señal y ventas por anotar: van en camino, no "Enviando…".
 *
 * Feria: "Van N en camino" (cuaderno). Retail: "N ventas en camino".
 */
internal fun tituloEnCamino(cuantas: Int, feria: Boolean): String =
    if (feria) {
        if (cuantas == 1) "Va 1 en camino" else "Van $cuantas en camino"
    } else {
        "${ventas(cuantas)} en camino"
    }

/**
 * El server rechazó: en feria no se "envía", se anota en el puesto.
 */
internal fun tituloRechazadas(cuantas: Int, feria: Boolean): String =
    if (feria) {
        if (cuantas == 1) {
            "1 venta no se anotó en el puesto"
        } else {
            "$cuantas ventas no se anotaron en el puesto"
        }
    } else {
        if (cuantas == 1) {
            "1 venta no se pudo enviar"
        } else {
            "$cuantas ventas no se pudieron enviar"
        }
    }

/**
 * Segunda línea. Sin cola tocable no invita a tocar.
 * Offline + en camino: solo la promesa de la señal (corto).
 */
internal fun detalleFranja(
    conectado: Boolean,
    esperando: Int,
    rechazadas: Int,
    feria: Boolean,
): String = when {
    rechazadas > 0 -> if (feria) "Tocá para ver cuáles." else "Tócalo para ver cuáles."
    !conectado && esperando > 0 ->
        if (esperando == 1) "Sale sola al volver la señal" else "Salen solas al volver la señal"
    !conectado -> detalleSinConexion()
    else -> if (feria) "Tocá para verlas." else "Tócalo para verlas."
}

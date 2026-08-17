package cl.rutbusiness.app.ui.entrada

/**
 * Copy del cartel de primer uso (3 pantallas antes del login).
 *
 * Puro y testeable. Con `nube=true` (APK feria/nube) habla de **puesto** y de
 * la mesa: cero computador, IP, servidor ni sistema. On-prem (`nube=false`)
 * puede nombrar el computador del negocio y la dirección LAN.
 *
 * [googleDisponible] es el mismo `disponible()` del botón de Google en la
 * pantalla de entrada: si el texto tuviera su propia constante, un APK con
 * client id mostraría el botón andando y, dos pantallas antes, "pronto".
 */
internal data class PasoDelPrimerUso(
    val titulo: String,
    val encabezado: String,
    val parrafos: List<String>,
    val lista: List<String> = emptyList(),
    val remate: String? = null,
)

/** CTA principal: en el último paso feria/nube dice "Abrir el puesto". */
internal fun ctaPrimarioPrimerUso(ultimo: Boolean, nube: Boolean): String =
    when {
        ultimo && nube -> "Abrir el puesto"
        ultimo -> "Empezar"
        else -> "Siguiente"
    }

/** Saltar: feria/nube suena a la mesa; retail nombra la explicación. */
internal fun ctaSaltarPrimerUso(nube: Boolean): String =
    if (nube) "Ya sé, entrar" else "Saltar la explicación"

/**
 * Los tres pasos, con el paso 3 escrito según lo que este build puede hacer.
 *
 * @param googleDisponible mismo `disponible()` del botón de Google en entrada.
 * @param nube true = APK feria/nube (puesto, sin computador).
 */
internal fun pasosDelPrimerUso(
    googleDisponible: Boolean,
    nube: Boolean = false,
): List<PasoDelPrimerUso> {
    val cuenta = if (googleDisponible) {
        "Tu cuenta de Google, o tu correo y tu clave."
    } else {
        "Tu correo y tu clave. Pronto también con tu cuenta de Google " +
            "(sin otra contraseña que acordarte)."
    }

    val pasoUno = if (nube) {
        PasoDelPrimerUso(
            titulo = "Esto es RutAgent",
            encabezado = "El cuaderno del puesto, en tu teléfono",
            parrafos = listOf(
                "Anotás una venta con la voz o el teclado, sabés quién te debe y " +
                    "cuánto hiciste hoy — más rápido que el cuaderno de mil pesos.",
                "Hablale como en la mesa. Antes de guardar nada, te lo muestra " +
                    "para que lo revises.",
            ),
        )
    } else {
        PasoDelPrimerUso(
            titulo = "Esto es RutAgent",
            encabezado = "El cuaderno del negocio, en tu teléfono",
            parrafos = listOf(
                "Anotás una venta con la voz o el teclado, sabés quién te debe y " +
                    "cuánto hiciste hoy — más rápido que el cuaderno de mil pesos.",
                "Hablale como a un empleado de confianza. Antes de guardar nada, " +
                    "te lo muestra para que lo revises.",
            ),
        )
    }

    val pasoDos = PasoDelPrimerUso(
        titulo = "Dónde se guarda todo",
        encabezado = "En el teléfono y en un respaldo cifrado",
        parrafos = listOf(
            "Vendés aunque no haya señal: lo del día queda en el aparato.",
            "Si activás el respaldo, se sube cifrado con una llave tuya. " +
                "Nosotros no podemos leerla ni recuperarla: la escribís en el " +
                "cuaderno el primer día (tarjeta de rescate).",
        ),
        remate = "Sin esa llave, el respaldo no sirve. Con ella y tu cuenta, " +
            "volvés a entrar si se te rompe el teléfono.",
    )

    val pasoTres = if (nube) {
        PasoDelPrimerUso(
            titulo = "Lo que necesitás a mano",
            encabezado = "Para empezar",
            parrafos = emptyList(),
            lista = listOf(
                "Un nombre para tu puesto (como lo dicen en la feria).",
                cuenta,
            ),
            remate = "Nada más. No hace falta un computador ni una dirección.",
        )
    } else {
        PasoDelPrimerUso(
            titulo = "Lo que necesitás a mano",
            encabezado = "Para entrar la primera vez",
            parrafos = emptyList(),
            lista = listOf(
                "La dirección del computador del negocio (si te la dieron). " +
                    "Se ve así: 192.168.1.10:8080",
                "El nombre corto de tu negocio.",
                cuenta,
            ),
            remate = "¿No los tenés? Pedíselos a quien instaló el sistema. " +
                "Quedaron anotados el día que lo instaló.",
        )
    }

    return listOf(pasoUno, pasoDos, pasoTres)
}

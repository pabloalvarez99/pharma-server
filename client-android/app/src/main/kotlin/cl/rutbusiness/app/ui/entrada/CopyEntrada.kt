package cl.rutbusiness.app.ui.entrada

/**
 * Copy del formulario de entrar (login) según on-prem vs nube (ADR-0022).
 *
 * Puro y testeable: el APK de feria/nube no puede nombrar "computador",
 * "sistema del negocio", IP ni "servidor". El LAN on-prem sí habla del
 * computador que la dueña puede tocar.
 */
internal fun copyAvisoGoogleSinDestino(pideDireccion: Boolean): String =
    if (pideDireccion) {
        "Primero poné la dirección del computador del negocio."
    } else {
        "RutAgent no responde. Reintentá en un momento."
    }

/**
 * Tras un fallo del canje Google: el mensaje del server + qué hacer.
 * Nunca dice "servidor" (jerga); manda a correo y clave.
 */
internal fun copyAvisoGoogleFalloLogin(mensajeDelServer: String): String =
    mensajeDelServer.trimEnd() +
        " Si Google aún no está listo, usá correo y clave."

package cl.rutbusiness.app.ui.common

import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.ui.components.RbErrorCopy
import cl.rutbusiness.ui.components.RbErrorKind
import cl.rutbusiness.ui.components.rbErrorCopy

/**
 * Traduce una falla de la capa de red al texto que ve el usuario.
 *
 * Cuando el server mandó su propio mensaje, ése gana: ya está escrito en
 * español y sabe cosas que el teléfono no ("no queda stock de Arroz Grado 1").
 * Mantener un catálogo paralelo acá sería garantizar que los dos se
 * desincronicen.
 *
 * **Tono = recado.** Si el fallback de [AppError] trae "servidor" o un código
 * HTTP, no se muestra: se cambia por el recado genérico de [rbErrorCopy].
 * Nube y feria no ven IP ni computador acá (salvo [AppError.DireccionInvalida],
 * que es sólo el camino local).
 *
 * @param que completa "no pudimos cargar ___": "tus productos", "tus clientes".
 */
fun AppError.aCopy(que: String): RbErrorCopy = when (this) {
    is AppError.ServidorNoResponde -> rbErrorCopy(RbErrorKind.Offline, que)
    is AppError.SesionExpirada -> rbErrorCopy(RbErrorKind.Unauthorized, que)
    is AppError.SinPermiso -> rbErrorCopy(RbErrorKind.Forbidden, que)
    is AppError.DireccionInvalida -> RbErrorCopy(
        title = "La dirección no se entiende",
        message = userMessage,
        retryLabel = null,
    )

    is AppError.CredencialesInvalidas -> RbErrorCopy(
        title = "No pudimos entrar",
        message = userMessage,
        retryLabel = "Reintentar",
    )

    is AppError.ErrorDelServidor -> {
        // Mensaje de dominio del server se respeta. El fallback de AppError
        // mete "servidor" + "error 503" y se lee como stacktrace.
        if (mensajeDeServidorEsTecnico(userMessage)) {
            rbErrorCopy(RbErrorKind.ServerFault, que)
        } else {
            RbErrorCopy(
                title = "No se pudo",
                message = userMessage,
                retryLabel = "Reintentar",
            )
        }
    }

    is AppError.Inesperado -> rbErrorCopy(RbErrorKind.Unknown, que)
}

/** Fallback de [AppError.ErrorDelServidor] o cualquier envelope con código HTTP. */
internal fun mensajeDeServidorEsTecnico(mensaje: String): Boolean {
    if (mensaje.isBlank()) return true
    if (mensaje.contains("servidor tuvo un problema", ignoreCase = true)) return true
    if (Regex("""\berror\s+\d{3}\b""", RegexOption.IGNORE_CASE).containsMatchIn(mensaje)) {
        return true
    }
    return false
}

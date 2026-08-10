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

    is AppError.ErrorDelServidor -> RbErrorCopy(
        title = "No se pudo completar",
        message = userMessage,
        retryLabel = "Reintentar",
    )

    is AppError.Inesperado -> rbErrorCopy(RbErrorKind.Unknown, que)
}

package cl.rutbusiness.core.net

import cl.rutbusiness.core.api.models.ErrorEnvelope
import cl.rutbusiness.core.error.AppError
import io.ktor.client.statement.HttpResponse
import io.ktor.client.statement.bodyAsText
import io.ktor.http.isSuccess
import io.ktor.util.reflect.typeInfo
import kotlinx.coroutines.CancellationException
import cl.rutbusiness.core.api.infrastructure.HttpResponse as RespuestaGenerada

/**
 * Resultado de hablar con el server. No se usa `kotlin.Result` porque obliga a
 * que la falla sea un `Throwable`, y acá la falla es un [AppError] con el
 * mensaje ya escrito para el usuario.
 */
sealed interface Resultado<out T> {
    data class Ok<T>(val valor: T) : Resultado<T>
    data class Falla(val error: AppError) : Resultado<Nothing>
}

/** Vehículo interno para sacar un [AppError] desde adentro de un [llamar]. */
class ErrorDeApi(val appError: AppError) : Exception(appError.userMessage)

/**
 * Corre una llamada al server y traduce cualquier explosión a un [AppError].
 *
 * Esta capa no deja escapar ninguna excepción: la UI de arriba nunca envuelve
 * nada en try/catch y por eso nunca puede quedar una pantalla en blanco por un
 * error no atrapado.
 */
suspend fun <T> llamar(baseUrl: String, bloque: suspend () -> T): Resultado<T> = try {
    Resultado.Ok(bloque())
} catch (e: CancellationException) {
    // Cancelar una corrutina no es un error del usuario: se re-lanza tal cual.
    throw e
} catch (e: ErrorDeApi) {
    Resultado.Falla(e.appError)
} catch (e: Throwable) {
    Resultado.Falla(AppError.ServidorNoResponde(baseUrl, technical = e.toString()))
}

/** Corta con [ErrorDeApi] si el server no contestó 2xx. */
suspend fun HttpResponse.exigirExito(baseUrl: String): HttpResponse {
    if (!status.isSuccess()) throw ErrorDeApi(errorDesde(this, baseUrl))
    return this
}

/**
 * Cuerpo tipado de una respuesta del cliente generado.
 *
 * El spec declara casi todos los bodies como `object` opaco (ver el comentario
 * de `crates/api/src/openapi.rs`), así que el generador devuelve `Any`. El tipo
 * real se pide acá, reusando los modelos que el mismo generador ya creó. Cuando
 * el server anote los schemas, esto se reemplaza por `.body()` a secas.
 */
suspend inline fun <reified V : Any> RespuestaGenerada<*>.cuerpo(baseUrl: String): V {
    if (!success) throw ErrorDeApi(errorDesde(response, baseUrl))
    return typedBody(typeInfo<V>())
}

/**
 * Convierte un status HTTP de error en el [AppError] que corresponde,
 * aprovechando el mensaje en español que ya trae el envelope del server en vez
 * de inventar uno paralelo que se desincronice.
 */
suspend fun errorDesde(respuesta: HttpResponse, baseUrl: String): AppError {
    val texto = runCatching { respuesta.bodyAsText() }.getOrNull()
    val envelope = texto
        ?.takeIf { it.isNotBlank() }
        ?.let { runCatching { ApiFactory.JSON.decodeFromString<ErrorEnvelope>(it) }.getOrNull() }
    val code = envelope?.error?.code
    val mensaje = envelope?.error?.message

    return when {
        code == "BAD_CREDENTIALS" -> AppError.CredencialesInvalidas(technical = texto)
        respuesta.status.value == 401 -> AppError.SesionExpirada(technical = texto)
        respuesta.status.value == 403 -> AppError.SinPermiso(technical = texto)
        respuesta.status.value == 503 -> AppError.ServidorNoResponde(baseUrl, technical = texto)
        else -> AppError.ErrorDelServidor(
            status = respuesta.status.value,
            code = code,
            serverMessage = mensaje,
            technical = texto,
        )
    }
}

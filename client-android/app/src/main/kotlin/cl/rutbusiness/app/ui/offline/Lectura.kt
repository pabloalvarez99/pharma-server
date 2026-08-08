package cl.rutbusiness.app.ui.offline

import cl.rutbusiness.core.error.AppError
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.offline.ClaveDeCache
import kotlinx.serialization.KSerializer

/**
 * De dónde salió lo que se está mostrando.
 *
 * Existe para que la pantalla **no pueda** olvidarse de decirlo: si el dato es
 * del teléfono, el tipo trae la hora en que se trajo, y mostrarlo sin la fecha
 * requeriría ignorar un campo a propósito. Un número viejo sin fecha al lado es
 * un número en el que la dueña va a confiar como si fuera de ahora.
 */
sealed interface Lectura<out T> {

    /** Recién traído del sistema del negocio. */
    data class DelServidor<T>(val valor: T) : Lectura<T>

    /** Lo último que se había traído. Va con su fecha, siempre. */
    data class DelTelefono<T>(val valor: T, val guardadoEn: Long) : Lectura<T>

    /** No se pudo, y tampoco había nada guardado. */
    data class Falla(val error: AppError) : Lectura<Nothing>

    val valorONulo: T? get() = when (this) {
        is DelServidor -> valor
        is DelTelefono -> valor
        is Falla -> null
    }
}

/**
 * Pide al server y, si no contesta, cae en lo que había guardado.
 *
 * El orden importa y no es negociable: **siempre se intenta el server
 * primero**. El caché no es una optimización de velocidad, es una red debajo
 * del trapecio; servirlo primero haría que la dueña viera stock viejo teniendo
 * señal, que es peor que esperar dos segundos.
 *
 * Sólo se cae al caché cuando el server **no contestó**. Si contestó que no
 * (sin permiso, sesión vencida), eso es una respuesta y se muestra tal cual:
 * tapar un 403 con datos viejos sería esconder que esta cuenta no puede ver
 * esto.
 */
suspend fun <T> conCache(
    offline: ServiciosOffline?,
    que: ClaveDeCache.Que,
    servidor: String,
    serializador: KSerializer<T>,
    traer: suspend () -> Resultado<T>,
): Lectura<T> {
    val clave = ClaveDeCache.de(que, servidor)

    return when (val respuesta = traer()) {
        is Resultado.Ok -> {
            offline?.cache?.guardar(clave, serializador, respuesta.valor)
            Lectura.DelServidor(respuesta.valor)
        }

        is Resultado.Falla -> {
            val guardado = if (respuesta.error is AppError.ServidorNoResponde) {
                offline?.cache?.leer(clave, serializador)
            } else {
                null
            }
            if (guardado != null) {
                Lectura.DelTelefono(guardado.valor, guardado.guardadoEn)
            } else {
                Lectura.Falla(respuesta.error)
            }
        }
    }
}

/**
 * Lo guardado, sin salir a la red.
 *
 * Para cuando ya se sabe que no hay conexión y salir a la red sería regalar
 * treinta segundos de espera antes de mostrar lo mismo.
 */
suspend fun <T> soloCache(
    offline: ServiciosOffline?,
    que: ClaveDeCache.Que,
    servidor: String,
    serializador: KSerializer<T>,
): Lectura.DelTelefono<T>? {
    val guardado = offline?.cache?.leer(ClaveDeCache.de(que, servidor), serializador) ?: return null
    return Lectura.DelTelefono(guardado.valor, guardado.guardadoEn)
}

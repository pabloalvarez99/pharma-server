package cl.rutbusiness.app.ui.entrada

import cl.rutbusiness.core.session.SessionRepository
import io.ktor.client.plugins.timeout
import io.ktor.client.request.get
import io.ktor.client.statement.bodyAsText
import io.ktor.http.isSuccess
import kotlinx.coroutines.CancellationException

/**
 * Qué había del otro lado de la dirección que escribió la dueña.
 *
 * Tres respuestas y no dos, porque "no se pudo conectar" mezcla dos problemas
 * que se arreglan en lugares distintos: que no conteste nadie se revisa en el
 * local, y que conteste otra cosa se revisa con quien instaló el sistema.
 */
sealed interface Sondeo {
    /** Contestó el sistema del negocio y está listo para trabajar. */
    data object EsElSistema : Sondeo

    /** No hubo respuesta: apagado, dirección mala, o teléfono en otra red. */
    data class NadieContesta(val tecnico: String) : Sondeo

    /**
     * Hubo respuesta HTTP, pero no la de un RutBusiness sano: otro programa
     * escuchando en ese número, el portal cautivo de un wifi, o el sistema
     * prendido pero sin su base de datos.
     */
    data class ContestaOtraCosa(val tecnico: String) : Sondeo
}

/**
 * Toca la puerta antes de intentar entrar.
 *
 * Interfaz y no clase concreta por lo mismo que [cl.rutbusiness.app.ui.impresora.EnlaceDeImpresora]:
 * los caminos de falla son el 80% del valor de esta pantalla y tienen que poder
 * probarse sin levantar un servidor ni desenchufar un router.
 */
interface ProbadorDeServidor {
    suspend fun sondear(baseUrl: String): Sondeo
}

/**
 * El de verdad: le pide al server su propio chequeo de salud.
 *
 * `GET /health/ready` es el endpoint correcto para esto y no `/api/v1/login`:
 * no necesita credenciales, no consume el limitador de intentos, y contesta
 * distinto según **el server esté sano o no**, que es justo lo que hay que
 * saber antes de mandar una contraseña.
 *
 * Se mira el cuerpo y no sólo el código: un portal cautivo de wifi contesta 200
 * con una página de login del hotel, y un 200 a secas nos haría decirle a la
 * dueña que su negocio está andando cuando lo que contestó fue el router del
 * café de al lado. `checks.db` sólo lo escribe `crates/api/src/health.rs`.
 *
 * Los tiempos son más cortos que los de [cl.rutbusiness.core.net.ApiFactory] a
 * propósito: acá hay alguien mirando la pantalla y esperando una respuesta, no
 * una pantalla ya cargada refrescándose. Treinta segundos de reloj girando es
 * el punto exacto en que se cierra la app y se desinstala.
 */
class ServidorPorHttp(private val sesion: SessionRepository) : ProbadorDeServidor {

    override suspend fun sondear(baseUrl: String): Sondeo {
        val http = sesion.apiPara(baseUrl).http

        val respuesta = try {
            http.get("$baseUrl/health/ready") {
                timeout {
                    connectTimeoutMillis = CONECTAR_MS
                    requestTimeoutMillis = RESPONDER_MS
                    socketTimeoutMillis = RESPONDER_MS
                }
            }
        } catch (e: CancellationException) {
            // Cancelar no es una falla del usuario: sale por donde entró.
            throw e
        } catch (e: Throwable) {
            return Sondeo.NadieContesta(e.toString())
        }

        val cuerpo = runCatching { respuesta.bodyAsText() }.getOrDefault("")
        val esNuestro = MARCA_DEL_SERVER.containsMatchIn(cuerpo)

        return when {
            respuesta.status.isSuccess() && esNuestro -> Sondeo.EsElSistema
            else -> Sondeo.ContestaOtraCosa(
                "${respuesta.status.value} ${cuerpo.take(120)}",
            )
        }
    }

    private companion object {
        const val CONECTAR_MS = 6_000L
        const val RESPONDER_MS = 10_000L

        /**
         * La huella de `ReadyResponse` en `crates/api/src/health.rs`.
         *
         * Se busca la forma y no el texto exacto: el valor de `db` cambia según
         * cómo esté la base, pero la llave está siempre. Se acepta espacio
         * después de los dos puntos porque un proxy puede reformatear el JSON.
         */
        val MARCA_DEL_SERVER = Regex("\"db\"\\s*:")
    }
}

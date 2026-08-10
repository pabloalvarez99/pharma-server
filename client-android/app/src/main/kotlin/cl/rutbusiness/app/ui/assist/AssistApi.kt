package cl.rutbusiness.app.ui.assist

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import io.ktor.client.call.body
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject

/**
 * Los dos endpoints del agente, escritos a mano.
 *
 * No salen del generador de OpenAPI porque el spec declara los bodies como
 * `object` opaco, así que el generador devolvería `Any` y habría que
 * re-tipear igual. Escribirlos acá deja el contrato a la vista, que en este
 * caso importa más que en otros: es un handshake de dos pasos donde el
 * segundo paso escribe en el ERP.
 *
 * El contrato lo define `crates/assist/src/actions.rs`:
 *
 * 1. `POST /assist/ask` con la pregunta. Si es una pregunta de lectura vuelve
 *    solo `text`. Si es una orden de escritura vuelve además `action`: la
 *    propuesta armada **sin haber escrito nada**, con un `confirm_token` que
 *    dura poco.
 * 2. `POST /assist/act` con ese token exacto. Recién ahí el server escribe.
 *    El token se consume: repetirlo es rechazado, y vence solo.
 *
 * Los parámetros de la acción quedan congelados en el server entre los dos
 * pasos y nunca viajan de vuelta desde el teléfono, así que no hay forma de
 * que la app cambie lo que se va a ejecutar después de que la dueña lo leyó.
 * Por eso esta clase manda el token y nada más.
 */
class AssistApi(private val api: ApiFactory) {

    suspend fun preguntar(pregunta: String): Resultado<RespuestaAgente> =
        llamar(api) {
            api.http.post("${api.baseUrl}/api/v1/assist/ask") {
                contentType(ContentType.Application.Json)
                setBody(PreguntaRequest(pregunta.trim()))
            }.exigirExito(api.baseUrl).body()
        }

    /**
     * Ejecuta una acción ya propuesta.
     *
     * @param confirmToken tal cual vino en la propuesta, sin tocar.
     */
    suspend fun confirmar(confirmToken: String): Resultado<ResultadoAccion> =
        llamar(api) {
            api.http.post("${api.baseUrl}/api/v1/assist/act") {
                contentType(ContentType.Application.Json)
                setBody(ConfirmarRequest(confirmToken))
            }.exigirExito(api.baseUrl).body()
        }
}

@Serializable
internal data class PreguntaRequest(val question: String)

@Serializable
internal data class ConfirmarRequest(@SerialName("confirm_token") val confirmToken: String)

/** Lo que devuelve `POST /assist/ask`. */
@Serializable
data class RespuestaAgente(
    /** Etiqueta interna del intent resuelto. Sirve para diagnóstico, no se muestra. */
    val intent: String = "",
    /** La respuesta en español, ya redactada por el server. */
    val text: String = "",
    /** Cifras estructuradas detrás de la prosa, cuando las hay. */
    val data: JsonObject? = null,
    /**
     * Presente **solo** cuando la pregunta era una orden de escribir. Su
     * ausencia es la señal de que no hay nada que confirmar.
     */
    val action: PropuestaAccion? = null,
)

/** La propuesta que la dueña lee antes de que se escriba nada. */
@Serializable
data class PropuestaAccion(
    /** Etiqueta de la acción: `registrar_gasto`, `ajustar_stock`, … */
    val name: String = "",
    /** Resumen en una línea, ya en español, escrito por el server. */
    val summary: String = "",
    /** Los parámetros exactos que se van a ejecutar. */
    val params: JsonObject = JsonObject(emptyMap()),
    /** Opaco. Se devuelve verbatim y nunca se muestra en pantalla. */
    @SerialName("confirm_token") val confirmToken: String = "",
    /** Cuándo deja de servir el token, en RFC 3339 UTC. */
    @SerialName("expires_at") val expiresAt: String = "",
)

/** Lo que devuelve `POST /assist/act` cuando la acción corrió. */
@Serializable
data class ResultadoAccion(
    val action: String = "",
    /** Confirmación en español de lo que quedó hecho. */
    val text: String = "",
    val data: JsonObject? = null,
)

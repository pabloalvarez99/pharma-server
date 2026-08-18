package cl.rutbusiness.app.ui.gastos

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * "Lo que gastaste": la plata que sale del puesto — arriendo, flete, bolsas —
 * la otra mitad de la ganancia que la ola 29 (ventas) no medía.
 *
 * Mismo patrón que `VentasApi.kt` y `FiadoApi.kt`: `ApiFactory`, [Resultado],
 * `llamar {}`, `exigirExito`. Ninguna llamada lanza; todas vuelven en
 * [Resultado].
 *
 * Los DTOs son el espejo de `crates/domain/src/expenses/model.rs`. **Regla de
 * plata: acá no se suma, no se promedia, no se resta ni un peso.** Los montos
 * viajan como texto decimal y así se guardan, se comparan con
 * [cl.rutbusiness.core.money.Dinero] y se muestran con
 * [cl.rutbusiness.core.money.Moneda]. Este endpoint no trae un total del
 * período — si hace falta uno, se le pide al agente, no se suma acá.
 */
class GastosApi(private val api: ApiFactory) {

    /**
     * Los gastos del rango pedido: `GET /api/v1/expenses?from=&to=&limit=`.
     *
     * El server devuelve un **array**, no un sobre. El rango se arma siempre
     * sobre `incurred_at` (cuándo se gastó), nunca `created_at` (cuándo se
     * anotó) — eso lo decide quien llama, acá sólo se mandan los parámetros.
     */
    suspend fun gastos(desde: String, hasta: String, limite: Int): Resultado<List<GastoDto>> =
        llamar(api) {
            api.http.get("${api.baseUrl}/api/v1/expenses") {
                url {
                    parameters.append("from", desde)
                    parameters.append("to", hasta)
                    parameters.append("limit", limite.toString())
                }
            }.exigirExito(api.baseUrl).body()
        }

    /**
     * Anotar un gasto nuevo: `POST /api/v1/expenses`.
     *
     * `category` no puede ir vacía (400 "category requerido" del server).
     * `payment_method` se manda siempre explícito, aunque el server por
     * defecto lo tome como "cash" si se omite.
     */
    suspend fun anotar(gasto: NuevoGasto): Resultado<GastoDto> = llamar(api) {
        api.http.post("${api.baseUrl}/api/v1/expenses") {
            contentType(ContentType.Application.Json)
            setBody(gasto)
        }.exigirExito(api.baseUrl).body()
    }
}

// --- lo que se le manda al server -------------------------------------------

@Serializable
data class NuevoGasto(
    val category: String,
    val description: String,
    /** Texto decimal, tal como lo escribió la dueña, normalizado. */
    val amount: String,
    @SerialName("payment_method") val paymentMethod: String,
    @SerialName("cash_session") val cashSession: String? = null,
    val supplier: String? = null,
    val note: String? = null,
    /** `null` = ahora, según lo resuelve el server. */
    @SerialName("incurred_at") val incurredAt: String? = null,
)

// --- lo que contesta el server ----------------------------------------------

/** Una fila de `GET /api/v1/expenses` o la respuesta de `POST /api/v1/expenses`. */
@Serializable
data class GastoDto(
    val id: String = "",
    val category: String = "",
    val description: String = "",
    val amount: String = "0",
    @SerialName("payment_method") val paymentMethod: String = "cash",
    @SerialName("cash_session") val cashSession: String? = null,
    val supplier: String? = null,
    val note: String? = null,
    @SerialName("created_by") val createdBy: String? = null,
    /** Cuándo se gastó de verdad. Se usa para agrupar y para filtrar por rango. */
    @SerialName("incurred_at") val incurredAt: String = "",
    /** Cuándo se anotó en la app. Nunca se usa para agrupar ni filtrar. */
    @SerialName("created_at") val createdAt: String = "",
)

package cl.rutbusiness.core.pos

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.header
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import kotlin.random.Random

/**
 * Cobrar.
 *
 * Los dos endpoints van escritos a mano y no salen del generador:
 * `POST /api/v1/pos/sale` declara su body como `serde_json::Value` en el spec
 * (el generador lo baja a `JsonElement`, que no ayuda), y
 * `GET /api/v1/orders/{id}/receipt` directamente **no está anotado** con
 * `#[utoipa::path]`, así que no aparece en el spec. Los tipos de acá son el
 * espejo de `crates/domain/src/sales/model.rs`.
 */
class PosRepository(private val api: ApiFactory) {

    /**
     * Manda la venta.
     *
     * [claveDeIdempotencia] es lo que impide cobrar dos veces. El server cachea
     * la respuesta por 24 h contra esa clave: el primer POST contesta 201, y
     * cualquier repetición con la misma clave contesta 200 con **la misma
     * orden**, sin tocar stock ni caja. Por eso la clave es de la *intención de
     * cobro*, no de la llamada HTTP: un doble toque y un reintento por red
     * cortada tienen que mandar la misma.
     */
    suspend fun vender(
        solicitud: SolicitudDeVenta,
        claveDeIdempotencia: String,
    ): Resultado<RespuestaDeVenta> = llamar(api) {
        api.http.post("${api.baseUrl}/api/v1/pos/sale") {
            contentType(ContentType.Application.Json)
            header("Idempotency-Key", claveDeIdempotencia)
            setBody(solicitud)
        }.exigirExito(api.baseUrl).body()
    }

    /** El comprobante, con el vuelto ya calculado por el server. */
    suspend fun comprobante(ordenId: String): Resultado<ComprobanteDto> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/orders/$ordenId/receipt")
            .exigirExito(api.baseUrl)
            .body()
    }

    companion object {
        /**
         * Clave de idempotencia nueva.
         *
         * 128 bits de aleatorio en hexa. No se usa `java.util.UUID` porque esto
         * es `commonMain` y tiene que compilar igual para iOS.
         */
        fun nuevaClave(): String = buildString(32) {
            repeat(32) { append(HEXA[Random.nextInt(16)]) }
        }

        private const val HEXA = "0123456789abcdef"
    }
}

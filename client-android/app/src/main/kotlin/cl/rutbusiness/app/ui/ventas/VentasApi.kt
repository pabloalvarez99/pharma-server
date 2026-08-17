package cl.rutbusiness.app.ui.ventas

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
import io.ktor.http.encodeURLPathPart
import kotlinx.serialization.EncodeDefault
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * "Lo que vendiste": las ventas de hoy, su detalle, y deshacer una entera.
 *
 * Mismo patrón que `FiadoApi.kt`: `ApiFactory`, [Resultado], `llamar {}`,
 * `exigirExito`. Ninguna llamada lanza; todas vuelven en [Resultado].
 *
 * Los DTOs son el espejo de `crates/domain/src/sales/model.rs`. Sólo se
 * declaran los campos que esta pantalla usa, todos con valor por defecto,
 * para que un campo nuevo del server no rompa el parseo de una versión vieja
 * de la app instalada en el teléfono.
 *
 * **Regla de plata: acá no se suma, no se promedia, no se resta ni un peso.**
 * Los montos viajan como texto decimal y así se guardan, se comparan con
 * [cl.rutbusiness.core.money.Dinero] y se muestran con
 * [cl.rutbusiness.core.money.Moneda]. Nunca `Double`, nunca `toInt()`.
 */
class VentasApi(private val api: ApiFactory) {

    /**
     * Las ventas del rango pedido, de más nueva a más vieja:
     * `GET /api/v1/orders?from=&to=&limit=`.
     *
     * El server devuelve un **array**, no un sobre.
     */
    suspend fun ventas(desde: String, hasta: String, limite: Int): Resultado<List<VentaDto>> =
        llamar(api) {
            api.http.get("${api.baseUrl}/api/v1/orders") {
                url {
                    parameters.append("from", desde)
                    parameters.append("to", hasta)
                    parameters.append("limit", limite.toString())
                }
            }.exigirExito(api.baseUrl).body()
        }

    /**
     * Qué llevaba una venta: `GET /api/v1/orders/{id}`.
     *
     * El id de la venta viene como `order:abc123`, con dos puntos adentro, así
     * que se codifica al armar la URL — de lo contrario el server vería dos
     * segmentos de ruta donde va uno solo.
     */
    suspend fun detalle(ventaId: String): Resultado<DetalleDeVentaDto> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/orders/${ventaId.encodeURLPathPart()}")
            .exigirExito(api.baseUrl)
            .body()
    }

    /**
     * Deshacer una venta entera: `POST /api/v1/pos/returns`.
     *
     * `motivo` no puede ir vacío (400 "motivo requerido" del server) ni
     * `items` (400 "items requeridos"). Los items van con el `unit_price` y la
     * `quantity` tal como los mandó [detalle] — acá no se recompone el precio
     * dividiendo el subtotal.
     */
    suspend fun deshacer(devolucion: NuevaDevolucion): Resultado<DevolucionDto> = llamar(api) {
        api.http.post("${api.baseUrl}/api/v1/pos/returns") {
            contentType(ContentType.Application.Json)
            setBody(devolucion)
        }.exigirExito(api.baseUrl).body()
    }
}

// --- lo que se le manda al server -------------------------------------------

@OptIn(ExperimentalSerializationApi::class)
@Serializable
data class NuevaDevolucion(
    val order: String,
    @EncodeDefault val tipo: String = "devolucion",
    val motivo: String,
    val items: List<ItemDeDevolucion>,
    @SerialName("metodo_reembolso") val metodoDeReembolso: String? = null,
)

@OptIn(ExperimentalSerializationApi::class)
@Serializable
data class ItemDeDevolucion(
    val product: String? = null,
    @SerialName("product_name") val nombreDelProducto: String,
    val quantity: Long,
    @SerialName("unit_price") val precioUnitario: String,
    @EncodeDefault val restock: Boolean = true,
)

// --- lo que contesta el server ----------------------------------------------

/** Una fila de `GET /api/v1/orders`. */
@Serializable
data class VentaDto(
    val id: String = "",
    val status: String = "",
    @SerialName("payment_method") val metodoDePago: String = "",
    val total: String = "0",
    @SerialName("refunded_total") val totalDevuelto: String = "0",
    @SerialName("customer_name") val nombreDelCliente: String? = null,
    val notes: String? = null,
    @SerialName("created_at") val creadaEn: String = "",
) {
    /**
     * Si esta venta ya está anulada entera.
     *
     * El server define `refunded` exactamente como `refunded_total >= total`;
     * acá se lee el status, nunca se resta [totalDevuelto] de [total] para
     * deducirlo — eso sería recalcular plata que el server ya decidió.
     */
    val estaDevuelta: Boolean get() = status == "refunded"
}

/** `GET /api/v1/orders/{id}`: el sobre con la venta y sus líneas. */
@Serializable
data class DetalleDeVentaDto(
    val order: VentaDto = VentaDto(),
    val items: List<ItemDeVentaDto> = emptyList(),
)

@Serializable
data class ItemDeVentaDto(
    val id: String = "",
    val product: String? = null,
    @SerialName("product_name") val nombreDelProducto: String = "",
    val quantity: Long = 0,
    @SerialName("unit_price") val precioUnitario: String = "0",
    val subtotal: String = "0",
)

/** Respuesta de `POST /api/v1/pos/returns`. */
@Serializable
data class DevolucionDto(
    @SerialName("order_marked_refunded") val ventaQuedoAnulada: Boolean = false,
)

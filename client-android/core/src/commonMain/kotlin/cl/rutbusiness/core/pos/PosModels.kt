package cl.rutbusiness.core.pos

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Los medios de pago que acepta el mostrador.
 *
 * Los códigos son los de `POS_METHODS` en `crates/domain/src/sales/model.rs`.
 * Si el server suma uno, esto no compila hasta que se agregue acá — que es
 * exactamente lo que queremos: un medio de pago que la app no conoce no puede
 * quedar mudo.
 */
enum class MedioDePago(val codigo: String, val etiqueta: String) {
    /** Billetes. Pide con cuánto paga y el server devuelve el vuelto. */
    Efectivo("pos_cash", "Efectivo"),

    /** "Te hago la transferencia": liquida exacto, no entra al efectivo del arqueo. */
    Transferencia("pos_transferencia", "Transferencia"),

    /** Cuenta corriente del cliente: no mueve caja y exige cliente. */
    Fiado("pos_fiado", "Fiado"),
    ;

    /** Sólo el efectivo tiene vuelto, así que sólo él pide "¿con cuánto paga?". */
    val pideMontoEntregado: Boolean get() = this == Efectivo

    /** Fiado sin cliente es deuda de nadie: el server lo rechaza con 400. */
    val exigeCliente: Boolean get() = this == Fiado
}

@Serializable
data class LineaDeVenta(
    val product: String,
    @SerialName("product_name") val productName: String,
    val quantity: Int,
    @SerialName("unit_price") val unitPrice: String,
)

@Serializable
data class SolicitudDeVenta(
    val items: List<LineaDeVenta>,
    @SerialName("payment_method") val paymentMethod: String,
    @SerialName("cash_amount") val cashAmount: String? = null,
    val customer: String? = null,
)

@Serializable
data class OrdenDto(
    val id: String,
    val status: String,
    @SerialName("payment_method") val paymentMethod: String,
    val total: String,
)

@Serializable
data class RespuestaDeVenta(
    val order: OrdenDto,
    @SerialName("loyalty_points_awarded") val loyaltyPointsAwarded: Long = 0,
)

@Serializable
data class LineaDeComprobante(
    val name: String,
    val qty: Long,
    @SerialName("unit_price") val unitPrice: String,
    @SerialName("line_total") val lineTotal: String,
)

/**
 * El comprobante que ve el cajero después de cobrar.
 *
 * Viene de `GET /api/v1/orders/{id}/receipt` y **todos** sus montos los calculó
 * el server, incluido `change`. La app no suma ni resta un peso acá.
 */
@Serializable
data class ComprobanteDto(
    @SerialName("order_id") val orderId: String,
    @SerialName("folio_or_number") val folio: String,
    @SerialName("tenant_name") val tenantName: String,
    val items: List<LineaDeComprobante> = emptyList(),
    val subtotal: String,
    val discount: String,
    val total: String,
    @SerialName("payment_method") val paymentMethod: String,
    @SerialName("cash_amount") val cashAmount: String? = null,
    /** Vuelto. `null` cuando el medio de pago no tiene (transferencia, fiado). */
    val change: String? = null,
    @SerialName("loyalty_points_awarded") val loyaltyPointsAwarded: Long = 0,
    @SerialName("footer_note") val footerNote: String = "",
)

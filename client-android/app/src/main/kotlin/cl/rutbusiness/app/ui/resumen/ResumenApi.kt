package cl.rutbusiness.app.ui.resumen

import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.net.exigirExito
import cl.rutbusiness.core.net.llamar
import io.ktor.client.call.body
import io.ktor.client.request.get
import io.ktor.client.request.parameter
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * De dónde sale el resumen del día.
 *
 * Todo lo que se muestra ya está calculado en el server. Esta clase pide y
 * transporta; no suma, no promedia, no proyecta. La regla de plata del encargo
 * no admite matices: la moneda y los decimales son por tenant, y un total
 * armado en el teléfono es un número distinto del que se cobró.
 *
 * Van a mano y no por el generador de OpenAPI por lo mismo que
 * [cl.rutbusiness.core.pos.PosRepository]: los handlers declaran sus bodies
 * como `serde_json::Value` (`crates/api/src/v1/dashboard.rs`) y el generador
 * baja eso a `JsonElement`. Los tipos de acá son el espejo de
 * `crates/domain/src/expenses/model.rs`, `.../credit/model.rs` y
 * `.../cash_register/model.rs`.
 *
 * Ninguna llamada lanza: todas vuelven en [Resultado], y la pantalla decide
 * qué bloque se cae sin arrastrar a los demás.
 */
class ResumenApi(private val api: ApiFactory) {

    /**
     * El agregado ejecutivo: `GET /api/v1/reports/dashboard`.
     *
     * Una sola llamada trae ventas de hoy, inventario y lotes por vencer, con
     * las cifras compuestas server-side. **Pide rol admin/owner/quimico**: un
     * cajero recibe 403, y eso no es un error a esconder sino la respuesta
     * correcta, que la pantalla muestra como tal.
     */
    suspend fun dashboard(): Resultado<DashboardDto> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/reports/dashboard")
            .exigirExito(api.baseUrl)
            .body()
    }

    /**
     * Ventas por día en un rango: `GET /api/v1/reports/sales-daily`.
     *
     * Se usa para traer **ayer**, que es la única cifra que el agregado no
     * incluye. Devuelve una fila por día con su `revenue` ya sumado por el
     * server; si ese día no hubo ventas, no hay fila.
     */
    suspend fun ventasDiarias(desde: String, hasta: String): Resultado<List<VentaDiariaDto>> =
        llamar(api) {
            api.http.get("${api.baseUrl}/api/v1/reports/sales-daily") {
                parameter("from", desde)
                parameter("to", hasta)
            }.exigirExito(api.baseUrl).body()
        }

    /** Lo que le deben al negocio: `GET /api/v1/reports/por-cobrar`. */
    suspend fun porCobrar(): Resultado<PorCobrarDto> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/reports/por-cobrar")
            .exigirExito(api.baseUrl)
            .body()
    }

    /** Cajas abiertas ahora mismo: `GET /api/v1/cash-sessions?status=open`. */
    suspend fun cajasAbiertas(): Resultado<List<CajaDto>> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/cash-sessions") {
            parameter("status", "open")
            parameter("limit", 5)
        }.exigirExito(api.baseUrl).body()
    }

    /**
     * Cuánta plata debería haber en una caja abierta:
     * `GET /api/v1/cash-sessions/{id}/arqueo`.
     *
     * El server contesta con `session.closing_cash_expected` ya resuelto
     * (apertura + ventas en efectivo + entradas - salidas). Ese cálculo vive en
     * `domain::cash_register::service::compute_summary` y **no** se rehace acá:
     * un peso de diferencia entre lo que dice el teléfono y lo que dice el
     * arqueo es un cuadre que no cierra.
     */
    suspend fun arqueo(cajaId: String): Resultado<ArqueoDto> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/cash-sessions/$cajaId/arqueo")
            .exigirExito(api.baseUrl)
            .body()
    }

    /** Lotes que se vencen pronto: `GET /api/v1/reports/near-expiry`. */
    suspend fun porVencer(dias: Int = DIAS_DE_VENCIMIENTO): Resultado<List<LotePorVencerDto>> =
        llamar(api) {
            api.http.get("${api.baseUrl}/api/v1/reports/near-expiry") {
                parameter("days", dias)
            }.exigirExito(api.baseUrl).body()
        }

    /**
     * Los productos que se están por acabar: `GET /api/v1/products?low_stock=`.
     *
     * El agregado del dashboard trae **cuántos** son, pero no cuáles, y "se te
     * están acabando 7 productos" sin nombres no le sirve a nadie para salir a
     * comprar. El umbral que se manda es el mismo con el que el server contó
     * ([UMBRAL_STOCK_BAJO]), para que el número y la lista no se contradigan.
     */
    suspend fun stockBajo(limite: Int = MAXIMO_EN_LISTA): Resultado<List<ProductDto>> =
        llamar(api) {
            api.http.get("${api.baseUrl}/api/v1/products") {
                parameter("low_stock", UMBRAL_STOCK_BAJO)
                parameter("active", true)
                parameter("limit", limite)
            }.exigirExito(api.baseUrl).body()
        }

    companion object {
        /**
         * El mismo `LOW_STOCK_DEFAULT` de
         * `crates/domain/src/catalog/model.rs`, que es con el que el dashboard
         * calcula `inventario.low_stock`. Está duplicado acá porque el endpoint
         * exige el umbral explícito y no hay forma de decirle "el tuyo". Si el
         * server lo cambia, este número se queda viejo: se marca así para que
         * el día que exista `/reports/dashboard` con los nombres adentro, esto
         * se borre.
         */
        const val UMBRAL_STOCK_BAJO = 5

        /** La ventana que usa el propio dashboard para `por_vencer`. */
        const val DIAS_DE_VENCIMIENTO = 30

        /**
         * Cuántos ítems se listan por bloque.
         *
         * Un resumen no es un inventario: si hay treinta productos por acabarse
         * la respuesta no es scrollear treinta filas, es ir a Productos. Tres
         * nombres alcanzan para saber de qué se trata.
         */
        const val MAXIMO_EN_LISTA = 3
    }
}

// --- lo que contesta el server ---------------------------------------------
//
// Todos los montos viajan como texto: `rust_decimal` se serializa con
// `serde(with = "rust_decimal::serde::str")` justo para que nadie los meta en
// un `Double` en el camino. Acá se reciben como `String` y se entregan tal cual
// a `Moneda.formatear`, que es lo único que los convierte en algo legible.

/** `GET /api/v1/reports/dashboard`. */
@Serializable
data class DashboardDto(
    @SerialName("ventas_hoy") val ventasHoy: VentasDto = VentasDto(),
    val inventario: InventarioDto = InventarioDto(),
    /** Cuántos lotes vencen dentro de los próximos 30 días (o ya vencieron). */
    @SerialName("por_vencer") val porVencer: Int = 0,
)

@Serializable
data class VentasDto(
    /** Cuántas boletas. */
    val orders: Long = 0,
    /** Lo vendido, en texto decimal del server. */
    val revenue: String = "0",
    val cash: String = "0",
    val card: String = "0",
)

@Serializable
data class InventarioDto(
    @SerialName("low_stock") val stockBajo: Int = 0,
    @SerialName("out_of_stock") val sinStock: Int = 0,
)

/** Una fila de `GET /api/v1/reports/sales-daily`. */
@Serializable
data class VentaDiariaDto(
    /** `YYYY-MM-DD` en UTC. */
    val date: String = "",
    val orders: Long = 0,
    val revenue: String = "0",
)

/** `GET /api/v1/reports/por-cobrar`. */
@Serializable
data class PorCobrarDto(
    @SerialName("total_por_cobrar") val total: String = "0",
    @SerialName("debtor_count") val deudores: Int = 0,
)

/** Una caja de `GET /api/v1/cash-sessions`. */
@Serializable
data class CajaDto(
    val id: String = "",
    @SerialName("register_name") val nombre: String = "",
    val status: String = "",
)

/** `GET /api/v1/cash-sessions/{id}/arqueo`. */
@Serializable
data class ArqueoDto(val session: CajaSesionDto = CajaSesionDto())

@Serializable
data class CajaSesionDto(
    @SerialName("register_name") val nombre: String = "",
    /**
     * Lo que **debería** haber en el cajón ahora, calculado por el server. En
     * una caja abierta el arqueo lo rellena; en una cerrada trae el del cierre.
     */
    @SerialName("closing_cash_expected") val esperado: String? = null,
)

/** Una fila de `GET /api/v1/reports/near-expiry`. */
@Serializable
data class LotePorVencerDto(
    @SerialName("product_name") val producto: String = "",
    /** Días enteros hasta el vencimiento. Negativo = ya venció. */
    @SerialName("days_to_expiry") val diasParaVencer: Long = 0,
    val expired: Boolean = false,
    val stock: Long = 0,
)

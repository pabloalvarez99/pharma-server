package cl.rutbusiness.core.reports

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
 * La casa única de los reportes de venta: qué se vende más, cómo entra la
 * plata, márgenes y rotación de stock.
 *
 * Nace antes que las pantallas que la van a usar, para que la ola 29 (top
 * productos), la 30 (cómo te pagan), la 31 (márgenes) y la que siga con
 * rotación no reinventen cada una su propio cliente a mano en su propio
 * paquete de UI, como pasó con [cl.rutbusiness.app.ui.resumen.ResumenApi]. Un
 * solo lugar que sabe que la plata viaja como texto es un lugar donde esa
 * regla no se puede olvidar por pantalla nueva.
 *
 * Van a mano y no por el generador de OpenAPI por el mismo motivo que
 * `ResumenApi`: los handlers de `crates/api/src/v1/expenses.rs` declaran sus
 * bodies como `serde_json::Value` (ver `#[utoipa::path]` de cada uno), así que
 * el generador baja eso a `JsonElement` inútil. Los tipos de acá son el
 * espejo de `crates/domain/src/expenses/model.rs`.
 *
 * Ninguna llamada lanza: todas vuelven en [Resultado], y la pantalla decide
 * qué bloque se cae sin arrastrar a los demás.
 */
class ReportsApi(private val api: ApiFactory) {

    /**
     * Ranking de productos por venta: `GET /api/v1/reports/top-products`.
     *
     * `abc_class` (A/B/C) es jerga de inventario — el Pareto de a qué le
     * conviene prestarle atención, calculado sobre el ranking completo antes
     * de aplicar [limite] — y **nunca se muestra tal cual** en una pantalla de
     * feria. Sirve para ordenar o para pesar visualmente, no para pintar como
     * texto.
     *
     * [TopProductoDto.productId] viene `null` cuando la venta no quedó
     * enlazada a un producto del catálogo (por ejemplo, un ítem libre); el
     * nombre sigue disponible en [TopProductoDto.productName] igual.
     */
    suspend fun topProductos(
        desde: String? = null,
        hasta: String? = null,
        limite: Int = LIMITE_TOP_PRODUCTOS_DEFECTO,
    ): Resultado<List<TopProductoDto>> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/reports/top-products") {
            if (desde != null) parameter("from", desde)
            if (hasta != null) parameter("to", hasta)
            parameter("limit", limite)
        }.exigirExito(api.baseUrl).body()
    }

    /**
     * Ingresos del período por método de pago:
     * `GET /api/v1/reports/sales-by-method`.
     *
     * Dos cosas que hay que saber antes de sumar esta lista:
     *
     * 1. Una venta mixta (parte efectivo, parte tarjeta) aporta monto a los
     *    dos buckets, y por eso cuenta en el `orders` de **ambos**. Sumar
     *    `orders` de todas las filas y llamarlo "ventas del período" da un
     *    número inflado que no calza con el conteo real de boletas.
     * 2. El bucket `fiado` va incluido en el total de `amount` de esta lista:
     *    es ingreso devengado, no plata que ya está en la caja. "Te entró" y
     *    "te deben" no son lo mismo, y sumarlos sin decirlo es una mentira
     *    sobre plata.
     *
     * La copia de pantalla se elige por [VentaPorMetodoDto.method] (el bucket
     * estable: `efectivo`|`tarjeta`|`transferencia`|`fiado`|`otro`), nunca por
     * [VentaPorMetodoDto.label]. El `label` es el respaldo en español que
     * manda el server para un bucket que el cliente no conozca todavía, no la
     * fuente de la frase de feria.
     */
    suspend fun ventasPorMetodo(
        desde: String? = null,
        hasta: String? = null,
    ): Resultado<List<VentaPorMetodoDto>> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/reports/sales-by-method") {
            if (desde != null) parameter("from", desde)
            if (hasta != null) parameter("to", hasta)
        }.exigirExito(api.baseUrl).body()
    }

    /**
     * Márgenes por día: `GET /api/v1/reports/margins-daily`.
     *
     * Ya no está detrás de licencia (dejó de estarlo el 2026-08-17): todos
     * los reportes sobre los datos del propio negocio son Free.
     *
     * [MargenDiarioDto.itemsWithoutCost] mayor a cero significa que el margen
     * de esa fila es **parcial**: hay ítems vendidos cuyo producto no tiene
     * costo cargado, y esos quedan fuera de `cost`. Mostrar el margen sin
     * decir que es parcial es una mentira chica. Hoy este número va a salir
     * alto seguido, porque `crear_producto_rapido` (la alta rápida del
     * agente) no pide costo — eso lo arregla la captura de costo, no esta
     * pantalla.
     */
    suspend fun margenesDiarios(
        desde: String? = null,
        hasta: String? = null,
    ): Resultado<List<MargenDiarioDto>> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/reports/margins-daily") {
            if (desde != null) parameter("from", desde)
            if (hasta != null) parameter("to", hasta)
        }.exigirExito(api.baseUrl).body()
    }

    /**
     * Rotación de stock: `GET /api/v1/reports/stock-rotation`.
     *
     * [RotacionStockDto.turnover] y [RotacionStockDto.daysOfInventory] vienen
     * `null` cuando el stock actual es ≤ 0 (no se puede dividir por cero). Esos
     * productos igual aparecen en la lista con su `qty_sold`, para que un
     * quiebre de stock de algo que se vende rápido no desaparezca del
     * reporte. Es un `null` de verdad, no un cero: pintarlo como "0.0" de
     * rotación diría que el producto dejó de venderse, cuando lo que pasó es
     * que se agotó.
     */
    suspend fun rotacionStock(
        desde: String? = null,
        hasta: String? = null,
    ): Resultado<List<RotacionStockDto>> = llamar(api) {
        api.http.get("${api.baseUrl}/api/v1/reports/stock-rotation") {
            if (desde != null) parameter("from", desde)
            if (hasta != null) parameter("to", hasta)
        }.exigirExito(api.baseUrl).body()
    }

    companion object {
        /** El mismo default que documenta `TopProductsFilters::limit` en el server. */
        const val LIMITE_TOP_PRODUCTOS_DEFECTO = 50
    }
}

// --- lo que contesta el server ---------------------------------------------
//
// Todos los montos viajan como texto: `rust_decimal` se serializa con
// `serde(with = "rust_decimal::serde::str")` (o `str_option` para los
// nullable) justo para que nadie los meta en un `Double` en el camino. Acá se
// reciben como `String` y se entregan tal cual a `Moneda.formatear`; nada acá
// suma, promedia ni recalcula un porcentaje que el server ya resolvió.

/** Una fila de `GET /api/v1/reports/top-products`. */
@Serializable
data class TopProductoDto(
    val rank: Long = 0,
    @SerialName("product_id") val productId: String? = null,
    @SerialName("product_name") val productName: String = "",
    @SerialName("qty_sold") val qtySold: Long = 0,
    val revenue: String = "0",
    @SerialName("revenue_pct") val revenuePct: String = "0",
    /**
     * Bucket de Pareto ("A"|"B"|"C") sobre el ranking completo. Jerga de
     * inventario: sirve para ordenar y pesar visualmente, nunca para pintarlo
     * tal cual en una pantalla de feria.
     */
    @SerialName("abc_class") val abcClass: String = "",
)

/** Una fila de `GET /api/v1/reports/sales-by-method`. */
@Serializable
data class VentaPorMetodoDto(
    /** Bucket estable para máquinas: `efectivo`|`tarjeta`|`transferencia`|`fiado`|`otro`. */
    val method: String = "",
    /** Etiqueta es-CL del server; respaldo para un bucket que el cliente no conozca. */
    val label: String = "",
    /** Ventas que aportaron a este bucket; una venta mixta cuenta en dos filas. */
    val orders: Long = 0,
    val amount: String = "0",
)

/** Una fila de `GET /api/v1/reports/margins-daily`. */
@Serializable
data class MargenDiarioDto(
    /** `YYYY-MM-DD` en UTC. */
    val date: String = "",
    val revenue: String = "0",
    val cost: String = "0",
    val margin: String = "0",
    @SerialName("margin_pct") val marginPct: String = "0",
    /** Mayor a cero: el margen de esta fila es parcial, no calza con la venta completa. */
    @SerialName("items_without_cost") val itemsWithoutCost: Long = 0,
)

/** Una fila de `GET /api/v1/reports/stock-rotation`. */
@Serializable
data class RotacionStockDto(
    @SerialName("product_id") val productId: String = "",
    @SerialName("product_name") val productName: String = "",
    @SerialName("qty_sold") val qtySold: Long = 0,
    @SerialName("current_stock") val currentStock: Long = 0,
    /** `null` cuando `current_stock` es ≤ 0: no es cero, es "no se puede calcular". */
    val turnover: String? = null,
    /** `null` en las mismas condiciones que [turnover]. */
    @SerialName("days_of_inventory") val daysOfInventory: String? = null,
)

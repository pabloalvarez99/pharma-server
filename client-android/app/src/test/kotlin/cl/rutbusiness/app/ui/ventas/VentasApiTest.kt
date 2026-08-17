package cl.rutbusiness.app.ui.ventas

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.ReporteDeRed
import cl.rutbusiness.core.net.Resultado
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.client.request.HttpRequestData
import io.ktor.http.HttpStatusCode
import io.ktor.http.content.TextContent
import io.ktor.http.headersOf
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `VentasApi` sin red: arma la URL de listar/detalle, parsea las dos formas
 * de respuesta, y arma el cuerpo del POST de devolución.
 */
class VentasApiTest {

    @Test
    fun `listar arma la URL con from, to y limit`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val api = apiQueResponde(pedidos, """[]""")

        runBlocking {
            VentasApi(api).ventas(desde = "2026-08-17T00:00:00Z", hasta = "2026-08-17T23:59:59Z", limite = 50)
        }

        val pedido = pedidos.single()
        assertTrue(pedido.url.encodedPath.endsWith("/api/v1/orders"))
        assertEquals("2026-08-17T00:00:00Z", pedido.url.parameters["from"])
        assertEquals("2026-08-17T23:59:59Z", pedido.url.parameters["to"])
        assertEquals("50", pedido.url.parameters["limit"])
    }

    @Test
    fun `listar parsea el array de ventas`() {
        val cuerpo = """
            [
              {
                "id": "order:abc123",
                "status": "completed",
                "payment_method": "cash",
                "total": "12000.00",
                "refunded_total": "0.00",
                "customer_name": null,
                "notes": null,
                "created_at": "2026-08-17T15:30:00Z"
              }
            ]
        """.trimIndent()
        val api = apiQueResponde(mutableListOf(), cuerpo)

        val r = runBlocking { VentasApi(api).ventas("a", "b", 50) }

        assertTrue("falló listar: $r", r is Resultado.Ok)
        val venta = (r as Resultado.Ok).valor.single()
        assertEquals("order:abc123", venta.id)
        assertEquals("completed", venta.status)
        assertEquals("cash", venta.metodoDePago)
        assertEquals("12000.00", venta.total)
        assertEquals("0.00", venta.totalDevuelto)
        assertTrue(!venta.estaDevuelta)
    }

    @Test
    fun `una venta refunded se lee por status, no restando montos`() {
        val cuerpo = """[{"id":"order:x","status":"refunded","total":"1000","refunded_total":"1000"}]"""
        val api = apiQueResponde(mutableListOf(), cuerpo)

        val r = runBlocking { VentasApi(api).ventas("a", "b", 50) } as Resultado.Ok
        assertTrue(r.valor.single().estaDevuelta)
    }

    @Test
    fun `detalle codifica el id con dos puntos en la URL`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val cuerpo = """{"order":{"id":"order:abc123"},"items":[]}"""
        val api = apiQueResponde(pedidos, cuerpo)

        runBlocking { VentasApi(api).detalle("order:abc123") }

        val pedido = pedidos.single()
        assertTrue(
            "la URL debe llevar el id codificado: ${pedido.url.encodedPath}",
            pedido.url.encodedPath.endsWith("/api/v1/orders/order%3Aabc123") ||
                pedido.url.encodedPath.endsWith("/api/v1/orders/order:abc123"),
        )
    }

    @Test
    fun `detalle parsea el sobre order mas items`() {
        val cuerpo = """
            {
              "order": {"id": "order:abc123", "status": "completed", "total": "3600"},
              "items": [
                {
                  "id": "item:1",
                  "product": "product:tomate",
                  "product_name": "Tomate",
                  "quantity": 2,
                  "unit_price": "800",
                  "subtotal": "1600"
                }
              ]
            }
        """.trimIndent()
        val api = apiQueResponde(mutableListOf(), cuerpo)

        val r = runBlocking { VentasApi(api).detalle("order:abc123") }

        assertTrue("falló detalle: $r", r is Resultado.Ok)
        val detalle = (r as Resultado.Ok).valor
        assertEquals("order:abc123", detalle.order.id)
        val item = detalle.items.single()
        assertEquals("Tomate", item.nombreDelProducto)
        assertEquals(2L, item.quantity)
        assertEquals("800", item.precioUnitario)
    }

    @Test
    fun `deshacer arma el cuerpo del POST tal como lo pide el server`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val cuerpo = """{"devolucion":{},"items":[],"stock_movements":[],"order_marked_refunded":true}"""
        val api = apiQueResponde(pedidos, cuerpo)

        val devolucion = NuevaDevolucion(
            order = "order:abc123",
            motivo = "La feriante deshizo la venta desde la app",
            items = listOf(
                ItemDeDevolucion(
                    product = "product:tomate",
                    nombreDelProducto = "Tomate",
                    quantity = 2,
                    precioUnitario = "800",
                    restock = true,
                ),
            ),
            metodoDeReembolso = "cash",
        )

        val r = runBlocking { VentasApi(api).deshacer(devolucion) }

        assertTrue("falló deshacer: $r", r is Resultado.Ok)
        assertTrue((r as Resultado.Ok).valor.ventaQuedoAnulada)

        val pedido = pedidos.single()
        assertTrue(pedido.url.encodedPath.endsWith("/api/v1/pos/returns"))
        val body = cuerpoJson(pedido)
        assertEquals("order:abc123", body["order"]?.jsonPrimitive?.content)
        assertEquals("devolucion", body["tipo"]?.jsonPrimitive?.content)
        assertEquals(
            "La feriante deshizo la venta desde la app",
            body["motivo"]?.jsonPrimitive?.content,
        )
        assertEquals("cash", body["metodo_reembolso"]?.jsonPrimitive?.content)
        val item = body["items"]?.jsonArray?.single() as JsonObject
        assertEquals("product:tomate", item["product"]?.jsonPrimitive?.content)
        assertEquals("Tomate", item["product_name"]?.jsonPrimitive?.content)
        assertEquals("2", item["quantity"]?.jsonPrimitive?.content)
        assertEquals("800", item["unit_price"]?.jsonPrimitive?.content)
        assertEquals("true", item["restock"]?.jsonPrimitive?.content)
    }

    private fun cuerpoJson(pedido: HttpRequestData): JsonObject {
        val texto = (pedido.body as? TextContent)?.text
        assertNotNull("el request salió sin cuerpo (${pedido.body::class})", texto)
        return ApiFactory.JSON.parseToJsonElement(texto!!) as JsonObject
    }

    private fun apiQueResponde(pedidos: MutableList<HttpRequestData>, cuerpo: String): ApiFactory =
        ApiFactory(
            "http://localhost:8080",
            ReporteDeRed.Nulo,
            MockEngine { pedido ->
                pedidos += pedido
                respond(
                    content = cuerpo,
                    status = HttpStatusCode.OK,
                    headers = headersOf("Content-Type", "application/json"),
                )
            },
        ) { null }
}

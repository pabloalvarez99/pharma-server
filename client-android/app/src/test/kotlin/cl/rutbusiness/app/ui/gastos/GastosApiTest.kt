package cl.rutbusiness.app.ui.gastos

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
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `GastosApi` sin red: arma la URL de listar con `from`/`to`/`limit`, parsea
 * el array de respuesta, y arma el cuerpo del POST de anotar.
 */
class GastosApiTest {

    @Test
    fun `listar arma la URL con from, to y limit`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val api = apiQueResponde(pedidos, """[]""")

        runBlocking {
            GastosApi(api).gastos(desde = "2026-08-01T00:00:00Z", hasta = "2026-08-17T23:59:59Z", limite = 200)
        }

        val pedido = pedidos.single()
        assertTrue(pedido.url.encodedPath.endsWith("/api/v1/expenses"))
        assertEquals("2026-08-01T00:00:00Z", pedido.url.parameters["from"])
        assertEquals("2026-08-17T23:59:59Z", pedido.url.parameters["to"])
        assertEquals("200", pedido.url.parameters["limit"])
    }

    @Test
    fun `listar parsea el array de gastos`() {
        val cuerpo = """
            [
              {
                "id": "expense:abc123",
                "category": "arriendo",
                "description": "Arriendo del puesto",
                "amount": "5000.00",
                "payment_method": "cash",
                "cash_session": null,
                "supplier": null,
                "note": null,
                "created_by": null,
                "incurred_at": "2026-08-17T09:00:00Z",
                "created_at": "2026-08-17T15:30:00Z"
              }
            ]
        """.trimIndent()
        val api = apiQueResponde(mutableListOf(), cuerpo)

        val r = runBlocking { GastosApi(api).gastos("a", "b", 200) }

        assertTrue("falló listar: $r", r is Resultado.Ok)
        val gasto = (r as Resultado.Ok).valor.single()
        assertEquals("expense:abc123", gasto.id)
        assertEquals("arriendo", gasto.category)
        assertEquals("Arriendo del puesto", gasto.description)
        assertEquals("5000.00", gasto.amount)
        assertEquals("cash", gasto.paymentMethod)
        assertEquals("2026-08-17T09:00:00Z", gasto.incurredAt)
        assertEquals("2026-08-17T15:30:00Z", gasto.createdAt)
    }

    @Test
    fun `anotar manda category, description, amount y payment_method`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val cuerpo = """
            {
              "id": "expense:nuevo",
              "category": "flete",
              "description": "Flete de la camioneta",
              "amount": "3000",
              "payment_method": "cash",
              "incurred_at": "2026-08-17T10:00:00Z",
              "created_at": "2026-08-17T10:00:00Z"
            }
        """.trimIndent()
        val api = apiQueResponde(pedidos, cuerpo)

        val nuevo = NuevoGasto(
            category = "flete",
            description = "Flete de la camioneta",
            amount = "3000",
            paymentMethod = "cash",
        )

        val r = runBlocking { GastosApi(api).anotar(nuevo) }

        assertTrue("falló anotar: $r", r is Resultado.Ok)
        assertEquals("expense:nuevo", (r as Resultado.Ok).valor.id)

        val pedido = pedidos.single()
        assertTrue(pedido.url.encodedPath.endsWith("/api/v1/expenses"))
        val body = cuerpoJson(pedido)
        assertEquals("flete", body["category"]?.jsonPrimitive?.content)
        assertEquals("cash", body["payment_method"]?.jsonPrimitive?.content)
        assertEquals("3000", body["amount"]?.jsonPrimitive?.content)
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

package cl.rutbusiness.app.ui.catalogo

import cl.rutbusiness.core.api.models.ProductDto
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.ReporteDeRed
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.rubro.PACK_FARMACIA
import cl.rutbusiness.core.rubro.PACK_FERIA
import cl.rutbusiness.core.rubro.PACK_OTRO
import cl.rutbusiness.core.rubro.RubroFeatures
import cl.rutbusiness.core.rubro.RubroPack
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
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Alta feria por ensure: path, body y cuándo se elige frente a create.
 *
 * Si "Agregar una cosa" en feria sigue pegando `POST /products`, el cashier
 * se come un 403 y al reintentar se duplican tomates. El ensure es cashier+
 * e idempotente por nombre en el server.
 */
class AsegurarProductoTest {

    @Test
    fun `feria y agent_home usan ensure`() {
        assertTrue(usaEnsure(PACK_FERIA))
        assertTrue(
            usaEnsure(
                RubroPack(
                    rubro = "minimarket",
                    features = RubroFeatures(agentHome = true),
                ),
            ),
        )
    }

    @Test
    fun `rubro feria sin agent_home igual usa ensure`() {
        assertTrue(
            usaEnsure(
                RubroPack(
                    rubro = "feria",
                    features = RubroFeatures(agentHome = false),
                ),
            ),
        )
    }

    @Test
    fun `farmacia y otro siguen con create`() {
        assertFalse(usaEnsure(PACK_FARMACIA))
        assertFalse(usaEnsure(PACK_OTRO))
    }

    @Test
    fun `asegurarProducto pega ensure con nombre y precio`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val api = apiQueResponde(pedidos)

        val r = runBlocking {
            asegurarProducto(api, nombre = "Tomates", precio = "2000")
        }

        assertTrue("falló ensure: $r", r is Resultado.Ok)
        assertEquals("Tomates", (r as Resultado.Ok).valor.name)

        val pedido = pedidos.single()
        assertEquals("POST", pedido.method.value)
        assertTrue(
            "ruta no es ensure: ${pedido.url.encodedPath}",
            pedido.url.encodedPath.endsWith("/api/v1/products/ensure"),
        )
        val body = cuerpo(pedido)
        assertEquals("Tomates", body["name"]?.jsonPrimitive?.content)
        assertEquals("2000", body["price"]?.jsonPrimitive?.content)
        assertEquals(
            "ensure no manda stock: lo decide el server",
            setOf("name", "price"),
            body.keys,
        )
    }

    @Test
    fun `crearProducto sigue en products y no en ensure`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val api = apiQueResponde(pedidos)

        runBlocking { crearProducto(api, nombre = "Paracetamol", precio = "1500") }

        val pedido = pedidos.single()
        assertEquals("/api/v1/products", pedido.url.encodedPath)
        assertFalse(pedido.url.encodedPath.contains("ensure"))
    }

    private fun cuerpo(pedido: HttpRequestData): JsonObject {
        val texto = (pedido.body as? TextContent)?.text
        assertNotNull("el request salió sin cuerpo (${pedido.body::class})", texto)
        return ApiFactory.JSON.parseToJsonElement(texto!!) as JsonObject
    }

    private fun apiQueResponde(pedidos: MutableList<HttpRequestData>): ApiFactory =
        ApiFactory(
            "http://localhost:8080",
            ReporteDeRed.Nulo,
            MockEngine { pedido ->
                pedidos += pedido
                val nombre = runCatching {
                    val texto = (pedido.body as? TextContent)?.text.orEmpty()
                    (ApiFactory.JSON.parseToJsonElement(texto) as? JsonObject)
                        ?.get("name")
                        ?.jsonPrimitive
                        ?.content
                }.getOrNull() ?: "Tomates"
                respond(
                    content = ApiFactory.JSON.encodeToString(producto("p1", nombre)),
                    status = HttpStatusCode.OK,
                    headers = headersOf("Content-Type", "application/json"),
                )
            },
        ) { "token-de-prueba" }

    private fun producto(id: String, nombre: String) = ProductDto(
        active = true,
        createdAt = "2026-08-09T00:00:00Z",
        id = id,
        name = nombre,
        physicalStock = false,
        prescriptionType = "none",
        price = "2000",
        slug = id,
        stock = 0L,
        updatedAt = "2026-08-09T00:00:00Z",
        attrs = JsonObject(mapOf("rb_simple" to kotlinx.serialization.json.JsonPrimitive(true))),
    )
}

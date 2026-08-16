package cl.rutbusiness.app.ui.alta

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
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Cuerpo de `crearNegocio`: en la nube el reintento de slug manda
 * `tenant_slug`; sin sugerencia no inventa el campo.
 */
class AltaApiTest {

    @Test
    fun `con slugSugerido en la nube el POST manda tenant_slug`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val api = apiQueResponde(pedidos)

        val r = runBlocking {
            crearNegocio(
                api = api,
                nombreDelNegocio = "Huevos de Marta",
                rubro = "feria",
                email = "marta@feria.cl",
                clave = "huevos2026",
                enLaNube = true,
                slugSugerido = "huevos-de-marta-2",
            )
        }

        assertTrue("falló alta: $r", r is Resultado.Ok)
        val pedido = pedidos.single()
        assertTrue(pedido.url.encodedPath.endsWith("/api/v1/alta"))
        val body = cuerpo(pedido)
        assertEquals("huevos-de-marta-2", body["tenant_slug"]?.jsonPrimitive?.content)
        assertEquals("Huevos de Marta", body["business_name"]?.jsonPrimitive?.content)
        assertEquals("feria", body["vertical"]?.jsonPrimitive?.content)
    }

    @Test
    fun `sin slugSugerido no manda tenant_slug`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val api = apiQueResponde(pedidos)

        runBlocking {
            crearNegocio(
                api = api,
                nombreDelNegocio = "Huevos de Marta",
                rubro = "feria",
                email = "marta@feria.cl",
                clave = "huevos2026",
                enLaNube = true,
            )
        }

        val body = cuerpo(pedidos.single())
        assertFalse("tenant_slug no debió ir: $body", body.containsKey("tenant_slug"))
    }

    @Test
    fun `setup local ignora slugSugerido`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val api = apiQueResponde(pedidos)

        runBlocking {
            crearNegocio(
                api = api,
                nombreDelNegocio = "Almacén Rosa",
                rubro = "minimarket",
                email = "rosa@almacen.cl",
                clave = "almacen1",
                enLaNube = false,
                slugSugerido = "no-debe-irse",
            )
        }

        val pedido = pedidos.single()
        assertTrue(pedido.url.encodedPath.endsWith("/api/v1/setup"))
        val body = cuerpo(pedido)
        assertFalse(body.containsKey("tenant_slug"))
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
                respond(
                    content = """{"token":"t","tenant_slug":"huevos-de-marta-2"}""",
                    status = HttpStatusCode.OK,
                    headers = headersOf("Content-Type", "application/json"),
                )
            },
        ) { null }
}

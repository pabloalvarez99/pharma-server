package cl.rutbusiness.app.ui.catalogo

import cl.rutbusiness.core.api.models.ProductDto
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
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Reconocer el centinela de los montos sueltos.
 *
 * Si falla, no falla poco: un centinela que no se reconoce se duplica en cada
 * cobro rápido, y uno que no se filtra aparece como una fila a $0 en "Lo que
 * vendo" y en el buscador de Cobrar, donde se agrega sin querer a una venta.
 */
class VentaSueltaTest {

    @Test
    fun `lo reconoce por la marca de attrs`() {
        val marcado = producto("p1", "Cualquier nombre", marcado = true)
        assertTrue(esVentaSuelta(marcado))
    }

    /**
     * La marca gana, y por eso existe: el nombre lo puede cambiar cualquiera
     * desde "Lo que vendo" o desde el ERP de escritorio, y sin la marca el
     * siguiente cobro rápido crearía un segundo centinela.
     */
    @Test
    fun `lo sigue reconociendo despues de que le cambien el nombre`() {
        val renombrado = producto("p1", "Varios", marcado = true)
        assertEquals(renombrado, ventaSueltaEn(listOf(producto("p0", "Tomate"), renombrado)))
    }

    /** Y por nombre, para uno creado antes de que existiera la marca. */
    @Test
    fun `lo reconoce por el nombre cuando no tiene marca`() {
        assertTrue(esVentaSuelta(producto("p1", NOMBRE_VENTA_SUELTA)))
        assertTrue(esVentaSuelta(producto("p1", "  venta suelta  ")))
    }

    @Test
    fun `un producto normal no es venta suelta`() {
        assertFalse(esVentaSuelta(producto("p1", "Tomate")))
        // Una marca puesta en false tampoco: `attrs` es JSON libre del server.
        assertFalse(esVentaSuelta(producto("p1", "Tomate", marcado = false)))
        assertNull(ventaSueltaEn(listOf(producto("p1", "Tomate"))))
    }

    /**
     * Y no se cae con basura en `attrs`.
     *
     * `attrs` lo escribe cualquier cliente del ERP. Una marca que llegó como
     * texto u objeto no puede tirar la pantalla de catálogo entera.
     */
    @Test
    fun `no se cae si la marca no es un booleano`() {
        val raro = producto("p1", "Tomate").copy(
            attrs = JsonObject(mapOf(CLAVE_VENTA_SUELTA to JsonPrimitive("si"))),
        )
        assertFalse(esVentaSuelta(raro))

        val anidado = producto("p1", "Tomate").copy(
            attrs = JsonObject(
                mapOf(CLAVE_VENTA_SUELTA to JsonObject(mapOf("x" to JsonPrimitive(1)))),
            ),
        )
        assertFalse(esVentaSuelta(anidado))
    }

    @Test
    fun `se saca de las listas que se muestran`() {
        val lista = listOf(
            producto("p0", "Tomate"),
            producto("p1", NOMBRE_VENTA_SUELTA, marcado = true),
            producto("p2", "Cilantro"),
        )

        val visible = sinVentaSuelta(lista)
        assertEquals(listOf("Tomate", "Cilantro"), visible.map { it.name })
    }

    // --- cómo nace el centinela ----------------------------------------------

    /**
     * **La prueba del encargo.** El centinela nace sin inventario y en cero.
     *
     * `physical_stock: false` es lo que hace que su stock no se mueva nunca: el
     * server saltea el chequeo y el descuento (`sales::service`), y además lo
     * deja fuera de las alertas de stock bajo y del tablero (migración 0031).
     * Sin el flag, un producto en cero que la dueña nunca cargó aparecería
     * gritando "sin stock" en su pantalla de inicio.
     *
     * Antes nacía con 100.000 unidades y cada venta le bajaba una, así que había
     * que reponerlo cada tanto — con una llamada que además daba 403 si quien
     * estaba en la caja no era admin. Si este assert se cae, ese parche vuelve.
     */
    @Test
    fun `el centinela se crea sin inventario y en cero`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val api = apiQueResponde(pedidos) { """[]""" }

        val r = runBlocking { asegurarVentaSuelta(api) }

        assertTrue("no se pudo crear el centinela: $r", r is Resultado.Ok)
        val creacion = pedidos.last { it.url.encodedPath.endsWith("/products") }
        val body = cuerpo(creacion)

        assertEquals(
            "sin physical_stock=false el centinela vuelve a ser un producto con stock",
            false,
            body["physical_stock"]?.jsonPrimitive?.booleanOrNull,
        )
        assertEquals(0, body["stock"]?.jsonPrimitive?.intOrNull)
        assertEquals(NOMBRE_VENTA_SUELTA, body["name"]?.jsonPrimitive?.content)
        // El precio del catálogo no se usa: cada línea lleva su propio monto.
        assertEquals("0", body["price"]?.jsonPrimitive?.content)
        // Y la marca, que es cómo se lo reconoce después de que le cambien el nombre.
        assertEquals(
            true,
            (body["attrs"] as? JsonObject)?.get(CLAVE_VENTA_SUELTA)?.jsonPrimitive?.booleanOrNull,
        )
    }

    /**
     * Un producto normal no arrastra el flag: se manda **ausente**, no `true`.
     *
     * Ausente deja decidir al DEFAULT del server, y por eso una app nueva le
     * puede hablar a un server que todavía no tiene el campo -y una vieja a uno
     * que sí- sin que ninguna de las dos cambie de comportamiento.
     */
    @Test
    fun `un producto normal no manda el flag`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val api = apiQueResponde(pedidos) { """[]""" }

        runBlocking { crearProducto(api, nombre = "Tomate", precio = "1500") }

        val body = cuerpo(pedidos.last())
        assertNull("mandar physical_stock=true fija el default en el cliente", body["physical_stock"])
        assertEquals(1, body["stock"]?.jsonPrimitive?.intOrNull)
    }

    /** Si ya existe, no se crea otro: se usa el que contestó el server. */
    @Test
    fun `si el server ya lo tiene no se crea de nuevo`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val api = apiQueResponde(pedidos) {
            ApiFactory.JSON.encodeToString(
                listOf(producto("p9", NOMBRE_VENTA_SUELTA, marcado = true)),
            )
        }

        val r = runBlocking { asegurarVentaSuelta(api) }

        assertTrue(r is Resultado.Ok)
        assertEquals("p9", (r as Resultado.Ok).valor.id)
        assertTrue(
            "se creó un segundo centinela habiendo uno",
            pedidos.none { it.method.value == "POST" },
        )
    }

    /**
     * Y si está en el teléfono, no se pregunta nada: la venta suelta funciona
     * sin señal, que es el punto entero del centinela.
     */
    @Test
    fun `con el centinela en el telefono no se toca la red`() {
        val pedidos = mutableListOf<HttpRequestData>()
        val api = apiQueResponde(pedidos) { """[]""" }
        val enElTelefono = listOf(producto("p9", NOMBRE_VENTA_SUELTA, marcado = true))

        val r = runBlocking { asegurarVentaSuelta(api, enElTelefono) }

        assertTrue(r is Resultado.Ok)
        assertTrue("se pidió a la red algo que ya estaba en el teléfono", pedidos.isEmpty())
    }

    /** El JSON que efectivamente viajó en el cuerpo del request. */
    private fun cuerpo(pedido: HttpRequestData): JsonObject {
        val texto = (pedido.body as? TextContent)?.text
        assertNotNull("el request salió sin cuerpo (${pedido.body::class})", texto)
        return ApiFactory.JSON.parseToJsonElement(texto!!) as JsonObject
    }

    /**
     * Cliente contra un motor de mentira que anota cada pedido.
     *
     * Anota el *request entero* y no sólo la ruta porque lo que define al
     * centinela está adentro del cuerpo, no en la URL.
     */
    private fun apiQueResponde(
        pedidos: MutableList<HttpRequestData>,
        busqueda: () -> String,
    ): ApiFactory = ApiFactory(
        "http://localhost:8080",
        ReporteDeRed.Nulo,
        MockEngine { pedido ->
            pedidos += pedido
            val cuerpo = when {
                pedido.method.value == "POST" ->
                    ApiFactory.JSON.encodeToString(producto("p1", NOMBRE_VENTA_SUELTA, marcado = true))
                else -> busqueda()
            }
            respond(
                content = cuerpo,
                status = if (pedido.method.value == "POST") {
                    HttpStatusCode.Created
                } else {
                    HttpStatusCode.OK
                },
                headers = headersOf("Content-Type", "application/json"),
            )
        },
    ) { "token-de-prueba" }

    private fun producto(id: String, nombre: String, marcado: Boolean? = null) = ProductDto(
        active = true,
        createdAt = "2026-08-09T00:00:00Z",
        id = id,
        name = nombre,
        physicalStock = true,
        prescriptionType = "none",
        price = "0",
        slug = id,
        stock = 100L,
        updatedAt = "2026-08-09T00:00:00Z",
        attrs = marcado?.let {
            JsonObject(mapOf(CLAVE_VENTA_SUELTA to JsonPrimitive(it)))
        },
    )
}

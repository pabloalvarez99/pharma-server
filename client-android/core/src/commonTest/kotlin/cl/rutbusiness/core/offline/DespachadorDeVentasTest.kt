package cl.rutbusiness.core.offline

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.SolicitudDeVenta
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.client.engine.mock.respondError
import io.ktor.http.HttpStatusCode
import io.ktor.http.headersOf
import io.ktor.utils.io.errors.IOException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest

/**
 * Lo que este archivo prueba es **la** promesa del encargo: se corta la red al
 * confirmar una venta, la venta queda guardada, y cuando vuelve la señal sale
 * sola **una sola vez**.
 *
 * La prueba de que no se duplica no es "la cola quedó vacía": es que el segundo
 * POST viaja con la **misma** `Idempotency-Key` que el primero. Ésa es la
 * cabecera con la que el server reconoce el reintento y contesta la orden que
 * ya creó en vez de crear otra (`crates/api/src/v1/sales.rs`: 201 la primera
 * vez, 200 con el payload cacheado en la repetición). Si la clave cambiara
 * entre intentos, el server vería dos ventas distintas y cobraría dos veces —
 * así que la clave es lo que se afirma.
 */
class DespachadorDeVentasTest {

    private val reloj = { 1_000_000L }

    private val solicitud = SolicitudDeVenta(
        items = listOf(
            LineaDeVenta(
                product = "product:arroz",
                productName = "Arroz Grado 1 kg",
                quantity = 2,
                unitPrice = "1490",
            ),
        ),
        paymentMethod = "pos_cash",
    )

    private val ventaCreada = """
        {"order":{"id":"order:abc","status":"paid","payment_method":"pos_cash","total":"2980"},
         "loyalty_points_awarded":0}
    """.trimIndent()

    /** Las claves de idempotencia que vio el "server", en orden. */
    private val clavesVistas = mutableListOf<String>()

    private fun despachadorCon(
        cola: ColaDeVentas,
        conexion: ConexionConElNegocio,
        enlace: MutableStateFlow<Boolean>,
        motor: MockEngine,
    ): DespachadorDeVentas = DespachadorDeVentas(
        cola = cola,
        conexion = conexion,
        hayEnlace = enlace,
        haySesion = MutableStateFlow(true),
        apiActiva = { ApiFactory("http://localhost:8080", conexion, motor) { "token-de-prueba" } },
        reloj = reloj,
    )

    private fun motor(respuestas: List<suspend (String?) -> Any>): MockEngine {
        var turno = 0
        return MockEngine { pedido ->
            val clave = pedido.headers["Idempotency-Key"]
            if (clave != null) clavesVistas += clave
            val respuesta = respuestas[turno.coerceAtMost(respuestas.lastIndex)]
            turno++
            when (val r = respuesta(clave)) {
                is HttpStatusCode -> respondError(r)
                is Throwable -> throw r
                else -> respond(
                    content = r as String,
                    status = if (turno == 1) HttpStatusCode.Created else HttpStatusCode.OK,
                    headers = headersOf("Content-Type", "application/json"),
                )
            }
        }
    }

    @Test
    fun `se corta la red, vuelve, y la venta sale con la misma clave`() = runTest {
        val disco = AlmacenDeMentira()
        val enlace = MutableStateFlow(true)
        val conexion = ConexionConElNegocio(enlace)

        // Primer intento: la red se corta en medio. Segundo: el server contesta.
        val motor = motor(
            listOf(
                { throw IOException("se cortó la red") },
                { ventaCreada },
            ),
        )

        val cola = ColaDeVentas(disco, reloj)
        cola.cargar()
        cola.encolar(
            VentaEnCola(
                clave = "clave-de-la-venta",
                solicitud = solicitud,
                cobradaEn = reloj(),
                lineas = 1,
            ),
        )

        val despachador = despachadorCon(cola, conexion, enlace, motor)

        despachador.intentarAhora()
        assertEquals(1, cola.ventas.value.size, "sin respuesta, la venta se queda")
        assertEquals(false, conexion.hayConexion.value, "y el cartel se prende")

        despachador.intentarAhora()

        assertEquals(0, cola.ventas.value.size, "con respuesta, la venta se va de la cola")
        assertEquals(true, conexion.hayConexion.value, "y el cartel se apaga solo")

        assertEquals(
            listOf("clave-de-la-venta", "clave-de-la-venta"),
            clavesVistas,
            "los dos intentos tienen que llevar la MISMA clave: es lo único que " +
                "impide que el server cobre dos veces",
        )
    }

    @Test
    fun `matar la app no cambia la clave con la que sale`() = runTest {
        val disco = AlmacenDeMentira()
        val enlace = MutableStateFlow(true)
        val conexion = ConexionConElNegocio(enlace)

        val antes = ColaDeVentas(disco, reloj)
        antes.cargar()
        antes.encolar(
            VentaEnCola(
                clave = "clave-que-sobrevive",
                solicitud = solicitud,
                cobradaEn = reloj(),
                lineas = 1,
            ),
        )

        // El teléfono se apaga con la venta adentro. Al abrir de nuevo, la cola
        // se lee de disco y el despachador arranca con la MISMA clave.
        val despues = ColaDeVentas(disco.trasReiniciarLaApp(), reloj)
        despues.cargar()

        val motor = motor(listOf({ ventaCreada }))
        despachadorCon(despues, conexion, enlace, motor).intentarAhora()

        assertEquals(listOf("clave-que-sobrevive"), clavesVistas)
        assertEquals(0, despues.ventas.value.size)
    }

    @Test
    fun `una repeticion que el server ya conocia tambien limpia la cola`() = runTest {
        val enlace = MutableStateFlow(true)
        val conexion = ConexionConElNegocio(enlace)

        // 200 en vez de 201: el server reconoció la clave y devolvió la orden
        // que ya había creado. Para la app es un éxito, no un caso especial.
        val motor = MockEngine { pedido ->
            clavesVistas += pedido.headers["Idempotency-Key"].orEmpty()
            respond(
                content = ventaCreada,
                status = HttpStatusCode.OK,
                headers = headersOf("Content-Type", "application/json"),
            )
        }

        val cola = ColaDeVentas(AlmacenDeMentira(), reloj)
        cola.cargar()
        cola.encolar(
            VentaEnCola("repetida", solicitud, cobradaEn = reloj(), lineas = 1),
        )

        despachadorCon(cola, conexion, enlace, motor).intentarAhora()

        assertEquals(0, cola.ventas.value.size)
        assertEquals(listOf("repetida"), clavesVistas)
    }

    @Test
    fun `el server que dice que no deja la venta a la vista, sin reintentarla`() = runTest {
        val enlace = MutableStateFlow(true)
        val conexion = ConexionConElNegocio(enlace)

        // 422: se vendió sin señal algo que después se quedó sin stock. No se
        // arregla reintentando, y borrarla sola sería perder plata en silencio.
        val motor = MockEngine { pedido ->
            clavesVistas += pedido.headers["Idempotency-Key"].orEmpty()
            respondError(HttpStatusCode.UnprocessableEntity)
        }

        val cola = ColaDeVentas(AlmacenDeMentira(), reloj)
        cola.cargar()
        cola.encolar(
            VentaEnCola("sin-stock", solicitud, cobradaEn = reloj(), lineas = 1),
        )

        val despachador = despachadorCon(cola, conexion, enlace, motor)
        despachador.intentarAhora()

        assertEquals(1, cola.ventas.value.size, "sigue a la vista")
        assertTrue(cola.ventas.value.first().rechazada)
        assertEquals(0, cola.cuantasEsperan, "pero deja de intentarse")

        despachador.intentarAhora()
        assertEquals(1, clavesVistas.size, "y no se vuelve a mandar")
    }

    @Test
    fun `un problema del server se reintenta, no se descarta`() = runTest {
        val enlace = MutableStateFlow(true)
        val conexion = ConexionConElNegocio(enlace)

        val motor = MockEngine { pedido ->
            clavesVistas += pedido.headers["Idempotency-Key"].orEmpty()
            respondError(HttpStatusCode.InternalServerError)
        }

        val cola = ColaDeVentas(AlmacenDeMentira(), reloj)
        cola.cargar()
        cola.encolar(VentaEnCola("cinco-cero-cero", solicitud, cobradaEn = reloj(), lineas = 1))

        despachadorCon(cola, conexion, enlace, motor).intentarAhora()

        assertTrue(cola.ventas.value.first().esperando, "un 500 se pasa solo: se reintenta")
        assertEquals(1, cola.ventas.value.first().intentos)
    }
}

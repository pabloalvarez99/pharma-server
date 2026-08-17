package cl.rutbusiness.app.ui.ventas

import cl.rutbusiness.core.session.AlmacenamientoPlataforma
import cl.rutbusiness.core.session.EstadoSesion
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.core.session.TokenStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import java.net.ServerSocket
import java.net.Socket
import java.util.concurrent.CopyOnWriteArrayList

/**
 * `VentasViewModel` contra un server de mentira, mismo criterio que
 * `CobrarSesionYClientesTest`: `SessionRepository` arma su propio
 * `ApiFactory` con el motor de la plataforma, así que no hay `MockEngine`
 * posible acá — se levanta un `ServerSocket` de mentira en su lugar.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
@OptIn(ExperimentalCoroutinesApi::class)
class VentasViewModelTest {

    private class ServerDeMentira(private val responder: (String) -> String) {
        private val socket = ServerSocket(0)
        val pedidos = CopyOnWriteArrayList<String>()
        val puerto: Int get() = socket.localPort
        val baseUrl: String get() = "http://127.0.0.1:$puerto"

        private val hilo = Thread {
            while (!socket.isClosed) {
                val cliente = try {
                    socket.accept()
                } catch (_: Exception) {
                    return@Thread
                }
                atender(cliente)
            }
        }.apply { isDaemon = true; start() }

        private fun atender(cliente: Socket) = cliente.use { s ->
            val entrada = s.getInputStream().bufferedReader()
            val primera = entrada.readLine() ?: return
            val ruta = primera.split(" ").getOrElse(1) { "" }
            pedidos += ruta
            var largo = 0
            while (true) {
                val linea = entrada.readLine() ?: break
                if (linea.isEmpty()) break
                if (linea.startsWith("Content-Length:", ignoreCase = true)) {
                    largo = linea.substringAfter(':').trim().toIntOrNull() ?: 0
                }
            }
            repeat(largo) { entrada.read() }
            s.getOutputStream().write(responder(ruta).toByteArray())
            s.getOutputStream().flush()
        }

        fun cerrar() {
            socket.close()
            hilo.interrupt()
        }
    }

    private fun respuesta(codigo: Int, razon: String, cuerpo: String): String =
        "HTTP/1.1 $codigo $razon\r\n" +
            "Content-Type: application/json\r\n" +
            "Content-Length: ${cuerpo.toByteArray().size}\r\n" +
            "Connection: close\r\n\r\n" +
            cuerpo

    private var server: ServerDeMentira? = null

    @Before
    fun antes() {
        Dispatchers.setMain(UnconfinedTestDispatcher())
    }

    @After
    fun despues() {
        server?.cerrar()
        Dispatchers.resetMain()
    }

    private class TokenEnMemoria : TokenStore {
        private var valor: String? = null
        override suspend fun leer(): String? = valor
        override suspend fun guardar(token: String) { valor = token }
        override suspend fun borrar() { valor = null }
    }

    private fun sesionActivaContra(baseUrl: String): SessionRepository {
        val app = RuntimeEnvironment.getApplication()
        val almacen = AlmacenamientoPlataforma.conTokensDePrueba(app, TokenEnMemoria())
        val sesion = SessionRepository(almacen)
        runBlocking {
            almacen.preferencias.guardarBaseUrl(baseUrl)
            almacen.tokens.guardar("token-de-prueba")
            sesion.restaurar()
        }
        assertTrue(
            "la prueba necesita arrancar con sesión activa",
            sesion.estado.value is EstadoSesion.Activa,
        )
        return sesion
    }

    private fun esperar(ms: Long = 5_000, condicion: () -> Boolean): Boolean {
        val hasta = System.currentTimeMillis() + ms
        while (System.currentTimeMillis() < hasta) {
            if (condicion()) return true
            Thread.sleep(20)
        }
        return condicion()
    }

    @Test
    fun `un error de red no borra la lista que ya se mostraba`() {
        val ventaDeVerdad = """
            [{"id":"order:a","status":"completed","payment_method":"cash","total":"1000","refunded_total":"0","created_at":"2026-08-17T10:00:00Z"}]
        """.trimIndent()
        var primeraLlamada = true
        server = ServerDeMentira { ruta ->
            when {
                ruta.startsWith("/api/v1/settings/money.currency") ->
                    respuesta(404, "Not Found", """{"error":{"code":"NOT_FOUND","message":"no"}}""")

                ruta.startsWith("/api/v1/orders") && primeraLlamada -> {
                    primeraLlamada = false
                    respuesta(200, "OK", ventaDeVerdad)
                }

                ruta.startsWith("/api/v1/orders") ->
                    respuesta(503, "Service Unavailable", """{"error":{"code":"DOWN","message":"caído"}}""")

                else -> respuesta(404, "Not Found", "{}")
            }
        }
        val sesion = sesionActivaContra(server!!.baseUrl)
        val vm = VentasViewModel(sesion)

        assertTrue(esperar { vm.ventas.isNotEmpty() })
        assertEquals(1, vm.ventas.size)

        vm.cargar()
        assertTrue(esperar { vm.error != null })
        assertEquals(1, vm.ventas.size)
    }

    @Test
    fun `una venta refunded no dispara la devolucion`() {
        val ventaDevuelta = """
            [{"id":"order:a","status":"refunded","payment_method":"cash","total":"1000","refunded_total":"1000","created_at":"2026-08-17T10:00:00Z"}]
        """.trimIndent()
        val detalleDevuelto = """
            {"order":{"id":"order:a","status":"refunded","payment_method":"cash","total":"1000","refunded_total":"1000","created_at":"2026-08-17T10:00:00Z"},
             "items":[{"id":"item:1","product_name":"Tomate","quantity":1,"unit_price":"1000","subtotal":"1000"}]}
        """.trimIndent()
        var devolucionesRecibidas = 0
        server = ServerDeMentira { ruta ->
            when {
                ruta.startsWith("/api/v1/settings/money.currency") ->
                    respuesta(404, "Not Found", """{"error":{"code":"NOT_FOUND","message":"no"}}""")

                ruta.startsWith("/api/v1/orders/") -> respuesta(200, "OK", detalleDevuelto)
                ruta.startsWith("/api/v1/orders") -> respuesta(200, "OK", ventaDevuelta)
                ruta.startsWith("/api/v1/pos/returns") -> {
                    devolucionesRecibidas++
                    respuesta(200, "OK", """{"order_marked_refunded":true}""")
                }

                else -> respuesta(404, "Not Found", "{}")
            }
        }
        val sesion = sesionActivaContra(server!!.baseUrl)
        val vm = VentasViewModel(sesion)

        assertTrue(esperar { vm.ventas.isNotEmpty() })
        val venta = vm.ventas.single()
        assertTrue(venta.estaDevuelta)

        vm.abrirVenta(venta)
        assertTrue(esperar { vm.detalle != null })

        vm.deshacer(feria = true)
        // El guardia de ventaElegida.estaDevuelta corta antes de llamar al server.
        Thread.sleep(200)
        assertEquals(0, devolucionesRecibidas)
    }

    @Test
    fun `deshacer con exito recarga la lista desde el server`() {
        val ventaViva = """
            [{"id":"order:a","status":"completed","payment_method":"cash","total":"1000","refunded_total":"0","created_at":"2026-08-17T10:00:00Z"}]
        """.trimIndent()
        val ventaYaDevuelta = """
            [{"id":"order:a","status":"refunded","payment_method":"cash","total":"1000","refunded_total":"1000","created_at":"2026-08-17T10:00:00Z"}]
        """.trimIndent()
        val detalleVivo = """
            {"order":{"id":"order:a","status":"completed","payment_method":"cash","total":"1000","refunded_total":"0","created_at":"2026-08-17T10:00:00Z"},
             "items":[{"id":"item:1","product_name":"Tomate","quantity":1,"unit_price":"1000","subtotal":"1000"}]}
        """.trimIndent()
        var listasServidas = 0
        server = ServerDeMentira { ruta ->
            when {
                ruta.startsWith("/api/v1/settings/money.currency") ->
                    respuesta(404, "Not Found", """{"error":{"code":"NOT_FOUND","message":"no"}}""")

                ruta.startsWith("/api/v1/orders/") -> respuesta(200, "OK", detalleVivo)
                ruta.startsWith("/api/v1/orders") -> {
                    listasServidas++
                    // La segunda vez que se pide la lista (después de deshacer) el
                    // server ya la muestra devuelta: es la prueba de que el
                    // ViewModel no la marcó en memoria, sino que preguntó de nuevo.
                    respuesta(200, "OK", if (listasServidas <= 1) ventaViva else ventaYaDevuelta)
                }

                ruta.startsWith("/api/v1/pos/returns") ->
                    respuesta(200, "OK", """{"order_marked_refunded":true}""")

                else -> respuesta(404, "Not Found", "{}")
            }
        }
        val sesion = sesionActivaContra(server!!.baseUrl)
        val vm = VentasViewModel(sesion)

        assertTrue(esperar { vm.ventas.isNotEmpty() })
        vm.abrirVenta(vm.ventas.single())
        assertTrue(esperar { vm.detalle != null })

        vm.deshacer(feria = true)
        assertTrue(esperar { vm.ventas.any { it.estaDevuelta } })
        assertNull(vm.errorDeAccion)
    }

    @Test
    fun `un 403 al deshacer da un mensaje que dice a quien pedirselo`() {
        val ventaViva = """
            [{"id":"order:a","status":"completed","payment_method":"cash","total":"1000","refunded_total":"0","created_at":"2026-08-17T10:00:00Z"}]
        """.trimIndent()
        val detalleVivo = """
            {"order":{"id":"order:a","status":"completed","payment_method":"cash","total":"1000","refunded_total":"0","created_at":"2026-08-17T10:00:00Z"},
             "items":[{"id":"item:1","product_name":"Tomate","quantity":1,"unit_price":"1000","subtotal":"1000"}]}
        """.trimIndent()
        server = ServerDeMentira { ruta ->
            when {
                ruta.startsWith("/api/v1/settings/money.currency") ->
                    respuesta(404, "Not Found", """{"error":{"code":"NOT_FOUND","message":"no"}}""")

                ruta.startsWith("/api/v1/orders/") -> respuesta(200, "OK", detalleVivo)
                ruta.startsWith("/api/v1/orders") -> respuesta(200, "OK", ventaViva)
                ruta.startsWith("/api/v1/pos/returns") ->
                    respuesta(403, "Forbidden", """{"error":{"code":"FORBIDDEN","message":"no puedes"}}""")

                else -> respuesta(404, "Not Found", "{}")
            }
        }
        val sesion = sesionActivaContra(server!!.baseUrl)
        val vm = VentasViewModel(sesion)

        assertTrue(esperar { vm.ventas.isNotEmpty() })
        vm.abrirVenta(vm.ventas.single())
        assertTrue(esperar { vm.detalle != null })

        vm.deshacer(feria = true)
        assertTrue(esperar { vm.errorDeAccion != null })
        val error = vm.errorDeAccion
        assertNotNull(error)
        assertEquals(mensajeSinPermisoDeshacer(feria = true), error!!.message)
    }
}

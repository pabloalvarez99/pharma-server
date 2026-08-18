package cl.rutbusiness.app.ui.gastos

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
 * `GastosViewModel` contra un server de mentira, mismo criterio que
 * `VentasViewModelTest`: `SessionRepository` arma su propio `ApiFactory` con
 * el motor de la plataforma, así que no hay `MockEngine` posible acá.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
@OptIn(ExperimentalCoroutinesApi::class)
class GastosViewModelTest {

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

    private fun respuestaMonedaNoConfigurada(ruta: String): String? =
        if (ruta.startsWith("/api/v1/settings/money.currency")) {
            respuesta(404, "Not Found", """{"error":{"code":"NOT_FOUND","message":"no"}}""")
        } else {
            null
        }

    @Test
    fun `un error de red no borra la lista que ya se mostraba`() {
        val gastoDeVerdad = """
            [{"id":"expense:a","category":"arriendo","description":"Arriendo","amount":"5000","payment_method":"cash","incurred_at":"2026-08-17T09:00:00Z","created_at":"2026-08-17T09:00:00Z"}]
        """.trimIndent()
        var primeraLlamada = true
        server = ServerDeMentira { ruta ->
            respuestaMonedaNoConfigurada(ruta) ?: when {
                ruta.startsWith("/api/v1/expenses") && primeraLlamada -> {
                    primeraLlamada = false
                    respuesta(200, "OK", gastoDeVerdad)
                }

                ruta.startsWith("/api/v1/expenses") ->
                    respuesta(503, "Service Unavailable", """{"error":{"code":"DOWN","message":"caído"}}""")

                else -> respuesta(404, "Not Found", "{}")
            }
        }
        val sesion = sesionActivaContra(server!!.baseUrl)
        val vm = GastosViewModel(sesion)

        assertTrue(esperar { vm.gastos.isNotEmpty() })
        assertEquals(1, vm.gastos.size)

        vm.cargar()
        assertTrue(esperar { vm.error != null })
        assertEquals(1, vm.gastos.size)
    }

    @Test
    fun `despues de anotar con exito se recarga desde el server, no se agrega en memoria`() {
        val listaVacia = """[]"""
        val listaConElNuevo = """
            [{"id":"expense:nuevo","category":"flete","description":"Flete","amount":"3000","payment_method":"cash","incurred_at":"2026-08-17T10:00:00Z","created_at":"2026-08-17T10:00:00Z"}]
        """.trimIndent()
        var listasServidas = 0
        var anotaciones = 0
        server = ServerDeMentira { ruta ->
            respuestaMonedaNoConfigurada(ruta) ?: when {
                ruta.startsWith("/api/v1/expenses") && ruta.contains("from") -> {
                    listasServidas++
                    respuesta(200, "OK", if (listasServidas <= 1) listaVacia else listaConElNuevo)
                }

                ruta == "/api/v1/expenses" -> {
                    anotaciones++
                    respuesta(
                        201,
                        "Created",
                        """{"id":"expense:nuevo","category":"flete","description":"Flete","amount":"3000","payment_method":"cash","incurred_at":"2026-08-17T10:00:00Z","created_at":"2026-08-17T10:00:00Z"}""",
                    )
                }

                else -> respuesta(404, "Not Found", "{}")
            }
        }
        val sesion = sesionActivaContra(server!!.baseUrl)
        val vm = GastosViewModel(sesion)

        assertTrue(esperar { !vm.cargando })
        assertTrue(vm.gastos.isEmpty())

        vm.abrirFormulario()
        vm.cambiarCategoriaNueva("flete")
        vm.cambiarDescripcionNueva("Flete")
        vm.cambiarMontoNuevo("3000")
        vm.anotar(feria = true)

        assertTrue(esperar { vm.gastos.isNotEmpty() })
        assertEquals(1, anotaciones)
        assertEquals("expense:nuevo", vm.gastos.single().id)
    }

    @Test
    fun `el rango de hoy filtra por incurred_at, nunca por created_at`() {
        server = ServerDeMentira { ruta ->
            respuestaMonedaNoConfigurada(ruta) ?: respuesta(200, "OK", "[]")
        }
        val sesion = sesionActivaContra(server!!.baseUrl)
        val vm = GastosViewModel(sesion)

        assertTrue(esperar { !vm.cargando })

        val pedidoDeGastos = server!!.pedidos.first { it.startsWith("/api/v1/expenses") }
        // El nombre del filtro es `from`/`to`, el mismo par que usa el server para
        // filtrar por `incurred_at` — ver GastosApi.kt.
        assertTrue(pedidoDeGastos.contains("from="))
        assertTrue(pedidoDeGastos.contains("to="))
    }

    @Test
    fun `un 403 al anotar da un mensaje que dice a quien pedirselo`() {
        server = ServerDeMentira { ruta ->
            respuestaMonedaNoConfigurada(ruta) ?: when {
                ruta.startsWith("/api/v1/expenses") && ruta.contains("from") ->
                    respuesta(200, "OK", "[]")

                ruta == "/api/v1/expenses" ->
                    respuesta(403, "Forbidden", """{"error":{"code":"FORBIDDEN","message":"no puedes"}}""")

                else -> respuesta(404, "Not Found", "{}")
            }
        }
        val sesion = sesionActivaContra(server!!.baseUrl)
        val vm = GastosViewModel(sesion)

        assertTrue(esperar { !vm.cargando })

        vm.abrirFormulario()
        vm.cambiarCategoriaNueva("flete")
        vm.cambiarDescripcionNueva("Flete")
        vm.cambiarMontoNuevo("3000")
        vm.anotar(feria = true)

        assertTrue(esperar { vm.errorDeGuardado != null })
        val error = vm.errorDeGuardado
        assertNotNull(error)
        assertEquals(CopyGastos.mensajeSinPermiso(feria = true), error!!.message)
    }
}

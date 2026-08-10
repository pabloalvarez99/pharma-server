package cl.rutbusiness.app.ui.cobrar

import cl.rutbusiness.core.api.models.ProductDto
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
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Dos promesas de la pantalla de cobro, contra un server de verdad.
 *
 * No hay `MockEngine` acá a propósito: [SessionRepository] arma sus propios
 * [cl.rutbusiness.core.net.ApiFactory] con el motor de la plataforma, así que
 * la única forma de ejercitar el camino completo -sesión activa, token,
 * respuesta del server, ViewModel- es que haya algo escuchando en un puerto.
 * Es un `ServerSocket` de veinte líneas, no un `pharma-api`.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
@OptIn(ExperimentalCoroutinesApi::class)
class CobrarSesionYClientesTest {

    /** Server de mentira: responde lo que se le diga y anota qué le pidieron. */
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
            // "POST /api/v1/pos/sale HTTP/1.1" → "/api/v1/pos/sale"
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
            // Drenar el body: si no se lee, el cliente puede ver un reset antes
            // de alcanzar a leer la respuesta.
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
        // `viewModelScope` corre en `Dispatchers.Main`. Sin esto no se ejecuta
        // nada de lo que el ViewModel lanza y la prueba mediría el silencio.
        Dispatchers.setMain(UnconfinedTestDispatcher())
    }

    @After
    fun despues() {
        server?.cerrar()
        Dispatchers.resetMain()
    }

    /**
     * El token en memoria. El de producción cifra contra `AndroidKeyStore`, que
     * fuera de un aparato no existe: en la JVM revienta con
     * `NoSuchAlgorithmException` antes de que la prueba llegue a nada.
     */
    private class TokenEnMemoria : TokenStore {
        private var valor: String? = null
        override suspend fun leer(): String? = valor
        override suspend fun guardar(token: String) { valor = token }
        override suspend fun borrar() { valor = null }
    }

    /** Sesión activa apuntando al server de mentira, como tras un login real. */
    private fun sesionActivaContra(baseUrl: String): SessionRepository {
        val app = RuntimeEnvironment.getApplication()
        val almacen = AlmacenamientoPlataforma.conTokensDePrueba(app, TokenEnMemoria())
        val sesion = SessionRepository(almacen)
        runBlocking {
            almacen.preferencias.guardarBaseUrl(baseUrl)
            almacen.tokens.guardar("token-que-el-server-va-a-rechazar")
            sesion.restaurar()
        }
        assertTrue(
            "la prueba necesita arrancar con sesión activa",
            sesion.estado.value is EstadoSesion.Activa,
        )
        return sesion
    }

    private fun producto(id: String, nombre: String) = ProductDto(
        active = true,
        createdAt = "2026-08-09T00:00:00Z",
        id = id,
        name = nombre,
        physicalStock = true,
        prescriptionType = "none",
        price = "5000",
        slug = id,
        stock = 10L,
        updatedAt = "2026-08-09T00:00:00Z",
    )

    /** Espera hasta [ms] a que se cumpla [condicion]; devuelve si se cumplió. */
    private fun esperar(ms: Long = 5_000, condicion: () -> Boolean): Boolean {
        val hasta = System.currentTimeMillis() + ms
        while (System.currentTimeMillis() < hasta) {
            if (condicion()) return true
            Thread.sleep(20)
        }
        return condicion()
    }

    // --- la sesión vencida en medio de un cobro -------------------------------

    /**
     * El copy de `RbErrorKind.Unauthorized` promete "te vamos a llevar a la
     * pantalla de entrada", y no trae botón. Quien cumple esa promesa es
     * `sesion.salir()`: `RutBusinessApp` observa `sesion.estado` y cambia solo.
     *
     * Faltaba justo en el cobro. El cajero se quedaba en la pantalla de pago,
     * con el cliente delante, leyendo un rescate que no llegaba nunca y sin
     * nada que tocar. Todas las demás acciones -abrir caja, anotar movimiento,
     * cerrar caja, abonar un fiado- ya cerraban la sesión.
     */
    @Test
    fun `un 401 al cobrar cierra la sesion, que es lo que el mensaje promete`() {
        // El 401 sale SÓLO en el cobro. Si el server rechazara todo, la
        // búsqueda del `init` ya cerraría la sesión por su propio camino
        // (`buscar()` sí la cierra) y esta prueba pasaría en verde con el bug
        // intacto — se comprobó: así pasaba.
        val s = ServerDeMentira { ruta ->
            if (ruta.startsWith("/api/v1/pos/sale")) {
                respuesta(401, "Unauthorized", """{"error":"token expirado"}""")
            } else {
                respuesta(200, "OK", "[]")
            }
        }
        server = s
        val sesion = sesionActivaContra(s.baseUrl)

        val vm = CobrarViewModel(sesion)
        vm.agregar(producto("product:pan", "Pan amasado"))
        vm.irAPagar()
        vm.cambiarMontoEntregado("10000")
        assertEquals("la prueba necesita poder cobrar", null, vm.impedimentoParaCobrar())

        vm.cobrar()

        assertTrue(
            "el server contestó 401 y nadie cerró la sesión: el mensaje dice que " +
                "te lleva a la pantalla de entrada y la app se queda donde está",
            esperar { sesion.estado.value is EstadoSesion.SinSesion },
        )
    }

    // --- los clientes del fiado ----------------------------------------------

    /**
     * En la feria el fiado es cómo funciona el barrio, no un extra. La lista de
     * a quién se le puede fiar tiene que estar cuando la cajera elige "Fiado",
     * y no empezando a viajar en ese momento: se pide al abrir la pantalla,
     * mientras se arma el carrito.
     *
     * Antes sólo salía desde `irAPagar()`. Con un catálogo grande el server
     * tarda segundos, así que la lista llegaba -si llegaba- después de que la
     * cajera ya estaba mirando el selector vacío.
     */
    @Test
    fun `los clientes se piden al abrir la pantalla, no al llegar al pago`() {
        val listo = CountDownLatch(1)
        val s = ServerDeMentira { ruta ->
            if (ruta.startsWith("/api/v1/clientes")) listo.countDown()
            respuesta(200, "OK", "[]")
        }
        server = s
        val sesion = sesionActivaContra(s.baseUrl)

        // Sólo construirlo. No se toca `irAPagar()` en ningún momento.
        CobrarViewModel(sesion)

        assertTrue(
            "la lista de clientes no se pidió al abrir Cobrar: la cajera va a " +
                "llegar al fiado con el selector vacío y el vecino esperando",
            listo.await(5, TimeUnit.SECONDS),
        )
    }

    /**
     * Ir y volver entre buscar y pagar no puede disparar una llamada por viaje.
     * El guard viejo miraba sólo `clientes.isNotEmpty()`, que sigue siendo
     * falso mientras la primera respuesta viene en camino — y en un puesto sin
     * señal buena, cada pedido de más es tiempo que la pantalla no tiene.
     */
    @Test
    fun `entrar y salir del pago no pide la lista de clientes de nuevo`() {
        val s = ServerDeMentira { respuesta(200, "OK", "[]") }
        server = s
        val sesion = sesionActivaContra(s.baseUrl)

        val vm = CobrarViewModel(sesion)
        esperar { s.pedidos.any { it.startsWith("/api/v1/clientes") } }
        vm.agregar(producto("product:pan", "Pan amasado"))

        repeat(3) {
            vm.irAPagar()
            vm.volverABuscar()
        }
        // Margen para que un pedido de más alcance a llegar y ser contado.
        esperar(300) { false }

        assertEquals(
            "la lista de clientes se pidió más de una vez",
            1,
            s.pedidos.count { it.startsWith("/api/v1/clientes") },
        )
    }
}

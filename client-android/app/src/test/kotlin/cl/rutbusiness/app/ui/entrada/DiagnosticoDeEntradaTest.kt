package cl.rutbusiness.app.ui.entrada

import cl.rutbusiness.core.error.AppError
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Las cuatro fallas de conexión, una por una.
 *
 * El encargo del founder es literal: cuatro problemas distintos con cuatro
 * soluciones distintas, porque "error de conexión" a secas no le sirve a nadie.
 * Esta prueba es lo que impide que en un refactor dos de esos cuatro terminen
 * compartiendo mensaje otra vez.
 *
 * Se prueba la función y no el `ViewModel` a propósito: montar el `ViewModel`
 * necesitaría un `SessionRepository`, que abre el almacén cifrado del
 * AndroidKeyStore. Toda la decisión de qué se le dice a la dueña vive acá.
 */
class DiagnosticoDeEntradaTest {

    private val direccion = "http://192.168.1.10:8080"

    private fun diagnosticar(
        hayRed: Boolean? = true,
        sondeo: Sondeo = Sondeo.EsElSistema,
        registro: MutableList<String> = mutableListOf(),
    ): FallaDeConexion? = runBlocking {
        diagnosticarLaEntrada(
            direccion = direccion,
            hayRed = hayRed?.let { { it } },
            sondear = { registro += it; sondeo },
        )
    }

    // --- las cuatro categorías --------------------------------------------

    @Test
    fun `sin red en el telefono lo dice sin salir a la red`() {
        val sondeos = mutableListOf<String>()
        val falla = diagnosticar(hayRed = false, registro = sondeos)

        assertTrue("tenía que ser SinRed, fue $falla", falla is FallaDeConexion.SinRed)
        // Lo importante no es sólo el mensaje: con el teléfono desconectado no
        // se sale a esperar diez segundos de timeout para decir lo mismo.
        assertEquals("no había red y sin embargo se sondeó", emptyList<String>(), sondeos)
    }

    @Test
    fun `nadie contesta en esa direccion`() {
        val falla = diagnosticar(sondeo = Sondeo.NadieContesta("ConnectException"))
        assertTrue(falla is FallaDeConexion.NadieContesta)
        assertTrue(
            "el mensaje tiene que decir qué dirección se intentó, y como ella la escribió",
            falla!!.queHacer.contains("192.168.1.10:8080"),
        )
    }

    @Test
    fun `contesta algo que no es el sistema del negocio`() {
        val falla = diagnosticar(sondeo = Sondeo.ContestaOtraCosa("200 <html>"))
        assertTrue(falla is FallaDeConexion.ContestaPeroNoEsElSistema)
        assertTrue(
            "es el caso del puerto equivocado y el mensaje tiene que nombrarlo",
            falla!!.queHacer.contains("después de los dos puntos"),
        )
    }

    @Test
    fun `los datos que no coinciden se dicen recien despues de llegar`() {
        val falla = fallaDeLogin(AppError.CredencialesInvalidas(), direccion)
        assertTrue(falla is FallaDeConexion.DatosQueNoCoinciden)
        // La dueña ya sabe que la dirección está bien: mandarla a revisar el
        // wifi acá sería el error que este encargo vino a arreglar.
        assertFalse(falla.queHacer.contains("wifi"))
    }

    @Test
    fun `cuando el sistema contesta bien no hay falla`() {
        assertNull(diagnosticar(sondeo = Sondeo.EsElSistema))
    }

    @Test
    fun `sin servicio de red se sondea igual en vez de inventar una respuesta`() {
        val sondeos = mutableListOf<String>()
        val falla = diagnosticar(hayRed = null, registro = sondeos)

        assertNull(falla)
        assertEquals(listOf(direccion), sondeos)
    }

    // --- qué pasa cuando el login falla después de un sondeo bueno ---------

    @Test
    fun `si la conexion se corta despues del sondeo se culpa a la conexion`() {
        val falla = fallaDeLogin(AppError.ServidorNoResponde(direccion), direccion)
        assertTrue(falla is FallaDeConexion.NadieContesta)
    }

    @Test
    fun `correo en dos puestos pide el nombre corto`() {
        val falla = fallaDeLogin(
            AppError.ErrorDelServidor(409, "NECESITA_NEGOCIO", "escribí el nombre corto"),
            direccion,
            nube = true,
        )
        assertTrue(falla is FallaDeConexion.FaltaNombreCorto)
        assertFalse(falla.queHacer.contains("computador", ignoreCase = true))
        assertTrue(falla.queHacer.contains("nombre corto"))
    }

    @Test
    fun `un error propio del server no se lee como clave mala`() {
        val falla = fallaDeLogin(
            AppError.ErrorDelServidor(status = 503, code = null, serverMessage = null),
            direccion,
        )
        assertTrue(falla is FallaDeConexion.ContestaPeroNoEsElSistema)
    }

    // --- las reglas de escritura que valen para las cuatro -----------------

    private val todas = listOf(
        FallaDeConexion.SinRed(),
        FallaDeConexion.NadieContesta(direccion),
        FallaDeConexion.ContestaPeroNoEsElSistema(direccion),
        FallaDeConexion.DatosQueNoCoinciden(),
    )

    @Test
    fun `las cuatro dicen cosas distintas`() {
        assertEquals(
            "dos fallas comparten título: vuelven a ser un solo «error de conexión»",
            4,
            todas.map { it.titulo }.toSet().size,
        )
        assertEquals(
            "dos fallas comparten instrucción: mandan a arreglar el mismo lugar",
            4,
            todas.map { it.queHacer }.toSet().size,
        )
    }

    /**
     * Cero jerga.
     *
     * La lista es la del encargo más lo que se coló en la versión anterior de
     * esta pantalla. "Servidor" está adentro a propósito: la palabra se enseña
     * una vez en el primer uso y no se repite en un mensaje de error.
     */
    @Test
    fun `ninguna falla usa jerga`() {
        val prohibidas = listOf(
            "endpoint", "API", "instancia", "tenant", "servidor", "host", "puerto",
            "URL", "HTTP", "timeout", "socket", "backend",
        )
        val infractoras = todas.flatMap { falla ->
            val texto = "${falla.titulo} ${falla.queHacer}"
            prohibidas.filter { texto.contains(it, ignoreCase = true) }
                .map { "«${falla.titulo}» dice «$it»" }
        }
        assertTrue(infractoras.joinToString("\n"), infractoras.isEmpty())
    }

    /** Un título sin instrucción es un diagnóstico, y de eso venimos. */
    @Test
    fun `las cuatro dicen que hacer`() {
        todas.forEach { falla ->
            assertTrue(
                "«${falla.titulo}» no dice qué hacer",
                falla.queHacer.length > 40,
            )
        }
    }

    /** El detalle técnico nunca se filtra al texto que se lee. */
    @Test
    fun `en la nube nadie contesta no nombra computador ni IP`() {
        val falla = runBlocking {
            diagnosticarLaEntrada(
                direccion = "https://nube.test.invalid",
                hayRed = { true },
                sondear = { Sondeo.NadieContesta("timeout") },
                nube = true,
            )
        }
        assertTrue(falla is FallaDeConexion.NadieContesta)
        val texto = "${falla!!.titulo} ${falla.queHacer}"
        assertFalse(texto.contains("192.168"))
        assertFalse(texto.contains("computador", ignoreCase = true))
        assertFalse(texto.contains("nube.test.invalid"))
        assertTrue(falla.titulo.contains("RutAgent"))
    }

    @Test
    fun `el detalle tecnico no aparece en lo que se muestra`() {
        val tecnico = "java.net.ConnectException: failed to connect"
        val falla = FallaDeConexion.NadieContesta(direccion, tecnico)

        assertFalse(falla.titulo.contains("java.net"))
        assertFalse(falla.queHacer.contains("java.net"))
        assertEquals(tecnico, falla.tecnico)
    }
}

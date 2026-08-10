package cl.rutbusiness.core.net

import cl.rutbusiness.core.catalog.ProductRepository
import cl.rutbusiness.core.customers.CustomerRepository
import cl.rutbusiness.core.session.AuthApi
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.http.HttpStatusCode
import io.ktor.http.headersOf
import io.ktor.utils.io.errors.IOException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlinx.coroutines.test.runTest

/**
 * El 401 avisa una sola vez, desde un solo lugar, para todas las llamadas.
 *
 * **Por qué existe esta prueba.** Antes esto era una línea a mano en la rama de
 * error de cada pantalla: catorce `sesion.salir()` repartidos en seis
 * ViewModels. Ya faltó una vez —en el cobro, 2026-08-09—, y el síntoma no fue
 * un log feo: el cajero se quedaba mirando "te vamos a llevar a la pantalla de
 * entrada" con el cliente delante, sin botón, para siempre, porque el copy de
 * `RbErrorKind.Unauthorized` no trae `retryLabel`. El mensaje prometía un
 * rescate que nadie estaba cumpliendo.
 *
 * Un comentario pidiendo que cada quien se acuerde no es un mecanismo. El
 * mecanismo es [llamar], por donde pasa **todo** lo que habla con el server, y
 * lo que se prueba acá es justamente eso: que el aviso sale del embudo y no del
 * repositorio, así que un repositorio nuevo lo hereda sin escribir una línea.
 */
class AvisoDeSesionTest {

    /** Un oyente que sólo cuenta. Quién cierra la sesión es cosa de la sesión. */
    private class Oyente : AvisoDeSesion {
        var avisos = 0
            private set

        override suspend fun vencio() {
            avisos++
        }
    }

    private fun apiQueContesta(
        oyente: AvisoDeSesion,
        respuesta: suspend () -> Any,
    ): ApiFactory = ApiFactory(
        "http://localhost:8080",
        ReporteDeRed.Nulo,
        MockEngine {
            when (val r = respuesta()) {
                is Throwable -> throw r
                is Pair<*, *> -> respond(
                    content = r.second as String,
                    status = r.first as HttpStatusCode,
                    headers = headersOf("Content-Type", "application/json"),
                )
                else -> respond(
                    content = r as String,
                    status = HttpStatusCode.OK,
                    headers = headersOf("Content-Type", "application/json"),
                )
            }
        },
        sesionVencida = oyente,
    ) { "token-vencido" }

    private fun no(status: HttpStatusCode, cuerpo: String = """{"error":{"code":"UNAUTHORIZED"}}""") =
        status to cuerpo

    // --- el aviso sale, y no importa quién llamó ------------------------------

    /**
     * El caso que faltaba. Es una lectura de catálogo cualquiera; lo que se
     * afirma no es que este repositorio avise, sino que avisa sin que este
     * repositorio sepa que existe el problema.
     */
    @Test
    fun `un 401 en una lectura avisa que la sesion vencio`() = runTest {
        val oyente = Oyente()
        val r = ProductRepository(apiQueContesta(oyente) { no(HttpStatusCode.Unauthorized) })
            .buscar("pan")

        assertTrue(r is Resultado.Falla, "el 401 tiene que llegar como falla igual")
        assertEquals(1, oyente.avisos, "el 401 no avisó: la sesión se queda abierta con el token muerto")
    }

    /**
     * Otro repositorio, de otro paquete, sin nada en común con el anterior más
     * que [llamar]. Si el aviso viviera en el repositorio, éste no avisaría —
     * que es exactamente la forma que tenía el bug.
     */
    @Test
    fun `y avisa igual desde cualquier otro repositorio`() = runTest {
        val oyente = Oyente()
        CustomerRepository(apiQueContesta(oyente) { no(HttpStatusCode.Unauthorized) }).listar()

        assertEquals(1, oyente.avisos, "el aviso depende del repositorio, no del embudo")
    }

    /** Una sola llamada, un solo aviso: nadie recibe el 401 por duplicado. */
    @Test
    fun `avisa una vez por llamada`() = runTest {
        val oyente = Oyente()
        val api = apiQueContesta(oyente) { no(HttpStatusCode.Unauthorized) }

        ProductRepository(api).buscar("pan")
        ProductRepository(api).buscar("leche")

        assertEquals(2, oyente.avisos)
    }

    // --- y no sale cuando no corresponde -------------------------------------

    /**
     * **El 401 del login no es una sesión vencida.** Es una contraseña que no
     * era. El server lo distingue con `BAD_CREDENTIALS` y [errorDesde] lo mapea
     * a `CredencialesInvalidas`, así que nunca llega a este camino.
     *
     * Importa: si avisara, cada intento fallido de entrar dispararía el cierre
     * de sesión sobre la pantalla de entrada —borrando el token de nadie y
     * pisando lo que la persona estaba tipeando— justo mientras trata de
     * corregir la clave.
     */
    @Test
    fun `una clave equivocada en el login no es una sesion vencida`() = runTest {
        val oyente = Oyente()
        val api = apiQueContesta(oyente) {
            HttpStatusCode.Unauthorized to """{"error":{"code":"BAD_CREDENTIALS","message":"clave incorrecta"}}"""
        }

        val r = AuthApi(api).login("almacen", "ana@ejemplo.cl", "la-que-no-era")

        assertTrue(r is Resultado.Falla)
        assertEquals(0, oyente.avisos, "un login rechazado disparó el cierre de sesión")
    }

    /** El resto de los errores del server no tocan la sesión. */
    @Test
    fun `otros errores no cierran la sesion`() = runTest {
        listOf(
            HttpStatusCode.Forbidden,
            HttpStatusCode.NotFound,
            HttpStatusCode.InternalServerError,
            HttpStatusCode.ServiceUnavailable,
        ).forEach { status ->
            val oyente = Oyente()
            ProductRepository(apiQueContesta(oyente) { no(status) }).buscar("pan")
            assertEquals(0, oyente.avisos, "un $status cerró la sesión")
        }
    }

    /**
     * Y sobre todo: **quedarse sin señal no es quedarse sin sesión.** Es el caso
     * normal del puesto en la feria, y mandar a tipear la clave de nuevo cada
     * vez que se cae la red es exactamente lo que esta app no hace.
     */
    @Test
    fun `sin red no se cierra la sesion`() = runTest {
        val oyente = Oyente()
        ProductRepository(apiQueContesta(oyente) { IOException("se cortó la red") }).buscar("pan")

        assertEquals(0, oyente.avisos, "una caída de red mandó a la pantalla de entrada")
    }

    /** Sin oyente no explota nada: es el default de los tests y del tooling. */
    @Test
    fun `sin oyente el 401 sigue siendo una falla y nada mas`() = runTest {
        val api = ApiFactory(
            "http://localhost:8080",
            ReporteDeRed.Nulo,
            MockEngine { respond("", HttpStatusCode.Unauthorized) },
        ) { null }

        assertTrue(ProductRepository(api).buscar("pan") is Resultado.Falla)
    }
}

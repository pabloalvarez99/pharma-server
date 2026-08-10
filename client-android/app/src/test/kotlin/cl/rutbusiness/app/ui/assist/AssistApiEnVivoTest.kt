package cl.rutbusiness.app.ui.assist

import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.session.AuthApi
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * El contrato de dos pasos, contra un `pharma-api` de verdad.
 *
 * Los otros tests prueban la pantalla; éste prueba que los DTOs de
 * [AssistApi] calcen con lo que el server manda. Es lo único que atrapa un
 * campo mal nombrado: un `confirm_token` que se deserializa a `""` compila,
 * pasa todos los tests de UI, y falla recién en el teléfono de la dueña.
 *
 * **Se salta solo** si no hay server escuchando, así que en CI no molesta.
 * Para correrlo:
 *
 * ```
 * cargo build --bin pharma-api --bin pharma
 * PHARMA__BIND=0.0.0.0:8099 PHARMA__DB__PATH=$PWD/data/surreal \
 *   ./target/debug/pharma-api.exe &
 * ./target/debug/pharma.exe tenant-create "Almacen Dona Rosa" --slug donarosa
 * ./target/debug/pharma.exe user-create --tenant donarosa \
 *   --email rosa@donarosa.cl --roles owner --password '<clave>'
 * ./gradlew :app:testDebugUnitTest --tests '*AssistApiEnVivoTest*' \
 *   -Drb.assist.password='<la misma clave>'
 * ```
 *
 * La clave llega por propiedad de sistema y no está escrita acá: una
 * credencial no se commitea ni siendo de un server local de pruebas.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class AssistApiEnVivoTest {

    private val baseUrl = System.getProperty("rb.assist.baseUrl") ?: "http://127.0.0.1:8099"
    private val tenant = System.getProperty("rb.assist.tenant") ?: "donarosa"
    private val email = System.getProperty("rb.assist.email") ?: "rosa@donarosa.cl"
    private val password: String? = System.getProperty("rb.assist.password")

    /** El mensaje del error más su detalle técnico, para diagnosticar. */
    private fun detalle(r: Resultado<*>): String = when (r) {
        is Resultado.Ok -> "ok"
        is Resultado.Falla -> "${r.error.userMessage} | ${r.error.technical}"
    }

    /** Login real; devuelve una fábrica que ya lleva el bearer. */
    private fun conSesion(): ApiFactory? {
        val clave = password ?: return null
        var token: String? = null
        val api = ApiFactory(baseUrl) { token }
        val login = runBlocking {
            runCatching { AuthApi(api).login(tenant, email, clave) }.getOrNull()
        }
        return when (login) {
            is Resultado.Ok -> {
                token = login.valor.token
                api
            }

            else -> null
        }
    }

    @Test
    fun `preguntar, proponer, confirmar, y el segundo confirm es rechazado`() {
        val api = conSesion()
        assumeTrue("no hay pharma-api en $baseUrl con clave; test omitido", api != null)
        val assist = AssistApi(api!!)

        runBlocking {
            // 1. Pregunta de lectura: contesta con datos del negocio y sin
            //    proponer nada que escribir.
            val lectura = assist.preguntar("cuanto vendi hoy")
            assertTrue("la lectura falló: ${detalle(lectura)}", lectura is Resultado.Ok)
            val respuesta = (lectura as Resultado.Ok).valor
            assertTrue("una pregunta de lectura no puede traer acción", respuesta.action == null)
            assertTrue("el agente tiene que contestar algo", respuesta.text.isNotBlank())

            // 2. Orden de escribir: vuelve la propuesta, con token y sin haber
            //    escrito todavía.
            val orden = assist.preguntar("Registra un gasto de 5000 en arriendo")
            assertTrue("la propuesta falló: ${detalle(orden)}", orden is Resultado.Ok)
            val propuesta = (orden as Resultado.Ok).valor.action
            assertNotNull("una orden de escribir tiene que traer propuesta", propuesta)
            assertEquals("registrar_gasto", propuesta!!.name)
            assertTrue("sin token no se puede confirmar", propuesta.confirmToken.isNotBlank())
            assertTrue("el resumen es lo que lee la dueña", propuesta.summary.isNotBlank())

            // Los params son lo que la tarjeta muestra: si no llegan, la dueña
            // confirmaría a ciegas.
            val lineas = DetalleAccion.lineas(propuesta.params)
            assertTrue("la tarjeta se quedaría sin detalle", lineas.isNotEmpty())
            assertEquals("$5.000", lineas.first { it.etiqueta == "Monto" }.valor)

            // La fecha de vencimiento tiene que ser legible por el parser
            // propio; si no, la tarjeta no sabe cuánto queda.
            assertNotNull(
                "no se pudo leer expires_at: ${propuesta.expiresAt}",
                Vencimiento.epochSegundosDesdeRfc3339(propuesta.expiresAt),
            )

            // 3. Confirmar: recién acá el server escribe.
            val ejecucion = assist.confirmar(propuesta.confirmToken)
            assertTrue("la ejecución falló: ${detalle(ejecucion)}", ejecucion is Resultado.Ok)
            assertEquals("registrar_gasto", (ejecucion as Resultado.Ok).valor.action)

            // 4. El mismo token otra vez: rechazado. Ésta es la defensa contra
            //    la doble ejecución, y se prueba contra el server real porque
            //    es el server quien la garantiza.
            val repetido = assist.confirmar(propuesta.confirmToken)
            assertTrue("¡el token se pudo usar dos veces!", repetido is Resultado.Falla)

            // Y lo que ve la dueña de ese rechazo no la manda a repetirlo a
            // ciegas.
            val texto = mensajeDeRechazo((repetido as Resultado.Falla).error.userMessage)
            assertTrue("«$texto» no puede hablar de tokens", !texto.contains("token", true))
            assertTrue("«$texto» tiene que mandar a revisar", texto.contains("revísalo", true))
        }

        api.close()
    }
}

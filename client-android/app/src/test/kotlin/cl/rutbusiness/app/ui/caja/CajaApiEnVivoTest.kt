package cl.rutbusiness.app.ui.caja

import cl.rutbusiness.core.money.Dinero
import cl.rutbusiness.core.money.Moneda
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
 * El ritual completo de la caja contra un `pharma-api` de verdad.
 *
 * Los otros tests prueban las pantallas; éste prueba que la caja **funcione
 * punta a punta**: abrir, mover plata, mirar el arqueo y cerrar contando, con la
 * diferencia que devuelve el server. Es lo único que atrapa un campo mal
 * nombrado — `closing_cash_expected` escrito `closingCashExpected` se
 * deserializa a `null`, compila, pasa todos los tests de UI, y la pantalla de
 * arqueo le dice a la dueña "no pudimos traer la comparación" para siempre.
 *
 * **Este test escribe.** Abre y cierra una caja de verdad y le anota un retiro.
 * Va contra el mismo server de pruebas que `ResumenApiEnVivoTest` y **se salta
 * solo** si no hay ninguno escuchando, así que en CI no molesta. No apuntarlo al
 * server de un negocio.
 *
 * ```
 * cargo build --bin pharma-api --bin pharma
 * PHARMA_ALLOW_INSECURE_JWT=1 PHARMA__BIND=127.0.0.1:8099 \
 *   PHARMA__DB__PATH=$PWD/data/surreal ./target/debug/pharma-api.exe &
 * ./gradlew :app:testDebugUnitTest --tests '*CajaApiEnVivoTest*' \
 *   -Drb.assist.email='<usuario>' -Drb.assist.password='<clave>'
 * ```
 *
 * La clave llega por propiedad de sistema y no está escrita acá: una credencial
 * no se commitea ni siendo de un server local de pruebas.
 *
 * El `pharma-api` y el CLI `pharma` **no pueden correr a la vez**: la base
 * embebida es de un solo escritor, y el server no ve lo que el CLI escribió
 * mientras estaba levantado. Crear el usuario primero, levantar el server
 * después.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class CajaApiEnVivoTest {

    private val baseUrl = System.getProperty("rb.assist.baseUrl") ?: "http://127.0.0.1:8099"
    private val tenant = System.getProperty("rb.assist.tenant") ?: "donarosa"
    private val email = System.getProperty("rb.assist.email") ?: "rosa@donarosa.cl"
    private val password: String? = System.getProperty("rb.assist.password")

    private fun detalle(r: Resultado<*>): String = when (r) {
        is Resultado.Ok -> "ok"
        is Resultado.Falla -> "${r.error.userMessage} | ${r.error.technical}"
    }

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

    /** Dos montos del server son el mismo aunque vengan con distinta escala. */
    private fun mismoMonto(esperado: String, recibido: String?, que: String) {
        assertNotNull("$que no llegó", recibido)
        val a = Dinero.deTextoDeServidor(esperado)
        val b = Dinero.deTextoDeServidor(recibido!!)
        assertNotNull("$que = «$recibido» no es un decimal que Dinero sepa leer", b)
        assertEquals("$que tenía que ser $esperado y llegó $recibido", 0, a!!.compareTo(b!!))
    }

    @Test
    fun `abrir, mover plata, arquear y cerrar sale entero del server`() {
        val api = conSesion()
        assumeTrue("no hay pharma-api en $baseUrl con clave; test omitido", api != null)
        val caja = CajaApi(api!!)

        runBlocking {
            // 0. Un cajero tiene una sola caja abierta a la vez. Si quedó una de
            //    una corrida anterior, se cierra: no es limpieza cosmética, es
            //    que sin esto el `abrir` de abajo contesta 409 y el test miente
            //    sobre por qué falló.
            (caja.sesionAbierta() as? Resultado.Ok)?.valor?.let { vieja ->
                caja.cerrar(vieja.id, CierreDeCaja(contado = "0", notes = "cierre de prueba"))
            }

            // 1. Abrir.
            val abierta = caja.abrir(AperturaDeCaja(apertura = "20000", nombreDeCaja = "caja-1"))
            assertTrue("abrir caja falló: ${detalle(abierta)}", abierta is Resultado.Ok)
            val sesionId = (abierta as Resultado.Ok).valor.id
            assertTrue("la caja abierta viene sin id", sesionId.isNotBlank())

            // 2. El arqueo de una caja recién abierta: lo que debería haber es
            //    exactamente lo que se puso.
            val recienAbierta = caja.arqueo(sesionId)
            assertTrue("arqueo falló: ${detalle(recienAbierta)}", recienAbierta is Resultado.Ok)
            mismoMonto(
                "20000",
                (recienAbierta as Resultado.Ok).valor.session.esperado,
                "closing_cash_expected de una caja recién abierta",
            )

            // 3. Sacar plata, con motivo.
            val retiro = caja.moverPlata(
                sesionId,
                NuevoMovimiento(
                    tipo = NuevoMovimiento.RETIRO,
                    amount = "2500",
                    reason = "le pagué al del pan",
                ),
            )
            assertTrue("el movimiento falló: ${detalle(retiro)}", retiro is Resultado.Ok)
            assertTrue("el movimiento volvió sin ser retiro", (retiro as Resultado.Ok).valor.esRetiro)

            // 4. Y queda en la lista, con su motivo.
            val movimientos = caja.movimientos(sesionId)
            assertTrue("listar movimientos falló: ${detalle(movimientos)}", movimientos is Resultado.Ok)
            val anotado = (movimientos as Resultado.Ok).valor.firstOrNull { it.id == retiro.valor.id }
            assertNotNull("el retiro no quedó en la lista de movimientos", anotado)
            assertEquals("le pagué al del pan", anotado!!.reason)

            // 5. El arqueo ya lo descontó. **Este número no se calcula en el
            //    teléfono**: si el server dijera otra cosa, la pantalla mostraría
            //    otra cosa, y acá es donde se nota.
            val conRetiro = caja.arqueo(sesionId)
            assertTrue("arqueo falló: ${detalle(conRetiro)}", conRetiro is Resultado.Ok)
            val estado = (conRetiro as Resultado.Ok).valor
            mismoMonto("17500", estado.session.esperado, "closing_cash_expected con el retiro")
            mismoMonto("2500", estado.salidas, "movements_out")
            mismoMonto("0", estado.entradas, "movements_in")

            // 6. Cerrar contando $2.500 menos de lo esperado.
            val cerrada = caja.cerrar(
                sesionId,
                CierreDeCaja(contado = "15000", notes = "le di mal el vuelto a un cliente"),
            )
            assertTrue("cerrar caja falló: ${detalle(cerrada)}", cerrada is Resultado.Ok)
            val cierre = (cerrada as Resultado.Ok).valor.session

            assertEquals("closed", cierre.status)
            mismoMonto("15000", cierre.contado, "closing_cash_counted")
            mismoMonto("17500", cierre.esperado, "closing_cash_expected del cierre")
            mismoMonto("-2500", cierre.discrepancia, "discrepancia")

            // 7. Y eso es lo que lee la dueña. El texto sale de la misma función
            //    que usa la pantalla, con el dato que acaba de mandar el server.
            val lectura = leerDiferencia(cierre.discrepancia)
            assertEquals(Cuadre.Falta, lectura.cuadre)

            val copy = copyDeDiferencia(
                moneda = Moneda.de("CLP"),
                contadoDelServidor = cierre.contado,
                esperadoDelServidor = cierre.esperado,
                discrepanciaDelServidor = cierre.discrepancia,
            )
            assertEquals("Faltan $2.500", copy.titular)
            assertEquals("Contaste $15.000 y el sistema tenía anotados $17.500.", copy.explicacion)

            // 8. Cerrada la caja, no hay ninguna abierta: el día siguiente empieza
            //    otra vez por abrir.
            val despues = caja.sesionAbierta()
            assertTrue("listar cajas abiertas falló: ${detalle(despues)}", despues is Resultado.Ok)
            assertEquals(
                "quedó una caja abierta después de cerrar",
                null,
                (despues as Resultado.Ok).valor,
            )
        }

        api.close()
    }

    /** Las cajas físicas configuradas, que es lo que se elige al abrir. */
    @Test
    fun `las cajas configuradas se pueden listar`() {
        val api = conSesion()
        assumeTrue("no hay pharma-api en $baseUrl con clave; test omitido", api != null)

        runBlocking {
            val cajas = CajaApi(api!!).cajas()
            assertTrue("listar cajas falló: ${detalle(cajas)}", cajas is Resultado.Ok)
            (cajas as Resultado.Ok).valor.forEach { caja ->
                assertTrue("una caja sin id no se puede elegir", caja.id.isNotBlank())
                assertTrue("una caja sin nombre no le dice nada a nadie", caja.name.isNotBlank())
            }
        }

        api!!.close()
    }
}

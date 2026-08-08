package cl.rutbusiness.app.ui.fiado

import cl.rutbusiness.core.money.Dinero
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
 * El fiado contra un `pharma-api` de verdad: quién debe, su cuenta, y un abono
 * que **queda**.
 *
 * Es lo único que atrapa un campo mal nombrado. `total_por_cobrar` escrito
 * `totalPorCobrar` se deserializa a `"0"`, compila, pasa todos los tests de UI,
 * y la pantalla dice "nadie te debe plata" en el teléfono de una dueña a la que
 * sí le deben. Un cero inventado se cobra.
 *
 * **Este test escribe**: registra un abono de verdad, del monto más chico que la
 * moneda permite. Se salta solo si no hay server escuchando o si no hay ningún
 * deudor a quien abonarle. Ver `CajaApiEnVivoTest` para cómo levantar el server.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class FiadoApiEnVivoTest {

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

    private fun monto(campo: String, texto: String): Dinero {
        assertTrue("$campo llegó vacío", texto.isNotBlank())
        val leido = Dinero.deTextoDeServidor(texto)
        assertNotNull("$campo = «$texto» no es un decimal que Dinero sepa leer", leido)
        return leido!!
    }

    @Test
    fun `quien me debe sale entero del server y en orden`() {
        val api = conSesion()
        assumeTrue("no hay pharma-api en $baseUrl con clave; test omitido", api != null)

        runBlocking {
            val r = FiadoApi(api!!).deudores()
            assertTrue("por-cobrar falló: ${detalle(r)}", r is Resultado.Ok)
            val reporte = (r as Resultado.Ok).valor

            monto("total_por_cobrar", reporte.total)
            assertTrue("debtor_count no puede ser negativo", reporte.cuantos >= 0)

            // El orden lo pone el server -- mayor saldo primero -- y la pantalla
            // lo respeta tal cual. Si el server dejara de ordenar, la lista
            // dejaría de contestar "¿quién me debe más?" sin que nadie lo note.
            var anterior: Dinero? = null
            reporte.rows.forEach { fila ->
                assertTrue("un deudor sin nombre no le sirve a nadie", fila.name.isNotBlank())
                assertTrue("un deudor sin id no se puede abrir", fila.customer.isNotBlank())
                val saldo = monto("balance de ${fila.name}", fila.balance)
                assertTrue(
                    "«${fila.name}» está en la lista de deudores con saldo ${fila.balance}",
                    saldo.unidades > 0L,
                )
                anterior?.let {
                    assertTrue(
                        "los deudores no vinieron ordenados de mayor a menor",
                        it >= saldo,
                    )
                }
                anterior = saldo
                assertNotNull(
                    "last_movement de ${fila.name} no se entiende: «${fila.ultimoMovimiento}»",
                    fechaCorta(fila.ultimoMovimiento),
                )
            }
        }

        api!!.close()
    }

    /**
     * El caso que define "listo": un abono que después de tocar el botón está en
     * el server.
     */
    @Test
    fun `un abono queda anotado y le baja la deuda al cliente`() {
        val api = conSesion()
        assumeTrue("no hay pharma-api en $baseUrl con clave; test omitido", api != null)
        val fiado = FiadoApi(api!!)

        runBlocking {
            val reporte = (fiado.deudores() as? Resultado.Ok)?.valor
            assumeTrue("no hay deudores a quien abonarle; test omitido", !reporte?.rows.isNullOrEmpty())
            val deudor = reporte!!.rows.first()

            val antes = fiado.cuenta(deudor.customer)
            assertTrue("la cuenta falló: ${detalle(antes)}", antes is Resultado.Ok)
            val saldoAntes = monto("balance", (antes as Resultado.Ok).valor.balance)

            // Un peso: el abono más chico que no puede pasarse de la deuda, así
            // el test no depende de cuánto deba nadie.
            val abono = fiado.registrarAbono(deudor.customer, NuevoAbono(amount = "1"))
            assertTrue("el abono falló: ${detalle(abono)}", abono is Resultado.Ok)
            val anotado = (abono as Resultado.Ok).valor
            assertTrue("el movimiento volvió sin ser un abono", anotado.esAbono)

            val despues = fiado.cuenta(deudor.customer)
            assertTrue("la cuenta falló: ${detalle(despues)}", despues is Resultado.Ok)
            val cuenta = (despues as Resultado.Ok).valor

            // Comparar no es calcular: el saldo nuevo lo hizo el server, acá sólo
            // se comprueba que bajó.
            val saldoDespues = monto("balance", cuenta.balance)
            assertTrue(
                "la deuda no bajó después del abono: era ${(antes).valor.balance} y quedó " +
                    "${cuenta.balance}",
                saldoDespues < saldoAntes,
            )

            // Y el movimiento está en la cuenta, que es donde la dueña lo va a
            // buscar si la red se cortó y no sabe si quedó.
            val enLaCuenta = cuenta.entries.firstOrNull { it.id == anotado.id }
            assertNotNull("el abono no quedó en los movimientos de la cuenta", enLaCuenta)
            assertEquals("abono", enLaCuenta!!.kind)
        }

        api.close()
    }
}

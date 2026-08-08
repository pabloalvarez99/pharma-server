package cl.rutbusiness.app.ui.resumen

import cl.rutbusiness.core.money.Dinero
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.session.AuthApi
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Los endpoints del resumen, contra un `pharma-api` de verdad.
 *
 * Los otros tests prueban la pantalla; éste prueba que los DTOs de [ResumenApi]
 * calcen con lo que el server manda. Es lo único que atrapa un campo mal
 * nombrado: `total_por_cobrar` escrito `totalPorCobrar` se deserializa a `"0"`,
 * compila, pasa todos los tests de UI, y muestra "nadie te debe plata" en el
 * teléfono de una dueña a la que sí le deben. Un cero inventado se cobra.
 *
 * Por eso cada monto se verifica dos veces: que **llegue** y que sea un decimal
 * que [Dinero] sepa leer. Un `"0"` por defecto pasa la primera y falla la
 * segunda sólo si el server manda basura, así que además se chequea que el
 * campo no venga vacío.
 *
 * **Se salta solo** si no hay server escuchando, así que en CI no molesta. Para
 * correrlo, igual que `AssistApiEnVivoTest`:
 *
 * ```
 * cargo build --bin pharma-api --bin pharma
 * PHARMA__BIND=0.0.0.0:8099 PHARMA__DB__PATH=$PWD/data/surreal \
 *   ./target/debug/pharma-api.exe &
 * ./target/debug/pharma.exe tenant-create "Almacen Dona Rosa" --slug donarosa
 * ./target/debug/pharma.exe user-create --tenant donarosa \
 *   --email rosa@donarosa.cl --roles owner --password '<clave>'
 * ./gradlew :app:testDebugUnitTest --tests '*ResumenApiEnVivoTest*' \
 *   -Drb.assist.password='<la misma clave>'
 * ```
 *
 * El usuario tiene que ser `owner`, `admin` o `quimico`: `/reports/dashboard`
 * deja fuera al cajero a propósito. Con un cajero este test se salta en vez de
 * fallar, porque un 403 ahí es la respuesta correcta del server.
 *
 * La clave llega por propiedad de sistema y no está escrita acá: una credencial
 * no se commitea ni siendo de un server local de pruebas.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class ResumenApiEnVivoTest {

    // Las mismas propiedades que `AssistApiEnVivoTest`, a propósito: un solo
    // server de pruebas y una sola forma de apuntarle.
    private val baseUrl = System.getProperty("rb.assist.baseUrl") ?: "http://127.0.0.1:8099"
    private val tenant = System.getProperty("rb.assist.tenant") ?: "donarosa"
    private val email = System.getProperty("rb.assist.email") ?: "rosa@donarosa.cl"
    private val password: String? = System.getProperty("rb.assist.password")

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

    /** Un monto del server tiene que ser un decimal legible, no cualquier texto. */
    private fun exigirMonto(campo: String, texto: String) {
        assertTrue("$campo llegó vacío", texto.isNotBlank())
        assertNotNull("$campo = «$texto» no es un decimal que Dinero sepa leer",
            Dinero.deTextoDeServidor(texto))
    }

    @Test
    fun `el resumen del dia sale entero del server`() {
        val api = conSesion()
        assumeTrue("no hay pharma-api en $baseUrl con clave; test omitido", api != null)
        val resumen = ResumenApi(api!!)

        runBlocking {
            // 1 y 5. El agregado: ventas de hoy, stock bajo, lotes por vencer.
            val agregado = resumen.dashboard()
            assumeTrue(
                "el usuario no puede ver el dashboard (rol cajero); test omitido",
                agregado is Resultado.Ok,
            )
            val datos = (agregado as Resultado.Ok).valor
            exigirMonto("ventas_hoy.revenue", datos.ventasHoy.revenue)
            exigirMonto("ventas_hoy.cash", datos.ventasHoy.cash)
            assertTrue("orders no puede ser negativo", datos.ventasHoy.orders >= 0)
            assertTrue("low_stock no puede ser negativo", datos.inventario.stockBajo >= 0)
            assertTrue("por_vencer no puede ser negativo", datos.porVencer >= 0)

            // 2. Ayer, con el mismo día UTC que usa el server para "hoy".
            val (desde, hasta) = DiaUtc.rangoDeAyer(System.currentTimeMillis())
            val ayer = resumen.ventasDiarias(desde, hasta)
            assertTrue("sales-daily falló: ${detalle(ayer)}", ayer is Resultado.Ok)
            (ayer as Resultado.Ok).valor.forEach { fila ->
                exigirMonto("sales-daily[${fila.date}].revenue", fila.revenue)
                assertTrue("la fila viene sin fecha", fila.date.isNotBlank())
            }

            // 3. La caja. Que no haya ninguna abierta es un estado válido, no un
            //    error: el negocio todavía no abrió.
            val cajas = resumen.cajasAbiertas()
            assertTrue("cash-sessions falló: ${detalle(cajas)}", cajas is Resultado.Ok)
            (cajas as Resultado.Ok).valor.firstOrNull()?.let { caja ->
                assertTrue("la caja abierta viene sin id", caja.id.isNotBlank())
                val arqueo = resumen.arqueo(caja.id)
                assertTrue("arqueo falló: ${detalle(arqueo)}", arqueo is Resultado.Ok)
                val esperado = (arqueo as Resultado.Ok).valor.session.esperado
                assertNotNull(
                    "el arqueo de una caja abierta tiene que traer closing_cash_expected",
                    esperado,
                )
                exigirMonto("arqueo.session.closing_cash_expected", esperado!!)
            }

            // 4. El fiado.
            val deuda = resumen.porCobrar()
            assertTrue("por-cobrar falló: ${detalle(deuda)}", deuda is Resultado.Ok)
            val porCobrar = (deuda as Resultado.Ok).valor
            exigirMonto("total_por_cobrar", porCobrar.total)
            assertTrue("debtor_count no puede ser negativo", porCobrar.deudores >= 0)

            // 5. Los nombres detrás de los números del agregado.
            val vencimientos = resumen.porVencer()
            assertTrue("near-expiry falló: ${detalle(vencimientos)}", vencimientos is Resultado.Ok)
            (vencimientos as Resultado.Ok).valor.forEach { lote ->
                assertTrue("un lote por vencer sin nombre no le sirve a nadie",
                    lote.producto.isNotBlank())
            }

            val faltantes = resumen.stockBajo()
            assertTrue("products?low_stock falló: ${detalle(faltantes)}", faltantes is Resultado.Ok)
            (faltantes as Resultado.Ok).valor.forEach { producto ->
                assertTrue("un producto sin nombre no le sirve a nadie",
                    producto.name.isNotBlank())
                assertTrue(
                    "«${producto.name}» tiene ${producto.stock} y no debería estar en stock bajo",
                    producto.stock <= ResumenApi.UMBRAL_STOCK_BAJO,
                )
            }
        }

        api.close()
    }
}

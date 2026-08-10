package cl.rutbusiness.app.respaldo

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import cl.rutbusiness.core.backup.PruebaDeRetiro
import cl.rutbusiness.core.backup.UserBackupApi
import cl.rutbusiness.core.backup.claveNuevaDelNegocio
import cl.rutbusiness.core.backup.parsearMaterialRecuperacion
import cl.rutbusiness.core.backup.prepararRespaldoDesdeCola
import cl.rutbusiness.core.backup.rescatarRespaldoDesdeCero
import cl.rutbusiness.core.net.ApiFactory
import cl.rutbusiness.core.net.Resultado
import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.MedioDePago
import cl.rutbusiness.core.pos.SolicitudDeVenta
import cl.rutbusiness.core.session.AuthApi
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL

/**
 * Restauración desde cero contra un server de verdad, en el aparato de verdad
 * (ADR-0023).
 *
 * **Por qué esta prueba tiene que correr en un teléfono y no en la JVM.** Lo
 * que se está afirmando es que una persona que perdió el aparato recupera su
 * negocio con una tarjeta de papel. Las dos piezas que hacen eso posible son
 * PBKDF2 y AES-GCM, y en Android las dos las provee el proveedor de seguridad
 * de la plataforma, no la biblioteca común. Una prueba en la JVM del escritorio
 * ejercita otra implementación: exactamente el error que dejó pasar meses el
 * bug de entropía del `PBEKeySpec`, donde los vectores estándar en ASCII pasaban
 * mientras las semillas reales de 11 bytes colisionaban.
 *
 * **Qué modela el "teléfono nuevo".** El rescate se hace con un [ApiFactory]
 * recién construido cuyo proveedor de token devuelve `null` — no hay sesión, no
 * hay token, no hay nada guardado de la fase anterior. Si esta prueba pasara
 * reusando el cliente de la subida, no probaría nada: probaría que un aparato
 * con sesión puede bajar su propio respaldo, que es el caso que **no** importa.
 *
 * **Necesita un server escuchando.** Sin él se salta ([assumeTrue]) en vez de
 * fallar: una prueba de integración que rompe la build cuando falta la
 * dependencia externa termina siempre igual, desactivada.
 *
 * ```powershell
 * # En el host, con backend en memoria y sin freno de frecuencia:
 * $env:PHARMA__USER_BACKUP__BACKEND = "memory"
 * $env:PHARMA__USER_BACKUP__MIN_SECONDS_BETWEEN_UPLOADS = "0"
 * cargo run -p api
 * # Y después:
 * ./gradlew :app:connectedDebugAndroidTest
 * ```
 *
 * `10.0.2.2` es como el emulador ve el `localhost` del host. Se puede apuntar a
 * otro lado con `-Pandroid.testInstrumentationRunnerArguments.rbServer=...`.
 */
@RunWith(AndroidJUnit4::class)
class RespaldoDesdeCeroTest {

    private val baseUrl: String by lazy {
        InstrumentationRegistry.getArguments().getString("rbServer")
            ?: "http://10.0.2.2:8080"
    }

    private val negocioSlug = "feria-e2e"
    private val email = "duena@e2e.local"

    /**
     * Clave del negocio de laboratorio que **esta misma prueba crea** en un
     * server efímero. No abre nada que exista fuera de esta corrida: no es una
     * credencial archivada, es parte del fixture. Se puede pisar con
     * `-Pandroid.testInstrumentationRunnerArguments.rbPassword=…` si alguien
     * apunta la prueba a un server que no es descartable — que igual no
     * debería.
     */
    private val password: String by lazy {
        InstrumentationRegistry.getArguments().getString("rbPassword")
            ?: "laboratorio-descartable"
    }

    @Test
    fun el_negocio_vuelve_en_un_telefono_sin_sesion_con_la_tarjeta_del_cuaderno() =
        runBlocking {
            val token = tokenDelNegocio()
            assumeTrue("no hay server en $baseUrl", token != null)

            // --- Teléfono viejo: cobra sin señal y respalda -------------------
            val clave = claveNuevaDelNegocio()
            val tarjeta = clave.fraseCompleta()
            val material = parsearMaterialRecuperacion(tarjeta).getOrThrow()

            val ahora = System.currentTimeMillis()
            val venta = VentaEnCola(
                clave = "e2e-$ahora",
                solicitud = SolicitudDeVenta(
                    items = listOf(
                        LineaDeVenta(
                            product = "product:palta",
                            productName = "Palta kilo",
                            quantity = 2,
                            unitPrice = "3500",
                        ),
                    ),
                    paymentMethod = MedioDePago.Efectivo.codigo,
                    cashAmount = "10000",
                ),
                cobradaEn = ahora,
                lineas = 1,
            )

            val prep = prepararRespaldoDesdeCola(
                tenantId = negocioSlug,
                cola = listOf(venta),
                createdAtUnix = ahora / 1000L,
                rubro = "feria",
                materialRecuperacion = material,
                tenantSlug = negocioSlug,
            ).getOrThrow()

            val sobre = prep.sobre
            assertNotNull("sin sobre no hay nada que subir", sobre)
            assertNotNull(
                "sin hash de retiro el sobre no se puede bajar desde un teléfono nuevo",
                prep.retrievalHashHex,
            )

            val vieja = ApiFactory(baseUrl) { token }
            val subida = UserBackupApi(vieja).subirSobre(sobre!!, prep.retrievalHashHex)
            assumeTrue(
                "el server no tiene bucket cableado (backend=none): " +
                    "correr con PHARMA__USER_BACKUP__BACKEND=memory",
                subida is Resultado.Ok && (subida as Resultado.Ok).valor.accepted,
            )
            vieja.close()

            // --- Teléfono nuevo: nada guardado, sólo la tarjeta ---------------
            // Cliente recién nacido y sin token. No comparte estado con el de
            // arriba: es lo que hace que esto sea "desde cero" y no "de nuevo".
            val nueva = ApiFactory(baseUrl) { null }
            val rescate = rescatarRespaldoDesdeCero(
                backup = UserBackupApi(nueva),
                tenantSlug = negocioSlug,
                materialRaw = tarjeta,
            )

            val vuelta = rescate.getOrElse {
                throw AssertionError("el respaldo no volvió: ${it.message}")
            }
            assertEquals(1, vuelta.snapshot.pendingSales.size)
            val recuperada = vuelta.snapshot.pendingSales.first()
            assertEquals(venta.clave, recuperada.clave)
            assertEquals("10000", recuperada.solicitud.cashAmount)
            assertEquals("Palta kilo", recuperada.solicitud.items.first().productName)
            assertEquals(2, recuperada.solicitud.items.first().quantity)
            assertEquals("feria", vuelta.snapshot.rubro)

            // --- La tarjeta de otro negocio no abre éste -----------------------
            val ajena = claveNuevaDelNegocio().fraseCompleta()
            val fallido = rescatarRespaldoDesdeCero(
                backup = UserBackupApi(nueva),
                tenantSlug = negocioSlug,
                materialRaw = ajena,
            )
            assertTrue("una tarjeta ajena abrió el negocio", fallido.isFailure)

            nueva.close()
        }

    /**
     * Que dos aparatos con la misma tarjeta y el mismo slug lleguen al mismo
     * hash es lo único que hace posible el rescate: el que sube y el que baja
     * nunca son el mismo teléfono. Se verifica en el aparato porque el que
     * calcula el hash de la subida es el proveedor de seguridad de Android.
     */
    @Test
    fun la_prueba_de_retiro_es_la_misma_en_dos_derivaciones() = runBlocking {
        val material = parsearMaterialRecuperacion(
            claveNuevaDelNegocio().fraseCompleta(),
        ).getOrThrow()

        val a = PruebaDeRetiro.derivar(material, negocioSlug).getOrThrow()
        val b = PruebaDeRetiro.derivar(material, "  FERIA-E2E  ").getOrThrow()
        assertEquals(PruebaDeRetiro.hashHex(a), PruebaDeRetiro.hashHex(b))

        val otra = PruebaDeRetiro.derivar(material, "otro-negocio").getOrThrow()
        assertTrue(
            "el mismo cuaderno en otro negocio da la misma prueba",
            PruebaDeRetiro.hashHex(a) != PruebaDeRetiro.hashHex(otra),
        )
    }

    /**
     * Token del negocio de prueba: lo crea en la primera corrida (`/setup` sólo
     * funciona con la base vacía) y entra normal en las siguientes.
     *
     * `null` cuando no hay server escuchando — la prueba se salta.
     */
    private suspend fun tokenDelNegocio(): String? {
        val creado = intentarSetup()
        if (creado != null) return creado
        val api = ApiFactory(baseUrl) { null }
        return try {
            when (val r = AuthApi(api).login(negocioSlug, email, password)) {
                is Resultado.Ok -> r.valor.token
                is Resultado.Falla -> null
            }
        } finally {
            api.close()
        }
    }

    /** `POST /api/v1/setup`. Devuelve el token, o `null` si ya hay usuarios. */
    private fun intentarSetup(): String? {
        val cuerpo = JSONObject()
            .put("business_name", "Feria E2E")
            .put("tenant_slug", negocioSlug)
            .put("email", email)
            .put("password", password)
            .put("vertical", "feria")
            .toString()

        return try {
            val con = (URL("$baseUrl/api/v1/setup").openConnection() as HttpURLConnection).apply {
                requestMethod = "POST"
                connectTimeout = 3_000
                readTimeout = 10_000
                doOutput = true
                setRequestProperty("Content-Type", "application/json")
            }
            con.outputStream.use { it.write(cuerpo.toByteArray()) }
            if (con.responseCode !in 200..299) {
                con.disconnect()
                return null
            }
            val texto = con.inputStream.bufferedReader().use { it.readText() }
            con.disconnect()
            JSONObject(texto).optString("token").takeIf { it.isNotBlank() }
        } catch (e: IOException) {
            null
        }
    }

    /**
     * El sobre que viaja no contiene el texto del negocio.
     *
     * Es la misma afirmación que fija la prueba de servidor sobre las rutas,
     * pero del lado del que arma el paquete: si un día alguien "optimiza"
     * mandando el snapshot sin cifrar, esto lo agarra en el aparato.
     */
    @Test
    fun el_paquete_que_sale_del_telefono_no_lleva_el_negocio_en_claro() {
        val material = parsearMaterialRecuperacion(
            claveNuevaDelNegocio().fraseCompleta(),
        ).getOrThrow()
        val marca = "Palta kilo E2E"
        val prep = prepararRespaldoDesdeCola(
            tenantId = negocioSlug,
            cola = listOf(
                VentaEnCola(
                    clave = "marca",
                    solicitud = SolicitudDeVenta(
                        items = listOf(
                            LineaDeVenta("product:x", marca, 1, "1000"),
                        ),
                        paymentMethod = MedioDePago.Efectivo.codigo,
                    ),
                    cobradaEn = 1L,
                    lineas = 1,
                ),
            ),
            createdAtUnix = 1L,
            materialRecuperacion = material,
            tenantSlug = negocioSlug,
        ).getOrThrow()

        val bytes = prep.sobre!!.envelopeBytes
        assertNull(
            "el nombre del producto viajó en claro",
            bytes.decodeToString().indexOf(marca).takeIf { it >= 0 },
        )
    }
}

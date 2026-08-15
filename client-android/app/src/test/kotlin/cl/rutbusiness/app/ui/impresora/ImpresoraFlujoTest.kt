package cl.rutbusiness.app.ui.impresora

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.unit.Density
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Los caminos de falla de la impresora, que son el trabajo de verdad de este
 * encargo.
 *
 * Una impresora térmica falla seguido y por motivos aburridos: se quedó sin
 * papel, la desenchufaron, se la llevaron a la otra punta del local, Android
 * revocó el permiso. Lo que **no** puede pasar nunca es que alguna de esas
 * cosas se lleve puesta una venta ya cobrada. Cada prueba de acá mira las dos
 * cosas: que la dueña lea algo que le dice qué hacer, y que siempre tenga una
 * salida que no es arreglar la impresora.
 *
 * Todo con dobles: el emulador no tiene Bluetooth y la impresora física no
 * estuvo disponible, así que la única forma honesta de cubrir estos caminos es
 * simulando lo que contesta el teléfono. Lo que **no** cubre esto está dicho en
 * el reporte, sin adornos.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class ImpresoraFlujoTest {

    @get:Rule
    val compose = createComposeRule()

    private val laImpresora = ImpresoraConocida("00:11:22:33:44:55", "Epson TM-T20")
    private val losAudifonos = ImpresoraConocida("AA:BB:CC:DD:EE:FF", "Audífonos de Rosa")

    private val boleta = BoletaImprimible(
        orderId = "order:abc",
        folio = "1042",
        datetime = "2026-08-07T00:25:31Z",
        tenantName = "Almacén Doña Rosa",
        items = listOf(LineaDeBoleta("Arroz Grado 1", 2, "1290", "2580")),
        subtotal = "2580",
        discount = "0",
        total = "2580",
        paymentMethod = "pos_cash",
        cashAmount = "3000",
        change = "420",
        footerNote = "Gracias por su compra",
    )

    // --- dobles ------------------------------------------------------------

    private class EnlaceFalso(
        override val permisosQuePedir: List<String> = listOf("android.permission.BLUETOOTH_CONNECT"),
        var faltaElPermiso: Boolean = false,
        var tieneRadio: Boolean = true,
        var encendido: Boolean = true,
        var lista: Intento<List<ImpresoraConocida>>? = null,
        var alImprimir: Intento<Unit> = Intento.Ok(Unit),
    ) : EnlaceDeImpresora {
        var impresiones = 0
            private set
        var ultimaImpresora: ImpresoraElegida? = null
            private set
        var ultimosBytes: ByteArray? = null
            private set

        override fun faltaPermiso() = faltaElPermiso
        override fun hayBluetooth() = tieneRadio
        override fun bluetoothEncendido() = encendido
        override fun desfaseHorarioMinutos() = -240

        override fun emparejadas(): Intento<List<ImpresoraConocida>> = lista ?: when {
            !tieneRadio -> Intento.Fallo(FallaDeImpresion.SinBluetooth())
            faltaElPermiso -> Intento.Fallo(FallaDeImpresion.FaltaPermiso())
            !encendido -> Intento.Fallo(FallaDeImpresion.BluetoothApagado())
            else -> Intento.Ok(emptyList())
        }

        override suspend fun imprimir(
            impresora: ImpresoraElegida,
            bytes: ByteArray,
        ): Intento<Unit> {
            impresiones++
            ultimaImpresora = impresora
            ultimosBytes = bytes
            return alImprimir
        }
    }

    private class PreferenciasFalsas(var guardada: ImpresoraElegida? = null) :
        PreferenciasDeImpresora {
        var ultima: String? = null
        override fun leer() = guardada
        override fun guardar(impresora: ImpresoraElegida) { guardada = impresora }
        override fun olvidar() { guardada = null }
        override fun leerUltimaBoleta() = ultima
        override fun guardarUltimaBoleta(ordenId: String) { ultima = ordenId }
    }

    /** Devuelve la boleta y **nada más**: no puede tocar la venta ni queriendo. */
    private inner class BoletasFalsas(private val falla: FallaDeImpresion? = null) :
        FuenteDeBoletas {
        var pedidos = mutableListOf<String>()
        override suspend fun boleta(ordenId: String): Intento<BoletaImprimible> {
            pedidos += ordenId
            return falla?.let { Intento.Fallo(it) } ?: Intento.Ok(boleta.copy(orderId = ordenId))
        }
    }

    private fun armar(
        enlace: EnlaceFalso = EnlaceFalso(),
        preferencias: PreferenciasFalsas = PreferenciasFalsas(),
        boletas: FuenteDeBoletas = BoletasFalsas(),
    ) = ImpresoraViewModel(boletas = boletas, enlace = enlace, preferencias = preferencias)

    private fun mostrar(vm: ImpresoraViewModel, escala: Float = 1.0f, ordenId: String? = "order:abc") {
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(base.density, fontScale = escala),
            ) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                        TarjetaDeImpresion(vm = vm, ordenId = ordenId)
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    private fun mostrarReimpresion(vm: ImpresoraViewModel) {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                    TarjetaDeReimpresion(vm = vm, onCerrar = {})
                }
            }
        }
        compose.waitForIdle()
    }

    /**
     * Toca el **botón** que dice esto.
     *
     * Se exige acción de click a propósito: varias frases aparecen dos veces,
     * en el texto que explica y en el botón que resuelve ("toca Reintentar" y
     * el botón «Reintentar»). Buscar sin filtrar encontraría los dos.
     */
    private fun tocar(texto: String) {
        val botones = compose.onAllNodes(hasText(texto, substring = true) and hasClickAction())
        assertTrue(
            "no hay ningún botón que diga «$texto»",
            botones.fetchSemanticsNodes().isNotEmpty(),
        )
        botones[0].performScrollTo().performClick()
        compose.waitForIdle()
    }

    private fun seVe(texto: String) {
        val nodos = compose.onAllNodes(hasText(texto, substring = true))
        assertTrue("no aparece «$texto» en pantalla", nodos.fetchSemanticsNodes().isNotEmpty())
        nodos[0].performScrollTo().assertIsDisplayed()
    }

    /**
     * Lo que tiene que estar sí o sí en cualquier falla: una salida que no pasa
     * por arreglar la impresora, y la frase que dice que la plata está a salvo.
     */
    private fun laVentaSobrevive() {
        seVe("Seguir sin boleta")
        seVe("La venta ya está cobrada y guardada")
    }

    // --- camino de falla 1: sin permiso ------------------------------------

    @Test
    fun `sin permiso de bluetooth explica que hacer y ofrece darlo`() {
        val enlace = EnlaceFalso(faltaElPermiso = true)
        val vm = armar(enlace = enlace)
        mostrar(vm)

        tocar("Imprimir boleta")

        seVe("Falta darle permiso al Bluetooth")
        seVe("Toca «Dar permiso» y acepta")
        // Y el camino de salida cuando Android ya dejó de preguntar. El nombre
        // es el del lanzador —RutAgent, no el interno—, porque así lista la app
        // Ajustes; que siga siendo el mismo lo fija `NombreDelProductoTest`.
        seVe("Ajustes › Aplicaciones › RutAgent › Permisos")
        seVe("Dar permiso")
        laVentaSobrevive()

        assertEquals("sin permiso no se puede haber mandado nada", 0, enlace.impresiones)
    }

    // --- camino de falla 2: impresora apagada / fuera de alcance -----------

    @Test
    fun `la impresora apagada o lejos dice las dos cosas y deja reintentar`() {
        val enlace = EnlaceFalso(
            alImprimir = Intento.Fallo(FallaDeImpresion.NoContesta("Epson TM-T20", "socket")),
        )
        val vm = armar(
            enlace = enlace,
            preferencias = PreferenciasFalsas(
                ImpresoraElegida(laImpresora.direccion, laImpresora.nombre, AnchoDePapel.Mm58),
            ),
        )
        mostrar(vm)

        tocar("Imprimir boleta")

        seVe("«Epson TM-T20» no contesta")
        // Apagada y fuera de alcance se ven idénticas desde el teléfono: se
        // dicen las dos en vez de mandar a revisar lo que no era.
        seVe("Puede estar apagada o quedar muy lejos")
        seVe("Reintentar")
        laVentaSobrevive()
    }

    @Test
    fun `reintentar vuelve a mandar la misma boleta, nunca a cobrar`() {
        val enlace = EnlaceFalso(
            alImprimir = Intento.Fallo(FallaDeImpresion.NoContesta("Epson TM-T20")),
        )
        val boletas = BoletasFalsas()
        val vm = armar(
            enlace = enlace,
            preferencias = PreferenciasFalsas(
                ImpresoraElegida(laImpresora.direccion, laImpresora.nombre, AnchoDePapel.Mm58),
            ),
            boletas = boletas,
        )
        mostrar(vm)

        tocar("Imprimir boleta")
        tocar("Reintentar")

        assertEquals(2, enlace.impresiones)
        assertEquals(
            "reintentar tiene que pedir la MISMA venta",
            listOf("order:abc", "order:abc"),
            boletas.pedidos,
        )
    }

    // --- camino de falla 3: bluetooth apagado y sin papel -------------------

    @Test
    fun `el bluetooth apagado manda a encenderlo`() {
        val vm = armar(enlace = EnlaceFalso(encendido = false))
        mostrar(vm)

        tocar("Imprimir boleta")

        seVe("El Bluetooth está apagado")
        seVe("deslizando desde arriba")
        laVentaSobrevive()
    }

    @Test
    fun `sin papel manda a cambiar el rollo`() {
        val vm = armar(
            enlace = EnlaceFalso(alImprimir = Intento.Fallo(FallaDeImpresion.SinPapel("Epson TM-T20"))),
            preferencias = PreferenciasFalsas(
                ImpresoraElegida(laImpresora.direccion, laImpresora.nombre, AnchoDePapel.Mm58),
            ),
        )
        mostrar(vm)

        tocar("Imprimir boleta")

        seVe("«Epson TM-T20» se quedó sin papel")
        seVe("Cambia el rollo")
        laVentaSobrevive()
    }

    @Test
    fun `sin ninguna impresora emparejada ensena como emparejar`() {
        val vm = armar(enlace = EnlaceFalso(lista = Intento.Fallo(FallaDeImpresion.NingunaEmparejada())))
        mostrar(vm)

        tocar("Imprimir boleta")

        seVe("Todavía no hay ninguna impresora")
        seVe("Ajustes › Bluetooth")
        laVentaSobrevive()
    }

    /** Un teléfono sin radio no puede reintentar nada, y se dice así. */
    @Test
    fun `sin radio bluetooth no se ofrece reintentar en falso`() {
        val vm = armar(enlace = EnlaceFalso(tieneRadio = false, lista = null))
        mostrar(vm)

        tocar("Imprimir boleta")

        seVe("Este teléfono no tiene Bluetooth")
        seVe("La venta igual quedó registrada")
        assertTrue(
            "no puede ofrecer reintentar algo que no existe",
            compose.onAllNodes(hasText("Reintentar", substring = true)).fetchSemanticsNodes().isEmpty(),
        )
        seVe("Seguir sin boleta")
    }

    // --- la venta nunca se pierde ------------------------------------------

    @Test
    fun `seguir sin boleta deja la venta guardada y no deshace nada`() {
        val boletas = BoletasFalsas()
        val enlace = EnlaceFalso(
            alImprimir = Intento.Fallo(FallaDeImpresion.NoContesta("Epson TM-T20")),
        )
        val vm = armar(
            enlace = enlace,
            preferencias = PreferenciasFalsas(
                ImpresoraElegida(laImpresora.direccion, laImpresora.nombre, AnchoDePapel.Mm58),
            ),
            boletas = boletas,
        )
        mostrar(vm)

        tocar("Imprimir boleta")
        tocar("Seguir sin boleta")

        seVe("Seguiste sin boleta. La venta quedó guardada igual.")
        // Y sigue habiendo una forma de imprimirla más tarde.
        seVe("Probar de imprimir")

        // La única llamada que salió de acá fue LEER el comprobante. Ni una
        // escritura, ni un reintento de cobro, ni una anulación.
        assertEquals(listOf("order:abc"), boletas.pedidos)
    }

    // --- elegir y recordar la impresora -------------------------------------

    @Test
    fun `la primera vez pregunta cual es la impresora y despues la recuerda`() {
        val enlace = EnlaceFalso(lista = Intento.Ok(listOf(laImpresora, losAudifonos)))
        val prefs = PreferenciasFalsas()
        val vm = armar(enlace = enlace, preferencias = prefs)
        mostrar(vm)

        // Se avisa antes de tocar: el primer toque abre una lista, no imprime.
        seVe("La primera vez te vamos a preguntar cuál es tu impresora")
        assertNull("todavía no hay nada guardado", prefs.guardada)

        tocar("Imprimir boleta")
        seVe("¿Cuál es tu impresora?")
        // Se muestran todas las emparejadas, no sólo las que Android clasifica
        // como impresora: más de un clon barato se declara sin categoría.
        seVe("Audífonos de Rosa")

        tocar("Epson TM-T20")
        seVe("¿De qué ancho es el papel")
        tocar("58 mm")

        assertEquals(
            ImpresoraElegida(laImpresora.direccion, laImpresora.nombre, AnchoDePapel.Mm58),
            prefs.guardada,
        )
        // Y la boleta que estaba esperando sale sola: la dueña tocó "Imprimir",
        // no "Configurar".
        assertEquals(1, enlace.impresiones)
        assertEquals(AnchoDePapel.Mm58, enlace.ultimaImpresora?.ancho)
    }

    @Test
    fun `elegir 80 mm imprime con 48 columnas`() {
        val enlace = EnlaceFalso(lista = Intento.Ok(listOf(laImpresora)))
        val vm = armar(enlace = enlace)
        mostrar(vm)

        tocar("Imprimir boleta")
        tocar("Epson TM-T20")
        tocar("80 mm")

        assertEquals(AnchoDePapel.Mm80, enlace.ultimaImpresora?.ancho)
        val texto = String(
            enlace.ultimosBytes!!.map { (it.toInt() and 0xFF).toChar() }.toCharArray(),
        )
        assertTrue("la boleta de 80 mm separa con 48 guiones", texto.contains("-".repeat(48)))
    }

    @Test
    fun `con la impresora ya elegida imprime directo y lo dice`() {
        val enlace = EnlaceFalso()
        val vm = armar(
            enlace = enlace,
            preferencias = PreferenciasFalsas(
                ImpresoraElegida(laImpresora.direccion, laImpresora.nombre, AnchoDePapel.Mm58),
            ),
        )
        mostrar(vm)

        seVe("Sale por «Epson TM-T20», rollo de 58 mm.")
        tocar("Imprimir boleta")

        seVe("Boleta impresa.")
        assertEquals(1, enlace.impresiones)
    }

    // --- reimprimir ---------------------------------------------------------

    @Test
    fun `reimprimir vuelve a pedir la misma venta al server`() {
        val boletas = BoletasFalsas()
        val prefs = PreferenciasFalsas(
            ImpresoraElegida(laImpresora.direccion, laImpresora.nombre, AnchoDePapel.Mm58),
        ).apply { ultima = "order:vieja" }
        val enlace = EnlaceFalso()
        val vm = armar(enlace = enlace, preferencias = prefs, boletas = boletas)

        mostrarReimpresion(vm)

        seVe("Imprimir de nuevo")
        seVe("Reimprimir no vuelve a cobrar")
        tocar("Imprimir de nuevo")

        assertEquals(listOf("order:vieja"), boletas.pedidos)
        assertEquals(1, enlace.impresiones)
    }

    @Test
    fun `sin ninguna boleta previa no promete reimprimir`() {
        val vm = armar()
        mostrarReimpresion(vm)

        seVe("Todavía no imprimiste ninguna boleta desde este teléfono")
    }

    // --- no se puede traer la boleta ----------------------------------------

    @Test
    fun `si el server no manda el comprobante la venta igual esta cobrada`() {
        val vm = armar(
            boletas = BoletasFalsas(FallaDeImpresion.SinDatosDeLaBoleta("timeout")),
            preferencias = PreferenciasFalsas(
                ImpresoraElegida(laImpresora.direccion, laImpresora.nombre, AnchoDePapel.Mm58),
            ),
        )
        mostrar(vm)

        tocar("Imprimir boleta")

        seVe("No pudimos traer la boleta")
        seVe("La venta está cobrada y guardada")
        laVentaSobrevive()
    }

    /** Sin comprobante no hay id, y se dice en vez de ofrecer un botón muerto. */
    @Test
    fun `sin id de venta no se ofrece imprimir`() {
        val vm = armar()
        mostrar(vm, ordenId = null)

        seVe("Todavía no tenemos el detalle de esta venta")
        assertTrue(
            compose.onAllNodes(hasText("Imprimir boleta")).fetchSemanticsNodes().isEmpty(),
        )
    }
}

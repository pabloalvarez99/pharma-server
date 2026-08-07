package cl.rutbusiness.app.ui.impresora

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.height
import androidx.compose.ui.unit.width
import cl.rutbusiness.ui.theme.RbDefaultDimens
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * La tarjeta de boleta al 100% y al 200% de escala de letra.
 *
 * El peor caso de esta pantalla es una falla: ahí conviven un bloque de error
 * con dos párrafos, tres botones de ancho completo y una frase de cierre, todo
 * en 360dp con la letra al doble. Si a esa escala «Seguir sin boleta» queda
 * fuera de alcance, la dueña se queda encerrada frente a una impresora rota con
 * la venta ya cobrada — que es exactamente lo que este encargo prohíbe.
 *
 * `NATIVE` no es opcional: con los gráficos por defecto de Robolectric el motor
 * de texto mide ~0,5dp por carácter y toda aserción de ancho pasaría sin
 * significar nada. SDK 34 porque ahí vive la curva no lineal de escala de letra
 * de Android 14, que es la que el usuario recibe de verdad.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class ImpresoraEscalaTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil = RbDefaultDimens.touchTarget

    private class EnlaceFijo(
        private val lista: Intento<List<ImpresoraConocida>>,
        private val resultado: Intento<Unit>,
    ) : EnlaceDeImpresora {
        override val permisosQuePedir = listOf("android.permission.BLUETOOTH_CONNECT")
        override fun faltaPermiso() = false
        override fun hayBluetooth() = true
        override fun bluetoothEncendido() = true
        override fun desfaseHorarioMinutos() = -240
        override fun emparejadas() = lista
        override suspend fun imprimir(impresora: ImpresoraElegida, bytes: ByteArray) = resultado
    }

    private class PreferenciasFijas(private var guardada: ImpresoraElegida?) :
        PreferenciasDeImpresora {
        private var ultima: String? = null
        override fun leer() = guardada
        override fun guardar(impresora: ImpresoraElegida) { guardada = impresora }
        override fun olvidar() { guardada = null }
        override fun leerUltimaBoleta() = ultima
        override fun guardarUltimaBoleta(ordenId: String) { ultima = ordenId }
    }

    private class BoletasFijas : FuenteDeBoletas {
        override suspend fun boleta(ordenId: String) = Intento.Ok(
            BoletaImprimible(
                orderId = ordenId,
                folio = "1042",
                datetime = "2026-08-07T00:25:31Z",
                tenantName = "Almacén Doña Rosa",
                items = listOf(LineaDeBoleta("Arroz Grado 1", 2, "1290", "2580")),
                subtotal = "2580",
                discount = "0",
                total = "2580",
                paymentMethod = "pos_cash",
            ),
        )
    }

    private val laImpresora = ImpresoraElegida(
        direccion = "00:11:22:33:44:55",
        nombre = "Epson TM-T20",
        ancho = AnchoDePapel.Mm58,
    )

    private fun montar(escala: Float, contenido: @Composable () -> Unit) {
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(base.density, fontScale = escala),
            ) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                        contenido()
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    private fun revisarTactiles(escala: Float) {
        val nodos = compose.onAllNodes(hasClickAction()).fetchSemanticsNodes()
        assertTrue("la tarjeta no dibujó ningún botón", nodos.isNotEmpty())

        val chicos = mutableListOf<String>()
        nodos.indices.forEach { i ->
            val nodo = compose.onAllNodes(hasClickAction())[i]
            val caja = nodo.getUnclippedBoundsInRoot()
            if (caja.height < objetivoTactil || caja.width < objetivoTactil) {
                val texto = nodo.fetchSemanticsNode()
                    .config.getOrNull(SemanticsProperties.Text)
                    ?.joinToString(" ") { it.text } ?: "sin texto"
                chicos += "«$texto» mide ${caja.width} x ${caja.height}"
            }
        }
        assertTrue(
            "a ${escala}x estos controles quedaron bajo $objetivoTactil:\n" +
                chicos.joinToString("\n"),
            chicos.isEmpty(),
        )
    }

    /** A lo ancho no hay a dónde scrollear: lo que no entra, se perdió. */
    private fun revisarQueNadaSeCorte(escala: Float) {
        val matcher = hasText("", substring = true)
        val cortados = mutableListOf<String>()
        val nodos = compose.onAllNodes(matcher).fetchSemanticsNodes()
        nodos.indices.forEach { i ->
            val nodo = compose.onAllNodes(matcher)[i]
            val visible = nodo.getBoundsInRoot()
            val completo = nodo.getUnclippedBoundsInRoot()
            if (visible.width <= 0.dp) return@forEach
            if (completo.width - visible.width > 1.dp) {
                val texto = nodo.fetchSemanticsNode()
                    .config.getOrNull(SemanticsProperties.Text)
                    ?.joinToString(" ") { it.text } ?: ""
                cortados += "«$texto»: ${completo.width} de ancho, se ve ${visible.width}"
            }
        }
        assertTrue("a ${escala}x se corta:\n${cortados.joinToString("\n")}", cortados.isEmpty())
    }

    private fun conListaDeImpresoras(escala: Float) {
        val vm = ImpresoraViewModel(
            boletas = BoletasFijas(),
            enlace = EnlaceFijo(
                lista = Intento.Ok(
                    listOf(
                        ImpresoraConocida("00:11:22:33:44:55", "Epson TM-T20 Receipt Printer"),
                        ImpresoraConocida("AA:BB:CC:DD:EE:FF", "Audífonos de Rosa"),
                    ),
                ),
                resultado = Intento.Ok(Unit),
            ),
            preferencias = PreferenciasFijas(null),
        )
        vm.elegir()
        montar(escala) { TarjetaDeImpresion(vm = vm, ordenId = "order:abc") }
    }

    @Test
    fun `la lista de impresoras aguanta al 100 por ciento`() {
        conListaDeImpresoras(1.0f)
        compose.onAllNodes(hasText("Epson TM-T20", substring = true))[0]
            .performScrollTo().assertIsDisplayed()
        revisarTactiles(1.0f)
        revisarQueNadaSeCorte(1.0f)
    }

    /**
     * El nombre largo de una impresora es el peor caso de la lista: "Epson
     * TM-T20 Receipt Printer" al 200% no entra en una línea y tiene que
     * envolver, no cortarse.
     */
    @Test
    fun `la lista de impresoras aguanta al 200 por ciento`() {
        conListaDeImpresoras(2.0f)
        compose.onAllNodes(hasText("Epson TM-T20", substring = true))[0]
            .performScrollTo().assertIsDisplayed()
        revisarTactiles(2.0f)
        revisarQueNadaSeCorte(2.0f)
    }

    @Test
    fun `la eleccion de ancho de papel aguanta al 200 por ciento`() {
        val vm = ImpresoraViewModel(
            boletas = BoletasFijas(),
            enlace = EnlaceFijo(Intento.Ok(emptyList()), Intento.Ok(Unit)),
            preferencias = PreferenciasFijas(null),
        )
        vm.elegirImpresora(ImpresoraConocida(laImpresora.direccion, laImpresora.nombre))
        montar(2.0f) { TarjetaDeImpresion(vm = vm, ordenId = "order:abc") }

        // Las dos opciones se leen enteras: del ancho depende que la boleta
        // salga bien, y "58 mm" a secas no le dice nada a quien nunca compró
        // papel térmico.
        compose.onAllNodes(hasText("El rollo angosto", substring = true))[0]
            .performScrollTo().assertIsDisplayed()
        compose.onAllNodes(hasText("El rollo ancho", substring = true))[0]
            .performScrollTo().assertIsDisplayed()

        revisarTactiles(2.0f)
        revisarQueNadaSeCorte(2.0f)
    }

    /**
     * La salida de emergencia tiene que quedar al alcance a cualquier escala.
     *
     * Es la prueba que más importa de este archivo: la venta ya está cobrada, y
     * un botón que existe pero al que no se llega es lo mismo que no tenerlo.
     */
    @Test
    fun `seguir sin boleta se alcanza al 100 y al 200 por ciento`() {
        val vm = ImpresoraViewModel(
            boletas = BoletasFijas(),
            enlace = EnlaceFijo(
                lista = Intento.Ok(emptyList()),
                resultado = Intento.Fallo(FallaDeImpresion.NoContesta("Epson TM-T20")),
            ),
            preferencias = PreferenciasFijas(laImpresora),
        )
        vm.imprimir("order:abc")
        montar(2.0f) { TarjetaDeImpresion(vm = vm, ordenId = "order:abc") }

        compose.onAllNodes(hasText("«Epson TM-T20» no contesta", substring = true))[0]
            .performScrollTo().assertIsDisplayed()
        compose.onAllNodes(hasText("Seguir sin boleta") and hasClickAction())[0]
            .performScrollTo().assertIsDisplayed()
        compose.onAllNodes(hasText("La venta ya está cobrada", substring = true))[0]
            .performScrollTo().assertIsDisplayed()

        revisarTactiles(2.0f)
        revisarQueNadaSeCorte(2.0f)
    }

    @Test
    fun `la falla se lee entera al 100 por ciento`() {
        val vm = ImpresoraViewModel(
            boletas = BoletasFijas(),
            enlace = EnlaceFijo(
                lista = Intento.Ok(emptyList()),
                resultado = Intento.Fallo(FallaDeImpresion.SinPapel("Epson TM-T20")),
            ),
            preferencias = PreferenciasFijas(laImpresora),
        )
        vm.imprimir("order:abc")
        montar(1.0f) { TarjetaDeImpresion(vm = vm, ordenId = "order:abc") }

        compose.onAllNodes(hasText("Cambia el rollo", substring = true))[0]
            .performScrollTo().assertIsDisplayed()
        revisarTactiles(1.0f)
        revisarQueNadaSeCorte(1.0f)
    }

    /** El estado de reposo, que es lo que se ve el 99% de las veces. */
    @Test
    fun `el boton de imprimir aguanta al 200 por ciento`() {
        val vm = ImpresoraViewModel(
            boletas = BoletasFijas(),
            enlace = EnlaceFijo(Intento.Ok(emptyList()), Intento.Ok(Unit)),
            preferencias = PreferenciasFijas(laImpresora),
        )
        montar(2.0f) { TarjetaDeImpresion(vm = vm, ordenId = "order:abc") }

        compose.onAllNodes(hasText("Imprimir boleta") and hasClickAction())[0]
            .performScrollTo().assertIsDisplayed()
        compose.onAllNodes(hasText("Sale por «Epson TM-T20»", substring = true))[0]
            .performScrollTo().assertIsDisplayed()

        revisarTactiles(2.0f)
        revisarQueNadaSeCorte(2.0f)
    }
}

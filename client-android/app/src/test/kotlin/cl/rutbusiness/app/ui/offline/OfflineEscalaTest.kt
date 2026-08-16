package cl.rutbusiness.app.ui.offline

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasScrollToNodeAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.height
import androidx.compose.ui.unit.width
import cl.rutbusiness.core.offline.VentaEnCola
import cl.rutbusiness.core.pos.LineaDeVenta
import cl.rutbusiness.core.pos.SolicitudDeVenta
import cl.rutbusiness.ui.theme.RbDefaultDimens
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * La franja de "sin conexión" y la lista de la cola, al 100% y al 200%.
 *
 * Son las dos superficies nuevas que la dueña mira **cuando algo anda mal**, y
 * ése es justo el momento en que no se le puede cortar una palabra ni fallarle
 * un toque. La franja además tiene una regla propia que se mide acá: cuando se
 * puede tocar, mide como cualquier cosa tocable de esta app.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class OfflineEscalaTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil = RbDefaultDimens.touchTarget

    private val ahora = 1_000_000_000L

    private fun venta(
        clave: String,
        haceMinutos: Long,
        rechazada: Boolean = false,
        motivo: String? = null,
    ) = VentaEnCola(
        clave = clave,
        solicitud = SolicitudDeVenta(
            items = listOf(
                LineaDeVenta("product:arroz", "Arroz Grado 1 kg", 2, "1490"),
                LineaDeVenta("product:aceite", "Aceite maravilla 900 ml", 1, "2790"),
            ),
            paymentMethod = "pos_cash",
        ),
        cobradaEn = ahora - haceMinutos * 60_000L,
        lineas = 2,
        rechazada = rechazada,
        motivo = motivo,
    )

    // --- la franja -----------------------------------------------------------

    @Test
    fun `sin conexion y sin cola la franja dice que se ve lo guardado`() {
        mostrarFranja(escala = 1.0f, conectado = false, cola = emptyList())

        compose.onNodeWithText("Sin conexión").assertIsDisplayed()
        compose.onNodeWithText("Ves lo último que se cargó.")
            .assertIsDisplayed()
        revisarQueNadaSeCorte(1.0f)
    }

    @Test
    fun `con conexion y sin cola la franja no esta`() {
        mostrarFranja(escala = 1.0f, conectado = true, cola = emptyList())

        // La franja que no molesta es la que no está: con todo en orden, la
        // pantalla de la dueña no pierde ni un renglón.
        assertEquals(
            0,
            compose.onAllNodes(hasText("Sin conexión")).fetchSemanticsNodes().size,
        )
    }

    @Test
    fun `una venta esperando se dice en singular`() {
        mostrarFranja(escala = 1.0f, conectado = false, cola = listOf(venta("a", 3)))
        compose.onNodeWithText("Sin conexión · 1 venta esperando").assertIsDisplayed()
    }

    @Test
    fun `al 200 por ciento la franja con cola se lee y se puede tocar`() {
        mostrarFranja(
            escala = 2.0f,
            conectado = false,
            cola = listOf(venta("a", 3), venta("b", 8)),
        )

        compose.onNodeWithText("Sin conexión · 2 ventas esperando").assertIsDisplayed()
        revisarTactiles(2.0f)
        revisarQueNadaSeCorte(2.0f)
    }

    @Test
    fun `tocar la franja abre la cola`() {
        var abierta = false
        compose.setContent {
            ConEscala(1.0f) {
                FranjaDeConexion(
                    conectado = false,
                    cola = listOf(venta("a", 3)),
                    onVerCola = { abierta = true },
                )
            }
        }
        compose.waitForIdle()

        compose.onNodeWithText("Sin conexión · 1 venta esperando").performClick()
        assertTrue("tocar la franja tiene que llevar a la lista", abierta)
    }

    // --- la lista de la cola --------------------------------------------------

    @Test
    fun `al 100 por ciento la cola se lee entera`() {
        mostrarCola(escala = 1.0f)
        revisarCola(1.0f)
    }

    /** El caso que define "listo" según el piso de hardware. */
    @Test
    fun `al 200 por ciento la cola se lee entera`() {
        mostrarCola(escala = 2.0f)
        revisarCola(2.0f)
    }

    private fun revisarCola(escala: Float) {
        enLaCola("Venta de hace 3 minutos").assertIsDisplayed()
        // El detalle se repite en las dos notas. Al 100% las dos están
        // compuestas; al 200% con aire LazyColumn puede traer una sola. Se
        // scrollea si hace falta y se pide al menos una, no un conteo fijo.
        val detalle =
            "2 productos · 3 unidades · el total lo confirma el sistema al recibirla"
        if (compose.onAllNodes(hasText(detalle)).fetchSemanticsNodes().isEmpty()) {
            compose.onNode(hasScrollToNodeAction()).performScrollToNode(hasText(detalle))
        }
        assertTrue(
            "el detalle de productos se tiene que leer",
            compose.onAllNodes(hasText(detalle)).fetchSemanticsNodes().isNotEmpty(),
        )
        revisarTactiles(escala)
        revisarQueNadaSeCorte(escala)

        enLaCola("Venta de hace 12 minutos").assertIsDisplayed()
        enLaCola("No queda stock de Arroz Grado 1 kg.").assertIsDisplayed()
        enLaCola("No se anotó").assertIsDisplayed()
        revisarTactiles(escala)
        revisarQueNadaSeCorte(escala)
    }

    /**
     * Ni un monto en toda la pantalla.
     *
     * Es la regla de plata del encargo puesta a prueba: la cola muestra ventas
     * que el sistema todavía no confirmó, así que cualquier total que
     * apareciera acá lo habría calculado el teléfono. Se buscan las marcas de
     * plata que usa esta app -el símbolo y el separador de miles chileno- y no
     * tiene que haber ninguna.
     */
    @Test
    fun `la cola no muestra ni un peso`() {
        mostrarCola(escala = 1.0f)

        val conPlata = textosVisibles().filter { texto ->
            texto.contains("$") || Regex("""\d\.\d{3}""").containsMatchIn(texto)
        }
        assertTrue(
            "la cola no puede mostrar montos: el total lo pone el server al recibir la " +
                "venta.\n" + conPlata.joinToString("\n"),
            conPlata.isEmpty(),
        )
    }

    @Test
    fun `sin senal el boton de intentar ahora no promete nada`() {
        compose.setContent {
            ConEscala(1.0f) {
                PantallaDeCola(
                    cola = listOf(venta("a", 3)),
                    conectado = false,
                    ahora = ahora,
                    onIntentarAhora = {},
                    onDescartar = {},
                    onCerrar = {},
                )
            }
        }
        compose.waitForIdle()

        compose.onNodeWithText("Intentar ahora").assertIsNotEnabled()
    }

    // --- andamio --------------------------------------------------------------

    private fun mostrarFranja(escala: Float, conectado: Boolean, cola: List<VentaEnCola>) {
        compose.setContent {
            ConEscala(escala) {
                FranjaDeConexion(conectado = conectado, cola = cola, onVerCola = {})
            }
        }
        compose.waitForIdle()
    }

    private fun mostrarCola(escala: Float) {
        compose.setContent {
            ConEscala(escala) {
                PantallaDeCola(
                    cola = listOf(
                        venta("esperando", haceMinutos = 3),
                        venta(
                            clave = "rechazada",
                            haceMinutos = 12,
                            rechazada = true,
                            motivo = "No queda stock de Arroz Grado 1 kg.",
                        ),
                    ),
                    conectado = true,
                    ahora = ahora,
                    onIntentarAhora = {},
                    onDescartar = {},
                    onCerrar = {},
                )
            }
        }
        compose.waitForIdle()
    }

    private fun enLaCola(texto: String) = run {
        compose.onNode(hasScrollToNodeAction()).performScrollToNode(hasText(texto))
        compose.onNodeWithText(texto)
    }

    private fun textosVisibles(): List<String> =
        compose.onAllNodes(hasText("", substring = true)).fetchSemanticsNodes()
            .mapNotNull { nodo ->
                nodo.config.getOrNull(SemanticsProperties.Text)?.joinToString(" ") { it.text }
            }

    @Composable
    private fun ConEscala(escala: Float, contenido: @Composable () -> Unit) {
        val base = LocalDensity.current
        CompositionLocalProvider(
            LocalDensity provides Density(base.density, fontScale = escala),
        ) {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Box(Modifier.fillMaxSize()) { contenido() }
            }
        }
    }

    private fun revisarTactiles(escala: Float) {
        val chicos = mutableListOf<String>()
        val nodos = compose.onAllNodes(hasClickAction()).fetchSemanticsNodes()
        nodos.indices.forEach { i ->
            val nodo = compose.onAllNodes(hasClickAction())[i]
            val caja = nodo.getUnclippedBoundsInRoot()
            if (caja.height < objetivoTactil) {
                val texto = nodo.fetchSemanticsNode()
                    .config.getOrNull(SemanticsProperties.Text)
                    ?.joinToString(" ") { it.text } ?: "sin texto"
                chicos += "«$texto» mide ${caja.width} x ${caja.height}"
            }
        }
        assertTrue(
            "a ${escala}x estos toques quedaron bajo $objetivoTactil:\n" +
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
}

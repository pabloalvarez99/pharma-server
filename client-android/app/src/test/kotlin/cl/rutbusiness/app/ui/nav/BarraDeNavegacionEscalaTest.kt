package cl.rutbusiness.app.ui.nav

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
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
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.height
import androidx.compose.ui.unit.width
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
 * La barra de abajo al 100% y al 200% de escala de letra.
 *
 * Es el peor caso de layout que tiene la app: tres etiquetas completas repartidas
 * en 360dp de ancho, o sea 120dp por pestaña, con la letra al doble. Si al 200%
 * una etiqueta se corta, esa pestaña se queda sin nombre justo para la persona
 * que subió la letra porque ve poco — que es el usuario objetivo del producto.
 *
 * `NATIVE` no es opcional: con los gráficos por defecto de Robolectric el motor
 * de fuentes mide ~0,5dp por carácter y cualquier aserción de ancho pasaría sin
 * significar nada. SDK 34 porque ahí vive la curva no lineal de escala de letra
 * de Android 14, que es la que el usuario recibe de verdad.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class BarraDeNavegacionEscalaTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil = RbDefaultDimens.touchTarget

    private fun mostrar(escala: Float, actual: Destino = Destino.Agente, onElegir: (Destino) -> Unit = {}) {
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(base.density, fontScale = escala),
            ) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    // Anclada abajo, como en la app: así el alto que mide la
                    // prueba es el que la barra ocupa de verdad.
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
                        BarraDeNavegacion(actual = actual, onElegir = onElegir)
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    private fun revisarTactiles(escala: Float) {
        val nodos = compose.onAllNodes(hasClickAction()).fetchSemanticsNodes()
        assertEquals("la barra tiene que dibujar tres pestañas", 3, nodos.size)

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
            "a ${escala}x estas pestañas quedaron bajo $objetivoTactil:\n" +
                chicos.joinToString("\n"),
            chicos.isEmpty(),
        )
    }

    /** Ninguna etiqueta se corta: en una barra no hay a dónde scrollear. */
    private fun revisarQueNadaSeCorte(escala: Float) {
        val cortados = mutableListOf<String>()
        Destino.entries.forEach { destino ->
            val nodo = compose.onNodeWithText(destino.etiqueta)
            val visible = nodo.getBoundsInRoot()
            val completo = nodo.getUnclippedBoundsInRoot()
            if (completo.width - visible.width > 1.dp || completo.height - visible.height > 1.dp) {
                cortados += "«${destino.etiqueta}»: mide ${completo.width} x ${completo.height}, " +
                    "se ve ${visible.width} x ${visible.height}"
            }
        }
        assertTrue("a ${escala}x se corta:\n${cortados.joinToString("\n")}", cortados.isEmpty())
    }

    @Test
    fun `al 100 por ciento se ven las tres con su nombre`() {
        mostrar(escala = 1.0f)
        Destino.entries.forEach { compose.onNodeWithText(it.etiqueta).assertIsDisplayed() }
        revisarTactiles(1.0f)
        revisarQueNadaSeCorte(1.0f)
    }

    /** El caso que define "listo" según el piso de hardware. */
    @Test
    fun `al 200 por ciento se ven las tres con su nombre`() {
        mostrar(escala = 2.0f)
        Destino.entries.forEach { compose.onNodeWithText(it.etiqueta).assertIsDisplayed() }
        revisarTactiles(2.0f)
        revisarQueNadaSeCorte(2.0f)
    }

    /**
     * El ícono nunca va solo.
     *
     * Un pictograma sin palabra es una adivinanza, y ésta es la regla que más
     * fácil se pierde en un refactor "para que entre mejor". Que se caiga la
     * prueba antes que la pantalla.
     *
     * Va una prueba por escala y no un bucle: `setContent` se puede llamar una
     * sola vez por regla de Compose.
     */
    @Test
    fun `cada pestana tiene su etiqueta al 100 por ciento`() {
        mostrar(escala = 1.0f)
        revisarQueTodasTenganEtiqueta(1.0f)
    }

    @Test
    fun `cada pestana tiene su etiqueta al 200 por ciento`() {
        mostrar(escala = 2.0f)
        revisarQueTodasTenganEtiqueta(2.0f)
    }

    private fun revisarQueTodasTenganEtiqueta(escala: Float) {
        Destino.entries.forEach { destino ->
            assertTrue(
                "a ${escala}x falta la etiqueta «${destino.etiqueta}»",
                compose.onAllNodes(hasText(destino.etiqueta)).fetchSemanticsNodes().isNotEmpty(),
            )
        }
    }

    /** La barra tampoco puede comerse la pantalla entera al 200%. */
    @Test
    fun `al 200 por ciento la barra deja lugar al contenido`() {
        mostrar(escala = 2.0f)

        val alturas = compose.onAllNodes(hasClickAction()).fetchSemanticsNodes()
            .indices
            .map { compose.onAllNodes(hasClickAction())[it].getUnclippedBoundsInRoot().height }
        val alto = alturas.max()

        // 640dp de alto útil: si la barra pasa de un tercio, la pantalla de
        // arriba deja de servir para lo suyo.
        assertTrue("al 200% la barra mide $alto, más de un tercio de 640dp", alto < 213.dp)
    }

    @Test
    fun `tocar una pestana avisa cual`() {
        var elegido: Destino? = null
        mostrar(escala = 2.0f, onElegir = { elegido = it })

        compose.onNodeWithText(Destino.Cobrar.etiqueta).performClick()
        compose.waitForIdle()

        assertEquals(Destino.Cobrar, elegido)
    }
}

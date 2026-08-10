package cl.rutbusiness.app.ui.resumen

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.width
import cl.rutbusiness.core.money.Moneda
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * La cifra del día al 100% y al 200% de escala de letra.
 *
 * Un monto es una palabra sin espacios, así que cuando no entra Compose lo parte
 * por la mitad: "$1.234.567" se lee "$1.234." arriba y "567" abajo. La dueña
 * mira eso y no sabe si vendió un millón o mil doscientos. Por eso la cifra se
 * mide con montos de siete dígitos, que es el peor caso real de un negocio
 * chico en pesos, y en las dos monedas que el producto tiene que soportar hoy:
 * CLP sin decimales y una de dos decimales.
 *
 * `NATIVE` no es opcional: con los gráficos por defecto de Robolectric el motor
 * de texto mide ~0,5dp por carácter y toda aserción de ancho pasaría sin
 * significar nada. La pantalla es 360x640dp, el panel del aparato de referencia.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class ResumenEscalaTest {

    @get:Rule
    val compose = createComposeRule()

    private fun mostrar(
        escala: Float,
        moneda: Moneda = Moneda.POR_DEFECTO,
        vendidoHoy: String = "1234567",
        boletas: Long = 42L,
        comparacion: Comparacion = Comparacion.Mejor,
        vendidoAyer: String? = "987654",
    ) {
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(base.density, fontScale = escala),
            ) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    // La tarjeta vive en una lista que scrollea; el contenedor
                    // de la prueba imita eso para que "alcanzable" signifique lo
                    // mismo que en la pantalla real.
                    Box(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                        TarjetaDelDia(
                            moneda = moneda,
                            vendidoHoy = vendidoHoy,
                            boletas = boletas,
                            comparacion = comparacion,
                            vendidoAyer = vendidoAyer,
                        )
                    }
                }
            }
        }
        compose.waitForIdle()
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

    @Test
    fun `al 100 por ciento se lee la tarjeta entera`() {
        mostrar(escala = 1.0f)

        listOf(
            "Vendiste hoy",
            "$1.234.567",
            "42 boletas.",
            "Mejor que ayer",
            "Ayer, día completo: $987.654.",
        ).forEach { texto ->
            compose.onNodeWithText(texto, substring = true).performScrollTo().assertIsDisplayed()
        }
        revisarQueNadaSeCorte(1.0f)
    }

    /** El caso que define "listo" según el piso de hardware. */
    @Test
    fun `al 200 por ciento se lee la tarjeta entera`() {
        mostrar(escala = 2.0f)

        listOf(
            "Vendiste hoy",
            "$1.234.567",
            "42 boletas.",
            "Mejor que ayer",
            "Ayer, día completo: $987.654.",
        ).forEach { texto ->
            compose.onNodeWithText(texto, substring = true).performScrollTo().assertIsDisplayed()
        }
        revisarQueNadaSeCorte(2.0f)
    }

    /**
     * La cifra no puede quedar partida en dos líneas.
     *
     * Es lo que motiva que el multiplicador de tamaño se suelte con la letra
     * grande. Se mide contando líneas, no ancho: un monto que envuelve ocupa dos
     * renglones aunque no se "corte" en el sentido de quedar tapado.
     */
    @Test
    fun `la cifra del dia entra en una sola linea al 100 por ciento`() {
        mostrar(escala = 1.0f)
        revisarQueLaCifraNoSeParta(1.0f)
    }

    @Test
    fun `la cifra del dia entra en una sola linea al 200 por ciento`() {
        mostrar(escala = 2.0f)
        revisarQueLaCifraNoSeParta(2.0f)
    }

    private fun revisarQueLaCifraNoSeParta(escala: Float) {
        val lineas = compose.onNodeWithText("$1.234.567")
            .fetchSemanticsNode()
            .textLayoutLineCount()
        assertTrue(
            "a ${escala}x la cifra del día quedó en $lineas líneas: un monto partido " +
                "por la mitad se lee como otro monto",
            lineas == 1,
        )
    }

    /**
     * Una moneda de dos decimales no rompe nada.
     *
     * `USD 12345.67` es más largo que su equivalente en pesos y es el caso que
     * un tenant fuera de Chile recibe sin que nadie toque esta pantalla.
     */
    @Test
    fun `una moneda con decimales tambien entra al 200 por ciento`() {
        mostrar(
            escala = 2.0f,
            moneda = Moneda.de("USD"),
            vendidoHoy = "12345.67",
            vendidoAyer = "9876.54",
        )

        compose.onNodeWithText("US$12.345,67").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("US$9.876,54", substring = true).performScrollTo().assertIsDisplayed()
        revisarQueNadaSeCorte(2.0f)
    }

    /** Sin dato de ayer se dice, no se inventa una comparación. */
    @Test
    fun `sin lo de ayer la tarjeta lo dice`() {
        mostrar(escala = 1.0f, comparacion = Comparacion.SinDatoDeAyer, vendidoAyer = null)

        compose.onNodeWithText("Sin comparación").assertIsDisplayed()
        compose.onNodeWithText("No pudimos traer lo de ayer", substring = true).assertIsDisplayed()
    }

    /** Un día sin ventas no muestra "0 boletas" sino la frase que corresponde. */
    @Test
    fun `un dia sin ventas se dice con palabras`() {
        mostrar(escala = 1.0f, vendidoHoy = "0", boletas = 0L, comparacion = Comparacion.Peor)

        compose.onNodeWithText("Todavía no hay ninguna venta.").assertIsDisplayed()
        compose.onNodeWithText("$0").assertIsDisplayed()
    }
}

/**
 * Cuántas líneas ocupó el texto de un nodo.
 *
 * `SemanticsNode` no expone el `TextLayoutResult` directamente: hay que pedirle
 * el resultado a la acción `GetTextLayoutResult` que Compose instala en todo
 * `Text`.
 */
private fun androidx.compose.ui.semantics.SemanticsNode.textLayoutLineCount(): Int {
    val resultados = mutableListOf<androidx.compose.ui.text.TextLayoutResult>()
    config.getOrNull(androidx.compose.ui.semantics.SemanticsActions.GetTextLayoutResult)
        ?.action
        ?.invoke(resultados)
    return resultados.firstOrNull()?.lineCount ?: 0
}

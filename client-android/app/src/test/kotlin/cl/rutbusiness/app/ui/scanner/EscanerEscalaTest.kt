package cl.rutbusiness.app.ui.scanner

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
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
 * El escáner con la letra al 100% y al 200%.
 *
 * Lo que se mide es lo que no tiene a dónde escapar: los botones del panel de
 * abajo. En el resto de la app un texto que crece empuja scroll; acá empuja
 * contra el visor de la cámara, y si la fila de botones se sale de la pantalla
 * la cajera se queda sin la salida "escribir a mano" — que es justo la que
 * necesita cuando el escáner no le sirve.
 *
 * `NATIVE` y sdk 34 por la misma razón que en el resto de las pruebas de
 * escala: con los gráficos por defecto de Robolectric medir anchos no
 * significa nada, y la curva no lineal de escala de letra es la de Android 14.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class EscanerEscalaTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil = RbDefaultDimens.touchTarget

    private fun mostrar(escala: Float, contenido: @Composable () -> Unit) {
        compose.setContent {
            val base = LocalDensity.current
            CompositionLocalProvider(
                LocalDensity provides Density(base.density, fontScale = escala),
            ) {
                RbTheme(darkTheme = true, reducedMotion = true) {
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
                        contenido()
                    }
                }
            }
        }
        compose.waitForIdle()
    }

    private fun revisarTactiles(escala: Float, cuantos: Int) {
        val nodos = compose.onAllNodes(hasClickAction()).fetchSemanticsNodes()
        assertEquals("faltan botones en el panel", cuantos, nodos.size)

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
            "a ${escala}x estos botones quedaron bajo $objetivoTactil:\n" +
                chicos.joinToString("\n"),
            chicos.isEmpty(),
        )
    }

    private fun revisarQueNadaSeCorte(escala: Float, vararg textos: String) {
        val cortados = mutableListOf<String>()
        textos.forEach { texto ->
            val nodo = compose.onNodeWithText(texto, substring = true)
            val visible = nodo.getBoundsInRoot()
            val completo = nodo.getUnclippedBoundsInRoot()
            if (completo.width - visible.width > 1.dp || completo.height - visible.height > 1.dp) {
                cortados += "«$texto»: mide ${completo.width} x ${completo.height}, " +
                    "se ve ${visible.width} x ${visible.height}"
            }
        }
        assertTrue("a ${escala}x se corta:\n${cortados.joinToString("\n")}", cortados.isEmpty())
    }

    @Composable
    private fun PanelDePermiso(estado: EstadoDelPermiso, onEscribir: () -> Unit = {}) {
        PanelInferior {
            AccionesDePermiso(
                estado = estado,
                onPermitir = {},
                onAjustes = {},
                onEscribirAMano = onEscribir,
            )
        }
    }

    @Test
    fun `los botones del permiso son tocables al 100 por ciento`() {
        mostrar(1.0f) { PanelDePermiso(EstadoDelPermiso.SinPedir) }
        compose.onNodeWithText("Permitir cámara").assertIsDisplayed()
        compose.onNodeWithText("Escribir a mano").assertIsDisplayed()
        revisarTactiles(1.0f, cuantos = 2)
        revisarQueNadaSeCorte(1.0f, "Permitir cámara", "Escribir a mano")
    }

    /** El caso que define "listo" según el piso de hardware. */
    @Test
    fun `los botones del permiso son tocables al 200 por ciento`() {
        mostrar(2.0f) { PanelDePermiso(EstadoDelPermiso.SinPedir) }
        compose.onNodeWithText("Permitir cámara").assertIsDisplayed()
        compose.onNodeWithText("Escribir a mano").assertIsDisplayed()
        revisarTactiles(2.0f, cuantos = 2)
        revisarQueNadaSeCorte(2.0f, "Permitir cámara", "Escribir a mano")
    }

    /**
     * Con el permiso negado para siempre, la salida por Ajustes y la de escribir
     * a mano tienen que estar las dos. Perder cualquiera de las dos deja a la
     * cajera encerrada en una pantalla que sólo explica el problema.
     */
    @Test
    fun `bloqueada para siempre igual ofrece las dos salidas al 200 por ciento`() {
        mostrar(2.0f) { PanelDePermiso(EstadoDelPermiso.NegadoParaSiempre) }
        compose.onNodeWithText("Abrir ajustes").assertIsDisplayed()
        compose.onNodeWithText("Escribir a mano").assertIsDisplayed()
        revisarTactiles(2.0f, cuantos = 2)
    }

    @Test
    fun `escribir a mano avisa`() {
        var pedido = false
        mostrar(2.0f) { PanelDePermiso(EstadoDelPermiso.Negado, onEscribir = { pedido = true }) }

        compose.onNodeWithText("Escribir a mano").performClick()
        compose.waitForIdle()

        assertTrue("el botón de escribir a mano no hizo nada", pedido)
    }

    /**
     * El texto que se muestra **antes** del diálogo del sistema tiene que decir
     * para qué es la cámara y que la imagen no se va del teléfono. Es una regla
     * de producto, no una decoración: que se caiga la prueba antes que la
     * promesa.
     */
    @Test
    fun `la explicacion dice para que es y que la imagen no sale del telefono`() {
        mostrar(1.0f) {
            ExplicacionDelPermiso(EstadoDelPermiso.SinPedir, modifier = Modifier.fillMaxSize())
        }
        compose.onNodeWithText("código de barras", substring = true).assertIsDisplayed()
        compose.onNodeWithText("no sale de acá", substring = true).assertIsDisplayed()
    }

    /** El cartel de estado tampoco puede cortar el nombre del producto. */
    @Test
    fun `el cartel de lectura entra completo al 200 por ciento`() {
        mostrar(2.0f) {
            PanelInferior {
                Cartel(
                    fondo = RbTheme.colors.brandContainer,
                    titulo = "Listo: Detergente Concentrado 3 litros",
                    detalle = "2 unidades en el carrito",
                )
            }
        }
        revisarQueNadaSeCorte(2.0f, "Detergente Concentrado 3 litros", "2 unidades en el carrito")
    }
}

package cl.rutbusiness.app.ui.cobrar

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.height
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
 * El cobro rápido con el teclado numérico arriba.
 *
 * Es la pantalla más corta de la app y la que más se va a usar en un puesto, y
 * por eso es la que menos margen tiene: teclado abierto, letra al 200%, y el
 * botón de cobrar tiene que estar donde el pulgar ya está. Si hay que scrollear
 * para cobrar $2.000, el cliente ya se fue.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class MontoSueltoEscalaTest {

    @get:Rule
    val compose = createComposeRule()

    private val objetivoTactil: Dp = RbDefaultDimens.touchTarget

    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `con el teclado arriba al 200 por ciento el campo y el boton se ven`() {
        montar(escala = 2.0f)

        compose.onNodeWithContentDescription("¿Cuánto le cobras?").assertIsDisplayed()

        val cobrar = botonDeCobrar()
        cobrar.assertIsDisplayed()
        val caja = cobrar.getBoundsInRoot()
        println("MEDIDA cobrar-monto=${caja.height} escala=2.0")
        assertTrue(
            "a 2x el botón mide ${caja.height} de alto, bajo el objetivo táctil " +
                "de $objetivoTactil",
            caja.height >= objetivoTactil,
        )
    }

    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `el monto escrito llega al que cobra`() {
        var cobrado = 0
        montar(escala = 2.0f, onConfirmar = { cobrado += 1 })

        compose.onNodeWithContentDescription("¿Cuánto le cobras?").performTextInput("2000")
        botonDeCobrar().performClick()

        assertEquals(1, cobrado)
    }

    /**
     * El error se ve **sin scrollear**: va pegado al campo, que es donde está la
     * vista puesta, y no al final de la pantalla.
     */
    @Test
    @Config(qualifiers = "w360dp-h320dp-xhdpi")
    fun `el error de monto se ve con el teclado arriba`() {
        montar(escala = 2.0f, error = "El monto tiene que ser mayor que cero.")
        compose.onNodeWithText("El monto tiene que ser mayor que cero.").assertIsDisplayed()
    }

    /**
     * Nada de cargar productos acá.
     *
     * El punto entero de esta pantalla es que no hace falta tener nada cargado;
     * pedir un nombre o una unidad la convertiría en el formulario del que se
     * está escapando.
     */
    @Test
    fun `no pide nada mas que el monto`() {
        montar(escala = 1.0f)

        compose.onNodeWithContentDescription("¿Cómo se llama?").assertDoesNotExist()
        compose.onNodeWithContentDescription("Se vende por").assertDoesNotExist()
    }

    private fun botonDeCobrar(): SemanticsNodeInteraction =
        compose.onNode(hasText("Cobrar este monto") and hasClickAction())

    private fun montar(
        escala: Float,
        error: String? = null,
        onConfirmar: () -> Unit = {},
    ) {
        compose.setContent {
            var monto by remember { mutableStateOf("") }
            ConEscala(escala) {
                Column(modifier = Modifier.fillMaxSize()) {
                    MontoSueltoContenido(
                        modifier = Modifier.fillMaxSize(),
                        monto = monto,
                        onMonto = { monto = it },
                        simbolo = "$",
                        error = error,
                        preparando = false,
                        onConfirmar = onConfirmar,
                        onCancelar = {},
                    )
                }
            }
        }
        compose.waitForIdle()
    }

    @Composable
    private fun ConEscala(escala: Float, contenido: @Composable () -> Unit) {
        val base = LocalDensity.current
        CompositionLocalProvider(
            LocalDensity provides Density(base.density, fontScale = escala),
        ) {
            RbTheme(darkTheme = true, reducedMotion = true) { contenido() }
        }
    }
}

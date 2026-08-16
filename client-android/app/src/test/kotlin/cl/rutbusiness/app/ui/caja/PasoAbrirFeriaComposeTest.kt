package cl.rutbusiness.app.ui.caja

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.unit.height
import cl.rutbusiness.core.session.AlmacenamientoPlataforma
import cl.rutbusiness.core.session.SessionRepository
import cl.rutbusiness.ui.theme.RbDefaultDimens
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Feria + paso Abrir: CTA «Empezar el día» y sin «cajón» como instrucción.
 *
 * Monta [PasoAbrirCaja] real con [CajaViewModel.modoFeria] — no hace falta pack
 * en CompositionLocal porque el VM ya baja el flag.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class PasoAbrirFeriaComposeTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun `feria muestra Empezar el dia y no cajon como instruccion principal`() {
        val app = RuntimeEnvironment.getApplication()
        val vm = CajaViewModel(SessionRepository(AlmacenamientoPlataforma(app)))
        vm.modoFeria(true)

        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Box(Modifier.fillMaxSize()) {
                    PasoAbrirCaja(vm)
                }
            }
        }
        compose.waitForIdle()

        compose.onNodeWithText("Abrir el puesto").assertIsDisplayed()
        compose.onNodeWithText("Empezar el día").performScrollTo().assertIsDisplayed()

        val cta = compose.onNodeWithText("Empezar el día").getUnclippedBoundsInRoot()
        assertTrue(
            "CTA táctil ≥56dp, midió ${cta.height}",
            cta.height >= RbDefaultDimens.touchTarget,
        )

        // Instrucción principal (card + ayuda): no «cajón».
        val nodes = compose.onAllNodes(
            androidx.compose.ui.test.hasText("cajón", substring = true, ignoreCase = true),
        ).fetchSemanticsNodes()
        assertTrue(
            "no debería haber copy de cajón en el formulario feria visible",
            nodes.isEmpty(),
        )
        assertFalse(
            copyAbrirCaja(true).ayuda.lowercase().contains("cajón"),
        )
    }
}

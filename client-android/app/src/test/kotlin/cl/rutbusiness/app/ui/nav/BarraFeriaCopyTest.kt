package cl.rutbusiness.app.ui.nav

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Copy feria en la barra (ADR-0022): agent_home reescribe etiquetas.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class BarraFeriaCopyTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun `con agent_home se lee Vender y Hoy`() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
                    BarraDeNavegacion(
                        actual = Destino.Agente,
                        onElegir = {},
                        agentHome = true,
                    )
                }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText("Agente").assertIsDisplayed()
        compose.onNodeWithText("Vender").assertIsDisplayed()
        compose.onNodeWithText("Hoy").assertIsDisplayed()
    }

    @Test
    fun `sin agent_home se lee Cobrar y Tu dia`() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.BottomCenter) {
                    BarraDeNavegacion(
                        actual = Destino.Agente,
                        onElegir = {},
                        agentHome = false,
                    )
                }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText("Cobrar").assertIsDisplayed()
        compose.onNodeWithText("Tu día").assertIsDisplayed()
    }
}

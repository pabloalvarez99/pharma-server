package cl.rutbusiness.app.ui.entrada

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import cl.rutbusiness.core.session.IdentidadGoogleNoCableada
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Pantalla de entrada + copy Google feria (ADR-0022).
 *
 * Stub: el botón se ve y se puede tocar; no hay OAuth ni secretos.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class GoogleEntradaCopyTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun `feria muestra titulo de puesto y boton Google`() {
        var toques = 0
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Box(Modifier.fillMaxSize()) {
                    FormularioDeEntrada(
                        url = "192.168.1.10:8080",
                        onUrl = {},
                        ayudaDeDireccion = "ok",
                        errorDeDireccion = null,
                        negocio = "puesto-rosa",
                        onNegocio = {},
                        email = "rosa@feria.cl",
                        onEmail = {},
                        password = "x",
                        onPassword = {},
                        conexionConfirmada = false,
                        falla = null,
                        impedimento = null,
                        probando = false,
                        enviando = false,
                        puedeProbar = true,
                        puedeEntrar = true,
                        onProbar = {},
                        onEntrar = {},
                        onVerExplicacion = {},
                        rubroEsFeria = true,
                        copyGoogle = IdentidadGoogleNoCableada.copyPromocion(true),
                        etiquetaGoogle = IdentidadGoogleNoCableada.etiquetaBoton(),
                        googleDisponible = false,
                        onGoogle = { toques += 1 },
                    )
                }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText("Entrar a tu puesto").assertIsDisplayed()
        compose.onNodeWithText("Con tu cuenta de Google").assertIsDisplayed()
        compose.onNodeWithText("Continuar con Google")
            .performScrollTo()
            .assertIsDisplayed()
            .performClick()
        compose.waitForIdle()
        assertTrue("el boton Google no avisó el toque", toques == 1)
    }

    @Test
    fun `formal muestra Entrar a tu negocio`() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Box(Modifier.fillMaxSize()) {
                    FormularioDeEntrada(
                        url = "",
                        onUrl = {},
                        ayudaDeDireccion = "",
                        errorDeDireccion = null,
                        negocio = "",
                        onNegocio = {},
                        email = "",
                        onEmail = {},
                        password = "",
                        onPassword = {},
                        conexionConfirmada = false,
                        falla = null,
                        impedimento = null,
                        probando = false,
                        enviando = false,
                        puedeProbar = false,
                        puedeEntrar = false,
                        onProbar = {},
                        onEntrar = {},
                        onVerExplicacion = {},
                        rubroEsFeria = false,
                    )
                }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText("Entrar a tu negocio").assertIsDisplayed()
        compose.onNodeWithText("Cuenta de Google").assertIsDisplayed()
    }
}

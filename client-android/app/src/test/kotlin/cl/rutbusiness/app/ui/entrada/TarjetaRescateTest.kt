package cl.rutbusiness.app.ui.entrada

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import cl.rutbusiness.core.backup.claveDeDemostracion
import cl.rutbusiness.core.backup.payloadQrRescate
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Tarjeta de rescate day-1: palabras + payload QR del tenant.
 */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w360dp-h640dp-xhdpi", sdk = [34])
class TarjetaRescateTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun `muestra payload qr del tenant y confirma cuaderno`() {
        val clave = claveDeDemostracion()
        val slug = "puesto-rosa"
        val payload = payloadQrRescate(slug, clave.bloques)!!
        var listo = false

        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Box(Modifier.fillMaxSize()) {
                    TarjetaRescate(
                        onListo = { listo = true },
                        clave = clave,
                        tenantSlug = slug,
                    )
                }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText("Tu tarjeta de rescate").assertIsDisplayed()
        // Payload en card QR + texto de una página (2 nodos).
        compose.onAllNodesWithText("rutbusiness-rescue:v1:puesto-rosa:", substring = true)
            .assertCountEquals(2)
        assertTrue(payload.startsWith("rutbusiness-rescue:v1:puesto-rosa:"))
        // PDF / impresión: botón en el scroll (no se toca en Robolectric).
        compose.onNodeWithText("Imprimir o guardar PDF")
            .performScrollTo()
            .assertIsDisplayed()
        // Botón anclado abajo (fuera del scroll): siempre a la vista.
        compose.onNodeWithText("Ya la anoté en el cuaderno")
            .assertIsDisplayed()
            .performClick()
        compose.waitForIdle()
        assertTrue(listo)
    }
}

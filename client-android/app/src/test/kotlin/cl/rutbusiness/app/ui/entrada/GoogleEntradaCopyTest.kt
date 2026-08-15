package cl.rutbusiness.app.ui.entrada

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import cl.rutbusiness.core.session.IdentidadGoogle
import cl.rutbusiness.core.session.IdentidadGoogleNoCableada
import cl.rutbusiness.core.session.ResultadoGoogle
import cl.rutbusiness.ui.theme.RbTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
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

    // --- una sola verdad sobre si Google existe en este build -----------------

    /**
     * Google cableado y andando. No abre picker: `disponible()` es lo único que
     * mira el copy, y `pedirCuenta()` acá nunca se llama.
     */
    private object GoogleCableado : IdentidadGoogle {
        override fun disponible(): Boolean = true

        override fun copyPromocion(rubroEsFeria: Boolean): String =
            "Entrá con tu cuenta de Google."

        override suspend fun pedirCuenta(): ResultadoGoogle =
            error("el copy del primer uso no puede pedir una cuenta")
    }

    private fun servicios(identidad: IdentidadGoogle) = ServiciosDeEntrada(
        red = object : RedDelTelefono {
            override fun hayRed(): Boolean = true
        },
        preferencias = object : PreferenciasDeEntrada {
            override fun yaEntroAlgunaVez(): Boolean = false
            override fun marcarQueEntro() = Unit
            override fun rubroElegido(): String? = null
            override fun guardarRubroElegido(rubro: String) = Unit
            override fun debeMostrarTarjetaRescate(): Boolean = false
            override fun marcarTarjetaRescateVista() = Unit
        },
        identidadGoogle = identidad,
    )

    /**
     * El texto del primer uso sale del **mismo** `disponible()` que el botón.
     *
     * Sin esto vuelven las dos verdades: un APK con client id muestra el botón
     * de Google andando y, dos pantallas antes, promete que llega "pronto". La
     * primera que la persona lee es la falsa.
     */
    @Test
    fun `con pideDireccion false no se ve la IP ni el computador`() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Box(Modifier.fillMaxSize()) {
                    FormularioDeEntrada(
                        url = "https://nube.test.invalid",
                        onUrl = {},
                        pideDireccion = false,
                        ayudaDeDireccion = "no se muestra",
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
                        puedeProbar = false,
                        puedeEntrar = true,
                        onProbar = {},
                        onEntrar = {},
                        onVerExplicacion = {},
                        rubroEsFeria = true,
                    )
                }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText("Entrar a tu puesto").assertIsDisplayed()
        assertEquals(
            0,
            compose.onAllNodesWithText("¿Dónde está el computador del negocio?")
                .fetchSemanticsNodes().size,
        )
        assertEquals(
            0,
            compose.onAllNodesWithText("192.168", substring = true)
                .fetchSemanticsNodes().size,
        )
        assertEquals(
            0,
            compose.onAllNodesWithContentDescription("Dirección del computador")
                .fetchSemanticsNodes().size,
        )
        assertEquals(
            0,
            compose.onAllNodesWithText("Probar la dirección")
                .fetchSemanticsNodes().size,
        )
        compose.onNodeWithContentDescription("Nombre corto del negocio").assertIsDisplayed()
    }

    @Test
    fun `con nube el primer uso no pide direccion de computador`() {
        val conNube = pasosDelPrimerUso(googleDisponible = false, nube = true)
        val texto = conNube.joinToString(" ") { paso ->
            listOf(paso.titulo, paso.encabezado, paso.parrafos.joinToString(" "), paso.lista.joinToString(" "), paso.remate.orEmpty())
                .joinToString(" ")
        }
        assertFalse("la feria no puede ver una IP", texto.contains("192.168"))
        assertFalse(texto.contains("computador del negocio"))
        assertFalse(texto.contains("quien instaló el sistema"))
    }

    @Test
    fun `con Google cableado el paso 3 no promete que llega pronto`() {
        val conGoogle = pasosDelPrimerUso(googleDisponible = true).last()
        val sinGoogle = pasosDelPrimerUso(googleDisponible = false).last()

        assertTrue(
            "con Google cableado el paso 3 sigue diciendo 'pronto'",
            conGoogle.lista.none { it.contains("Pronto", ignoreCase = true) },
        )
        assertTrue(
            "con Google cableado el paso 3 no nombra la cuenta de Google",
            conGoogle.lista.any { it.contains("cuenta de Google") },
        )
        // Y sin client id el texto es exactamente el de siempre: prometer
        // "pronto" es honesto cuando el botón todavía no está.
        assertTrue(
            "sin Google el paso 3 dejó de prometer",
            sinGoogle.lista.any { it.contains("Pronto también con tu cuenta de Google") },
        )
    }

    @Test
    fun `la pantalla del primer uso muestra el texto que corresponde al build`() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Box(Modifier.fillMaxSize()) {
                    ProveerEntrada(servicios(GoogleCableado)) {
                        PrimerUso(onListo = {})
                    }
                }
            }
        }
        compose.waitForIdle()

        // Paso 1 y 2 hasta llegar al que cambia.
        compose.onNodeWithText("Siguiente").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Siguiente").performClick()
        compose.waitForIdle()

        compose.onNodeWithText("Tu cuenta de Google", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
    }

    @Test
    fun `sin servicios provistos el primer uso no finge que hay Google`() {
        compose.setContent {
            RbTheme(darkTheme = true, reducedMotion = true) {
                Box(Modifier.fillMaxSize()) { PrimerUso(onListo = {}) }
            }
        }
        compose.waitForIdle()

        compose.onNodeWithText("Siguiente").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Siguiente").performClick()
        compose.waitForIdle()

        compose.onNodeWithText("Pronto también", substring = true)
            .performScrollTo()
            .assertIsDisplayed()
    }
}
